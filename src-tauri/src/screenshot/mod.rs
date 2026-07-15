use crate::crypto::DeviceKeys;
use crate::error::{guard_native, AgentError, AgentResult};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

mod constants;
mod process;
mod remote;
mod storage;

pub use constants::{
    DEFAULT_JPEG_QUALITY, SCREENSHOT_FILE_EXTENSION, SETTING_BLUR_ENABLED, SETTING_JPEG_QUALITY,
};
pub use process::normalize_jpeg_quality;
pub use remote::resolve_screenshot_image;
pub use storage::upload_capture;

use process::process_capture_bytes;
use std::path::Path;

/// Captura o monitor que contém o centro da janela ativa (`Monitor::from_point`).
/// Fallback: monitor primário, depois o primeiro disponível.
pub struct ScreenshotCapture {
    output_dir: PathBuf,
    blur_enabled: bool,
    jpeg_quality: u8,
}

#[derive(Debug, Clone)]
pub struct TrackingScreenshotCaptureContext<'a> {
    pub tracking_id: &'a str,
    pub period_start: &'a str,
    pub time_category: &'a str,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingScreenshotRecord {
    pub id: String,
    pub tracking_id: String,
    pub original_id: String,
    pub file_path: String,
    pub sha256_hash: String,
    pub width: i32,
    pub height: i32,
    pub captured_at: String,
    pub blur_applied: bool,
}

impl ScreenshotCapture {
    pub fn new(output_dir: PathBuf) -> AgentResult<Self> {
        std::fs::create_dir_all(&output_dir)?;
        Ok(Self {
            output_dir,
            blur_enabled: false,
            jpeg_quality: DEFAULT_JPEG_QUALITY,
        })
    }

    pub fn set_blur(&mut self, enabled: bool) {
        self.blur_enabled = enabled;
    }

    pub fn set_jpeg_quality(&mut self, quality: u8) {
        self.jpeg_quality = normalize_jpeg_quality(quality);
    }

    pub fn capture_pixels(&self) -> AgentResult<(i32, i32, Vec<u8>)> {
        capture_screen_png()
    }

    pub fn persist_capture(
        &self,
        conn: &Connection,
        context: &TrackingScreenshotCaptureContext<'_>,
        width: i32,
        height: i32,
        image_bytes: &[u8],
    ) -> AgentResult<TrackingScreenshotRecord> {
        let id = Uuid::new_v4().to_string();
        let original_id = id.clone();
        let captured_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let file_name = format!("{id}.{SCREENSHOT_FILE_EXTENSION}");
        let file_path = self.output_dir.join(&file_name);

        let (stored_width, stored_height, stored_bytes) =
            process_capture_bytes(image_bytes, self.blur_enabled, self.jpeg_quality)?;
        let width = if stored_width > 0 { stored_width } else { width };
        let height = if stored_height > 0 { stored_height } else { height };

        let sha256_hash = DeviceKeys::hash_bytes(&stored_bytes);

        // Write DB record FIRST, then file. This way if the process crashes
        // between the two operations, the orphan record has no matching file
        // (harmless) rather than the reverse (an orphan file with no DB
        // record that would never be cleaned up).
        let now = captured_at.clone();
        conn.execute(
            "INSERT INTO tracking_screenshots (
                id, path, tracking_id, original_id, captured_at,
                period_started_at, time_category, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                id,
                file_path.to_string_lossy().to_string(),
                context.tracking_id,
                original_id,
                captured_at,
                context.period_start,
                context.time_category,
                now
            ],
        )?;

        // File write AFTER DB insert — if it fails, the DB record is a
        // harmless orphan (path points to a non-existent file) and the
        // sync worker will handle the error gracefully.
        std::fs::write(&file_path, &stored_bytes)?;

        Ok(TrackingScreenshotRecord {
            id,
            tracking_id: context.tracking_id.to_string(),
            original_id,
            file_path: file_path.to_string_lossy().to_string(),
            sha256_hash,
            width,
            height,
            captured_at,
            blur_applied: self.blur_enabled,
        })
    }
}

fn capture_screen_png() -> AgentResult<(i32, i32, Vec<u8>)> {
    guard_native("screenshot", || {
        let monitor = select_capture_monitor()?;
        let image = monitor
            .capture_image()
            .map_err(|e| AgentError::Other(e.to_string()))?;
        let width = image.width() as i32;
        let height = image.height() as i32;

        let mut png_bytes: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        image
            .write_to(&mut cursor, xcap::image::ImageFormat::Png)
            .map_err(|e| AgentError::Other(e.to_string()))?;

        Ok((width, height, png_bytes))
    })
}

fn select_capture_monitor() -> AgentResult<xcap::Monitor> {
    if let Ok(window) = active_win_pos_rs::get_active_window() {
        let center_x = (window.position.x + window.position.width / 2.0) as i32;
        let center_y = (window.position.y + window.position.height / 2.0) as i32;
        if let Ok(monitor) = xcap::Monitor::from_point(center_x, center_y) {
            return Ok(monitor);
        }
    }

    let monitors = xcap::Monitor::all().map_err(|e| AgentError::Other(e.to_string()))?;
    monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or(monitors.first())
        .cloned()
        .ok_or_else(|| AgentError::Other("no monitor found".into()))
}

pub fn cache_dir_for_db(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|dir| dir.join("screenshots").join("cache"))
        .unwrap_or_else(|| PathBuf::from("screenshots/cache"))
}

pub fn cache_file_path(db_path: &Path, screenshot_id: &str) -> PathBuf {
    cache_dir_for_db(db_path).join(format!("{screenshot_id}.{SCREENSHOT_FILE_EXTENSION}"))
}

pub fn purge_local_file(path: &str) -> AgentResult<()> {
    let file = Path::new(path);
    if !file.is_file() {
        return Ok(());
    }

    let parent = file
        .parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str());
    if parent != Some("screenshots") {
        return Err(AgentError::Other(format!(
            "refusing to delete screenshot outside screenshots dir: {path}"
        )));
    }

    std::fs::remove_file(file)?;
    log::info!("purged local screenshot {path}");
    Ok(())
}

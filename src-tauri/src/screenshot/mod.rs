use crate::crypto::DeviceKeys;
use crate::error::{guard_native, AgentError, AgentResult};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

mod constants;
mod process;
mod remote;
mod storage;

pub use constants::{
    DEFAULT_JPEG_QUALITY, SCREENSHOT_FILE_EXTENSION, SETTING_JPEG_QUALITY,
};
pub use process::normalize_jpeg_quality;
pub use process::process_raw_rgba;
pub use remote::resolve_screenshot_image;
pub use storage::upload_capture;

use std::path::Path;

pub struct ScreenshotCapture {
    output_dir: PathBuf,
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
    pub is_duplicate: bool,
    pub activity_level: String,
}

impl ScreenshotCapture {
    pub fn new(output_dir: PathBuf) -> AgentResult<Self> {
        std::fs::create_dir_all(&output_dir)?;
        Ok(Self {
            output_dir,
            jpeg_quality: DEFAULT_JPEG_QUALITY,
        })
    }

    pub fn set_jpeg_quality(&mut self, quality: u8) {
        self.jpeg_quality = normalize_jpeg_quality(quality);
    }

    pub fn jpeg_quality(&self) -> u8 {
        self.jpeg_quality
    }

    pub fn capture_pixels(&self) -> AgentResult<(i32, i32, Vec<u8>)> {
        capture_all_monitors_png()
    }

    pub fn persist_capture(
        &self,
        conn: &Connection,
        context: &TrackingScreenshotCaptureContext<'_>,
        width: i32,
        height: i32,
        image_bytes: &[u8],
        is_duplicate: bool,
        activity_level: &str,
    ) -> AgentResult<TrackingScreenshotRecord> {
        let id = Uuid::new_v4().to_string();
        let original_id = id.clone();
        let captured_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let file_name = format!("{id}.{SCREENSHOT_FILE_EXTENSION}");
        let file_path = self.output_dir.join(&file_name);

        let (stored_width, stored_height, stored_bytes) =
            process_raw_rgba(image_bytes, width, height, self.jpeg_quality)?;
        let width = if stored_width > 0 { stored_width } else { width };
        let height = if stored_height > 0 { stored_height } else { height };

        let sha256_hash = DeviceKeys::hash_bytes(&stored_bytes);

        let now = captured_at.clone();
        conn.execute(
            "INSERT INTO tracking_screenshots (
                id, path, tracking_id, original_id, captured_at,
                period_started_at, time_category, is_duplicate, activity_level,
                created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                id,
                file_path.to_string_lossy().to_string(),
                context.tracking_id,
                original_id,
                captured_at,
                context.period_start,
                context.time_category,
                is_duplicate,
                activity_level,
                now
            ],
        )?;

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
            is_duplicate,
            activity_level: activity_level.to_string(),
        })
    }
}

fn capture_all_monitors_png() -> AgentResult<(i32, i32, Vec<u8>)> {
    guard_native("screenshot", || {
        let monitors = xcap::Monitor::all()
            .map_err(|e| AgentError::Other(e.to_string()))?;

        if monitors.is_empty() {
            return Err(AgentError::Other("no monitors found".into()));
        }

        let mut positions: Vec<(i32, i32, u32, u32)> = Vec::with_capacity(monitors.len());
        let mut scale_factors: Vec<f32> = Vec::with_capacity(monitors.len());
        for m in &monitors {
            let x = m.x().map_err(|e| AgentError::Other(e.to_string()))?;
            let y = m.y().map_err(|e| AgentError::Other(e.to_string()))?;
            let w = m.width().map_err(|e| AgentError::Other(e.to_string()))?;
            let h = m.height().map_err(|e| AgentError::Other(e.to_string()))?;
            positions.push((x, y, w, h));
            let sf = m.scale_factor().unwrap_or(1.0);
            scale_factors.push(sf);
        }

        let min_x = positions.iter().map(|(x, _, _, _)| *x).min().unwrap_or(0);
        let min_y = positions.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);
        let max_x = positions
            .iter()
            .map(|(x, _, w, _)| x + *w as i32)
            .max()
            .unwrap_or(0);
        let max_y = positions
            .iter()
            .map(|(_, y, _, h)| y + *h as i32)
            .max()
            .unwrap_or(0);

        let canvas_w = (max_x - min_x) as u32;
        let canvas_h = (max_y - min_y) as u32;

        let mut canvas = xcap::image::RgbaImage::new(canvas_w, canvas_h);

        for (i, monitor) in monitors.iter().enumerate() {
            let (mx, my, _mw, _mh) = positions[i];
            let img = monitor
                .capture_image()
                .map_err(|e| AgentError::Other(e.to_string()))?;

            let scale = scale_factors[i];
            let img = if scale > 1.0 && scale.is_finite() {
                let pixel_w = img.width();
                let pixel_h = img.height();
                let point_w = (pixel_w as f32 / scale).round() as u32;
                let point_h = (pixel_h as f32 / scale).round() as u32;

                if pixel_w != point_w || pixel_h != point_h {
                    image::imageops::resize(
                        &img,
                        point_w.max(1),
                        point_h.max(1),
                        image::imageops::FilterType::CatmullRom,
                    )
                } else {
                    img
                }
            } else {
                img
            };

            let offset_x = (mx - min_x) as i64;
            let offset_y = (my - min_y) as i64;
            image::imageops::overlay(&mut canvas, &img, offset_x, offset_y);
        }

        Ok((canvas_w as i32, canvas_h as i32, canvas.into_raw()))
    })
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

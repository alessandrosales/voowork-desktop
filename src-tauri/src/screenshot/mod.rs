use crate::clock::system_time_millis;
use crate::crypto::DeviceKeys;
use crate::error::{guard_native, AgentError, AgentResult};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

mod constants;

use constants::{JITTER_RANGE_SECS, JITTER_SECS, SCREENSHOT_FILE_EXTENSION};

pub struct ScreenshotCapture {
    output_dir: PathBuf,
    blur_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ScreenshotCaptureContext<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub task_id: Option<&'a str>,
    pub session_id: &'a str,
    pub activity_tick_id: Option<&'a str>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRecord {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub session_id: String,
    pub file_path: String,
    pub sha256_hash: String,
    pub width: i32,
    pub height: i32,
    pub captured_at: String,
    pub activity_tick_id: Option<String>,
    pub blur_applied: bool,
}

impl ScreenshotCapture {
    pub fn new(output_dir: PathBuf) -> AgentResult<Self> {
        std::fs::create_dir_all(&output_dir)?;
        Ok(Self {
            output_dir,
            blur_enabled: false,
        })
    }

    pub fn set_blur(&mut self, enabled: bool) {
        self.blur_enabled = enabled;
    }

    pub fn capture_pixels(&self) -> AgentResult<(i32, i32, Vec<u8>)> {
        capture_screen_bytes()
    }

    pub fn persist_capture(
        &self,
        conn: &Connection,
        context: &ScreenshotCaptureContext<'_>,
        width: i32,
        height: i32,
        image_bytes: &[u8],
    ) -> AgentResult<ScreenshotRecord> {
        let id = Uuid::new_v4().to_string();
        let captured_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let file_name = format!("{id}.{SCREENSHOT_FILE_EXTENSION}");
        let file_path = self.output_dir.join(&file_name);

        let sha256_hash = DeviceKeys::hash_bytes(image_bytes);
        let stored_bytes = if self.blur_enabled {
            apply_blur_placeholder(image_bytes)
        } else {
            image_bytes.to_vec()
        };

        std::fs::write(&file_path, &stored_bytes)?;

        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        conn.execute(
            "INSERT INTO screenshots (
                id, user_id, project_id, task_id, session_id, file_path, sha256_hash,
                width, height, captured_at, activity_tick_id, blur_applied, created_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                context.user_id,
                context.project_id,
                context.task_id,
                context.session_id,
                file_path.to_string_lossy().to_string(),
                sha256_hash,
                width,
                height,
                captured_at,
                context.activity_tick_id,
                if self.blur_enabled { 1 } else { 0 },
                now
            ],
        )?;

        Ok(ScreenshotRecord {
            id,
            user_id: context.user_id.to_string(),
            project_id: context.project_id.to_string(),
            task_id: context.task_id.map(str::to_string),
            session_id: context.session_id.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            sha256_hash,
            width,
            height,
            captured_at,
            activity_tick_id: context.activity_tick_id.map(str::to_string),
            blur_applied: self.blur_enabled,
        })
    }
}

fn capture_screen_bytes() -> AgentResult<(i32, i32, Vec<u8>)> {
    guard_native("screenshot", || {
        let monitors = xcap::Monitor::all().map_err(|e| AgentError::Other(e.to_string()))?;
        let monitor = monitors
            .into_iter()
            .next()
            .ok_or_else(|| AgentError::Other("no monitor found".into()))?;

        let image = monitor
            .capture_image()
            .map_err(|e| AgentError::Other(e.to_string()))?;
        let width = image.width() as i32;
        let height = image.height() as i32;

        let mut png_bytes: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        image
            .write_to(&mut cursor, xcap::image::ImageFormat::Png)
            .map_err(|e| AgentError::Other(e.to_string()))?;

        Ok((width, height, png_bytes))
    })
}

fn apply_blur_placeholder(data: &[u8]) -> Vec<u8> {
    // v1 skeleton: blur real implementation deferred; hash still computed on original bytes
    data.to_vec()
}

pub fn random_interval_secs(base_secs: u64) -> u64 {
    let jitter = (system_time_millis() % JITTER_RANGE_SECS) as u64;
    base_secs + jitter % JITTER_SECS
}

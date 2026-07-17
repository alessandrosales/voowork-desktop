use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::{TrackingScreenshotAccess, TrackingScreenshotRow};
use crate::sync::ENTITY_TRACKING_SCREENSHOT;
use rusqlite::params;

impl Database {
    pub fn get_tracking_screenshot_access(
        &self,
        screenshot_id: &str,
    ) -> AgentResult<TrackingScreenshotAccess> {
        self.conn
            .query_row(
                "SELECT tracking_id, path, synced_at
                 FROM tracking_screenshots
                 WHERE id = ?1",
                params![screenshot_id],
                |row| {
                    Ok(TrackingScreenshotAccess {
                        tracking_id: row.get(0)?,
                        path: row.get(1)?,
                        synced_at: row.get(2)?,
                    })
                },
            )
            .map_err(AgentError::from)
    }

    pub fn list_tracking_screenshots(
        &self,
        limit: i64,
        offset: i64,
    ) -> AgentResult<Vec<TrackingScreenshotRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.tracking_id, s.original_id, s.captured_at, s.path, s.remote_path, s.synced_at,
                    COALESCE(q.status, 'local') AS sync_status
             FROM tracking_screenshots s
             LEFT JOIN sync_queue q ON q.entity_type = ?3 AND q.entity_id = s.id
             ORDER BY s.captured_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(
            params![limit, offset, ENTITY_TRACKING_SCREENSHOT],
            |row| {
                let path: String = row.get(4)?;
                let remote_path: Option<String> = row.get(5)?;
                let has_local_file = std::path::Path::new(&path).is_file()
                    && remote_path.as_deref() != Some(path.as_str());

                Ok(TrackingScreenshotRow {
                    id: row.get(0)?,
                    tracking_id: row.get(1)?,
                    original_id: row.get(2)?,
                    captured_at: row.get(3)?,
                    path,
                    remote_path,
                    synced_at: row.get(6)?,
                    sync_status: row.get(7)?,
                    has_local_file,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }
}

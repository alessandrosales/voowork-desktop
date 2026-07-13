use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::ScreenshotRow;
use rusqlite::params;

impl Database {
    pub fn list_screenshots(&self, limit: i64, offset: i64) -> AgentResult<Vec<ScreenshotRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.user_id, s.project_id, s.task_id, s.session_id, s.captured_at,
                    s.sha256_hash, s.width, s.height, s.blur_applied,
                    COALESCE(q.status, 'local') AS sync_status
             FROM screenshots s
             LEFT JOIN sync_queue q ON q.entity_type = 'screenshot' AND q.entity_id = s.id
             ORDER BY s.captured_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(ScreenshotRow {
                id: row.get(0)?,
                user_id: row.get(1)?,
                project_id: row.get(2)?,
                task_id: row.get(3)?,
                session_id: row.get(4)?,
                captured_at: row.get(5)?,
                sha256_hash: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                blur_applied: row.get::<_, i64>(9)? != 0,
                sync_status: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }
}

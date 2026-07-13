use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::{ActivityTickRow, AppFocusRow};
use rusqlite::params;

impl Database {
    pub fn list_activity_ticks(&self, limit: i64, offset: i64) -> AgentResult<Vec<ActivityTickRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, bucket_start, bucket_end, mouse_events, keyboard_events,
                    activity_score_confidence, record_hash
             FROM activity_ticks
             ORDER BY bucket_start DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(ActivityTickRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                bucket_start: row.get(2)?,
                bucket_end: row.get(3)?,
                mouse_events: row.get(4)?,
                keyboard_events: row.get(5)?,
                activity_score_confidence: row.get(6)?,
                record_hash: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }

    pub fn list_app_focus(
        &self,
        limit: i64,
        offset: i64,
        session_id: Option<&str>,
    ) -> AgentResult<Vec<AppFocusRow>> {
        let (sql, session_filter): (&str, Option<&str>) = if session_id.is_some() {
            (
                "SELECT id, session_id, app_name, window_title, process_path, process_id, captured_at
                 FROM app_focus_events
                 WHERE session_id = ?3
                 ORDER BY captured_at DESC
                 LIMIT ?1 OFFSET ?2",
                session_id,
            )
        } else {
            (
                "SELECT id, session_id, app_name, window_title, process_path, process_id, captured_at
                 FROM app_focus_events
                 ORDER BY captured_at DESC
                 LIMIT ?1 OFFSET ?2",
                None,
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(session_id) = session_filter {
            stmt.query_map(params![limit, offset, session_id], map_app_focus_row)?
        } else {
            stmt.query_map(params![limit, offset], map_app_focus_row)?
        };

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }
}

fn map_app_focus_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppFocusRow> {
    Ok(AppFocusRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        app_name: row.get(2)?,
        window_title: row.get(3)?,
        process_path: row.get(4)?,
        process_id: row.get(5)?,
        captured_at: row.get(6)?,
    })
}

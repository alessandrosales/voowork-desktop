use super::Database;
use crate::error::AgentResult;
use crate::models::TrackingPeripheralEventRow;
use rusqlite::params;
use uuid::Uuid;

impl Database {
    pub fn insert_tracking_peripheral_event(
        &self,
        tracking_id: &str,
        event: &str,
        count: f64,
        screenshot_original_id: &str,
        started_at: &str,
        ended_at: &str,
    ) -> AgentResult<String> {
        let id = Uuid::new_v4().to_string();
        let now = ended_at.to_string();
        self.conn.execute(
            "INSERT INTO tracking_peripheral_events (
                id, event, count, tracking_id, screenshot_original_id,
                started_at, ended_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                id,
                event,
                count,
                tracking_id,
                screenshot_original_id,
                started_at,
                ended_at,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn flush_tracking_peripheral_events_for_period(
        &self,
        tracking_id: &str,
        screenshot_original_id: &str,
        started_at: &str,
        ended_at: &str,
        mouse_events: u64,
        keyboard_events: u64,
    ) -> AgentResult<Vec<(String, String)>> {
        let mut inserted = Vec::new();

        if mouse_events > 0 {
            let id = self.insert_tracking_peripheral_event(
                tracking_id,
                "mouse_activity",
                mouse_events as f64,
                screenshot_original_id,
                started_at,
                ended_at,
            )?;
            inserted.push((id, "mouse_activity".into()));
        }

        if keyboard_events > 0 {
            let id = self.insert_tracking_peripheral_event(
                tracking_id,
                "keyboard_activity",
                keyboard_events as f64,
                screenshot_original_id,
                started_at,
                ended_at,
            )?;
            inserted.push((id, "keyboard_activity".into()));
        }

        Ok(inserted)
    }

    pub fn list_tracking_peripheral_events(
        &self,
        limit: i64,
        offset: i64,
    ) -> AgentResult<Vec<TrackingPeripheralEventRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tracking_id, event, count, screenshot_original_id, started_at, ended_at
             FROM tracking_peripheral_events
             ORDER BY started_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(TrackingPeripheralEventRow {
                id: row.get(0)?,
                tracking_id: row.get(1)?,
                event: row.get(2)?,
                count: row.get(3)?,
                screenshot_original_id: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }
}

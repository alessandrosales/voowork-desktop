use super::Database;
use crate::error::AgentResult;
use crate::models::TrackingInactivityPeriodRow;
use rusqlite::params;

impl Database {
    pub fn list_tracking_inactivity_periods(&self, limit: i64) -> AgentResult<Vec<TrackingInactivityPeriodRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tracking_id, inactivity_started_at, paused_at, resumed_at,
                    duration_seconds, discarded_seconds, reclassified_seconds,
                    category, status
             FROM tracking_inactivity_periods
             ORDER BY inactivity_started_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(TrackingInactivityPeriodRow {
                id: row.get(0)?,
                tracking_id: row.get(1)?,
                inactivity_started_at: row.get(2)?,
                paused_at: row.get(3)?,
                resumed_at: row.get(4)?,
                duration_seconds: row.get::<_, i64>(5)?.max(0) as u64,
                discarded_seconds: row.get::<_, i64>(6)?.max(0) as u64,
                reclassified_seconds: row.get::<_, i64>(7)?.max(0) as u64,
                category: row.get(8)?,
                status: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }
}

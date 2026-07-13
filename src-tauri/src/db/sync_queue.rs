use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::SyncQueueRow;
use rusqlite::params;

impl Database {
    pub fn sync_queue_stats(&self) -> AgentResult<(i64, i64, i64)> {
        let pending: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status IN ('pending', 'sending', 'failed')",
            [],
            |row| row.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status = 'failed'",
            [],
            |row| row.get(0),
        )?;
        let confirmed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status = 'confirmed'",
            [],
            |row| row.get(0),
        )?;
        Ok((pending, failed, confirmed))
    }

    pub fn list_sync_queue(&self, limit: i64, offset: i64) -> AgentResult<Vec<SyncQueueRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, status, attempts, error_message, created_at
             FROM sync_queue
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(SyncQueueRow {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                status: row.get(3)?,
                attempts: row.get(4)?,
                error_message: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }
}

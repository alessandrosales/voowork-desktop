use super::Database;
use crate::error::AgentResult;
use rusqlite::{params, OptionalExtension};

impl Database {
    pub fn get_setting(&self, key: &str) -> AgentResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let value = stmt
            .query_row(params![key], |row| row.get(0))
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn device_is_registered(&self) -> AgentResult<bool> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM device_metadata", [], |row| row.get(0))?;
        Ok(count > 0)
    }
}

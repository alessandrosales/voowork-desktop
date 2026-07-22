use crate::error::AgentResult;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct DeviceKeys;

impl DeviceKeys {
    pub fn ensure(conn: &Connection, device_name: &str) -> AgentResult<Self> {
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM device_metadata LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if existing.is_none() {
            let now = chrono::Utc::now().to_rfc3339();
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO device_metadata (id, device_name, public_key, private_key_b64, created_at, updated_at)
                 VALUES (?1, ?2, '', '', ?3, ?4)",
                params![id, device_name, now, now],
            )?;
        }

        Ok(Self)
    }

    pub fn device_name(conn: &Connection) -> AgentResult<String> {
        conn.query_row(
            "SELECT device_name FROM device_metadata LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(AgentError::from)
    }

    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

use crate::error::AgentError;
use rusqlite::OptionalExtension;

use crate::error::{AgentError, AgentResult};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct DeviceKeys {
    signing_key: SigningKey,
}

impl DeviceKeys {
    pub fn ensure(conn: &Connection, device_name: &str) -> AgentResult<Self> {
        let existing: Option<String> = conn
            .query_row(
                "SELECT private_key_b64 FROM device_metadata LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(private_key_b64) = existing {
            let key_bytes = STANDARD
                .decode(private_key_b64)
                .map_err(|e| AgentError::Crypto(e.to_string()))?;
            let signing_key = SigningKey::from_bytes(
                &key_bytes
                    .try_into()
                    .map_err(|_| AgentError::Crypto("invalid key length".into()))?,
            );
            return Ok(Self { signing_key });
        }

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let public_key_b64 = STANDARD.encode(verifying_key.to_bytes());
        let private_key_b64 = STANDARD.encode(signing_key.to_bytes());
        let now = chrono::Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO device_metadata (id, device_name, public_key, private_key_b64, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, device_name, public_key_b64, private_key_b64, now, now],
        )?;

        Ok(Self { signing_key })
    }

    pub fn sign_payload(&self, payload: &str) -> String {
        let signature = self.signing_key.sign(payload.as_bytes());
        STANDARD.encode(signature.to_bytes())
    }

    pub fn public_key_b64(conn: &Connection) -> AgentResult<String> {
        conn.query_row(
            "SELECT public_key FROM device_metadata LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(AgentError::from)
    }

    pub fn device_id(conn: &Connection) -> AgentResult<String> {
        conn.query_row("SELECT id FROM device_metadata LIMIT 1", [], |row| row.get(0))
            .map_err(AgentError::from)
    }

    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

use rusqlite::OptionalExtension;

use crate::error::AgentResult;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

pub struct SyncOutbox;

impl SyncOutbox {
    pub fn enqueue(
        conn: &Connection,
        entity_type: &str,
        entity_id: &str,
        payload: impl serde::Serialize,
    ) -> AgentResult<String> {
        let payload_str = serde_json::to_string(&payload)?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO sync_queue (id, entity_type, entity_id, payload_json, status, created_at, next_retry_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?5)",
            params![id, entity_type, entity_id, payload_str, now],
        )?;

        Ok(id)
    }

    pub fn mark_sending(conn: &Connection, id: &str) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sync_queue SET status = 'sending', last_attempt_at = ?2, attempts = attempts + 1 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_confirmed(conn: &Connection, id: &str) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sync_queue SET status = 'confirmed', confirmed_at = ?2, error_message = NULL WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Move o item para dead-letter (`status = 'dead'`): não é mais buscado
    /// por `fetch_pending_batch`, encerrando o retry. Usado para erros
    /// permanentes (4xx) e para itens que estouraram o limite de tentativas.
    pub fn mark_dead(conn: &Connection, id: &str, error: &str) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sync_queue SET status = 'dead', error_message = ?2, last_attempt_at = ?3, next_retry_at = NULL WHERE id = ?1",
            params![id, error, now],
        )?;
        Ok(())
    }

    pub fn mark_failed(conn: &Connection, id: &str, error: &str, attempts: i64) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let backoff_secs = (2_i64.pow(attempts.min(8) as u32)).min(3600);
        let next_retry = chrono::Utc::now() + chrono::Duration::seconds(backoff_secs);

        conn.execute(
            "UPDATE sync_queue SET status = 'failed', error_message = ?2, last_attempt_at = ?3, next_retry_at = ?4 WHERE id = ?1",
            params![id, error, now, next_retry.to_rfc3339()],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PendingSyncItem {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub payload_json: String,
    pub attempts: i64,
}

/// Recupera itens presos em `sending` após um crash/kill no meio de um envio.
///
/// Sem isso, um item marcado `sending` (attempts já incrementado) nunca mais
/// é buscado por `fetch_pending_batch` — fica órfão para sempre. Devolvê-lo
/// para `pending` no boot permite o reprocessamento; a idempotência por UUID
/// no backend protege contra duplo envio caso o envio original tenha chegado.
///
/// Deve ser chamado no boot, antes de o worker de sync iniciar.
pub fn requeue_stuck_sending_items(conn: &Connection) -> AgentResult<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE sync_queue SET status = 'pending', next_retry_at = ?1 WHERE status = 'sending'",
        params![now],
    )?;
    Ok(affected)
}

pub fn fetch_pending_batch(conn: &Connection, limit: usize) -> AgentResult<Vec<PendingSyncItem>> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, payload_json, attempts
         FROM sync_queue
         WHERE status IN ('pending', 'failed') AND (next_retry_at IS NULL OR next_retry_at <= ?1)
         ORDER BY created_at ASC
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![now, limit as i64], |row| {
        Ok(PendingSyncItem {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            payload_json: row.get(3)?,
            attempts: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn tracking_screenshot_file_path(conn: &Connection, screenshot_id: &str) -> AgentResult<Option<String>> {
    conn.query_row(
        "SELECT path FROM tracking_screenshots WHERE id = ?1",
        params![screenshot_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn mark_tracking_screenshot_synced(
    conn: &Connection,
    screenshot_id: &str,
    remote_path: Option<&str>,
) -> AgentResult<()> {
    let local_path = tracking_screenshot_file_path(conn, screenshot_id)?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(remote_path) = remote_path {
        conn.execute(
            "UPDATE tracking_screenshots
             SET updated_at = ?2, synced_at = ?2, remote_path = ?3, path = ?3
             WHERE id = ?1",
            params![screenshot_id, now, remote_path],
        )?;

        if let Some(local_path) = local_path {
            if local_path != remote_path {
                if let Err(err) = crate::screenshot::purge_local_file(&local_path) {
                    log::warn!("failed to purge local screenshot {local_path}: {err}");
                }
            }
        }
    } else {
        conn.execute(
            "UPDATE tracking_screenshots SET updated_at = ?2, synced_at = ?2 WHERE id = ?1",
            params![screenshot_id, now],
        )?;
    }

    Ok(())
}

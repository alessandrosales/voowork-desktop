use crate::crypto::DeviceKeys;
use crate::error::AgentResult;
use crate::sync::{SyncOutbox, ENTITY_IDLE_PERIOD};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::constants::{
    DEFAULT_IDLE_THRESHOLD_MINUTES, SETTING_PROFILE, SETTING_THRESHOLD_MINUTES,
};

pub fn load_idle_threshold_minutes(conn: &Connection) -> u64 {
    if let Ok(Some(value)) = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING_THRESHOLD_MINUTES],
            |row| row.get::<_, String>(0),
        )
        .optional()
    {
        if let Ok(minutes) = value.parse::<u64>() {
            if (1..=120).contains(&minutes) {
                return minutes;
            }
        }
    }

    let profile = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING_PROFILE],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or_else(|| "standard".into());

    match profile.as_str() {
        "data_entry" => 3,
        "knowledge" => 15,
        "meeting_heavy" => 30,
        _ => DEFAULT_IDLE_THRESHOLD_MINUTES,
    }
}

pub fn insert_paused_idle_period(
    conn: &Connection,
    session_id: &str,
    idle_started_at: &str,
    paused_at: &str,
    discarded_seconds: u64,
    device_keys: &DeviceKeys,
) -> AgentResult<String> {
    let period_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let discarded = discarded_seconds as i64;

    conn.execute(
        "INSERT INTO idle_periods (
            id, session_id, idle_started_at, paused_at, duration_seconds,
            discarded_seconds, reclassified_seconds, status, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 'paused', ?7, ?7)",
        params![
            period_id,
            session_id,
            idle_started_at,
            paused_at,
            discarded,
            discarded,
            created_at,
        ],
    )?;

    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "sessionId": session_id,
        "idleStartedAt": idle_started_at,
        "pausedAt": paused_at,
        "discardedSeconds": discarded_seconds,
        "status": "paused",
    });
    SyncOutbox::enqueue(conn, ENTITY_IDLE_PERIOD, &period_id, payload, device_keys)?;

    Ok(period_id)
}

/// Finaliza o período ocioso ao retorno do usuário.
/// Todo o intervalo desde `idle_started_at` até agora é descartado (não faturável).
pub fn finalize_idle_period_on_resume(
    conn: &Connection,
    period_id: &str,
    device_keys: &DeviceKeys,
) -> AgentResult<(u64, u64)> {
    let (idle_started_at, previous_discarded): (String, i64) = conn.query_row(
        "SELECT idle_started_at, discarded_seconds FROM idle_periods WHERE id = ?1",
        [period_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let resumed_at = chrono::Utc::now();
    let total_discarded = chrono::DateTime::parse_from_rfc3339(&idle_started_at)
        .ok()
        .map(|start| {
            resumed_at
                .signed_duration_since(start.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0) as u64
        })
        .unwrap_or(previous_discarded.max(0) as u64);

    let previous = previous_discarded.max(0) as u64;
    let resumed_at_str = resumed_at.to_rfc3339();

    conn.execute(
        "UPDATE idle_periods SET resumed_at = ?1, duration_seconds = ?2,
            discarded_seconds = ?2, updated_at = ?1
         WHERE id = ?3",
        params![resumed_at_str, total_discarded as i64, period_id],
    )?;

    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "resumedAt": resumed_at_str,
        "discardedSeconds": total_discarded,
        "durationSeconds": total_discarded,
        "status": "resumed",
    });
    SyncOutbox::enqueue(conn, ENTITY_IDLE_PERIOD, period_id, payload, device_keys)?;

    Ok((total_discarded, previous))
}

pub fn classify_idle_period_record(
    conn: &Connection,
    period_id: &str,
    category: &str,
    device_keys: &DeviceKeys,
) -> AgentResult<(bool, u64)> {
    let reclassify = matches!(category, "meeting_call" | "offline_work");
    let duration = conn.query_row(
        "SELECT discarded_seconds FROM idle_periods WHERE id = ?1",
        [period_id],
        |row| row.get::<_, i64>(0),
    )? as u64;

    let now = chrono::Utc::now().to_rfc3339();
    if reclassify {
        conn.execute(
            "UPDATE idle_periods SET category = ?1, status = 'reclassified', resumed_at = ?2,
             reclassified_seconds = discarded_seconds, discarded_seconds = 0, updated_at = ?2
             WHERE id = ?3",
            params![category, now, period_id],
        )?;
    } else {
        conn.execute(
            "UPDATE idle_periods SET category = ?1, status = 'discarded', resumed_at = ?2, updated_at = ?2
             WHERE id = ?3",
            params![category, now, period_id],
        )?;
    }

    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "category": category,
        "reclassified": reclassify,
        "durationSeconds": duration,
    });
    SyncOutbox::enqueue(conn, ENTITY_IDLE_PERIOD, period_id, payload, device_keys)?;

    Ok((reclassify, duration))
}

pub fn discard_idle_period_record(
    conn: &Connection,
    period_id: &str,
    device_keys: &DeviceKeys,
) -> AgentResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE idle_periods SET status = 'discarded', resumed_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, period_id],
    )?;

    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "status": "discarded",
    });
    SyncOutbox::enqueue(conn, ENTITY_IDLE_PERIOD, period_id, payload, device_keys)?;

    Ok(())
}

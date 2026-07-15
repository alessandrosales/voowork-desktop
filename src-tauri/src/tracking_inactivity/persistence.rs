use crate::error::AgentResult;
use crate::sync::{SyncOutbox, ENTITY_TRACKING_INACTIVITY_PERIOD};
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use super::constants::{
    DEFAULT_INACTIVITY_THRESHOLD_MINUTES, SETTING_INACTIVITY_PROFILE, SETTING_INACTIVITY_THRESHOLD_MINUTES,
};

pub fn load_inactivity_threshold_minutes(conn: &Connection) -> u64 {
    if let Ok(Some(value)) = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING_INACTIVITY_THRESHOLD_MINUTES],
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
            [SETTING_INACTIVITY_PROFILE],
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
        _ => DEFAULT_INACTIVITY_THRESHOLD_MINUTES,
    }
}

pub fn insert_paused_inactivity_period(
    conn: &Connection,
    tracking_id: &str,
    inactivity_started_at: &str,
    paused_at: &str,
    discarded_seconds: u64,
) -> AgentResult<String> {
    let period_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracking_inactivity_periods (
            id, tracking_id, inactivity_started_at, paused_at, status,
            duration_seconds, discarded_seconds, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, 'paused', ?5, ?5, ?6, ?6)",
        rusqlite::params![period_id, tracking_id, inactivity_started_at, paused_at, discarded_seconds, paused_at],
    )?;
    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "trackingId": tracking_id,
        "idleStartedAt": inactivity_started_at,
        "pausedAt": paused_at,
        "discardedSeconds": discarded_seconds,
        "status": "paused",
    });
    SyncOutbox::enqueue(conn, ENTITY_TRACKING_INACTIVITY_PERIOD, &period_id, payload)?;
    Ok(period_id)
}

pub fn finalize_inactivity_period_on_resume(
    conn: &Connection,
    period_id: &str,
    inactivity_started_at: &str,
    previous_discarded: u64,
) -> AgentResult<(u64, u64)> {
    let resumed_at = chrono::Utc::now();
    let total_discarded = chrono::DateTime::parse_from_rfc3339(inactivity_started_at)
        .ok()
        .map(|start| {
            resumed_at
                .signed_duration_since(start.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0) as u64
        })
        .unwrap_or(previous_discarded);

    let resumed_at_str = resumed_at.to_rfc3339();
    conn.execute(
        "UPDATE tracking_inactivity_periods
         SET resumed_at = ?2, duration_seconds = ?3, discarded_seconds = ?4,
             status = 'resumed', updated_at = ?2
         WHERE id = ?1",
        rusqlite::params![period_id, resumed_at_str, total_discarded, total_discarded],
    )?;
    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "resumedAt": resumed_at_str,
        "discardedSeconds": total_discarded,
        "durationSeconds": total_discarded,
        "status": "resumed",
    });
    SyncOutbox::enqueue(conn, ENTITY_TRACKING_INACTIVITY_PERIOD, period_id, payload)?;

    Ok((total_discarded, previous_discarded))
}

pub fn classify_tracking_inactivity_period_record(
    conn: &Connection,
    period_id: &str,
    category: &str,
    discarded_seconds: u64,
) -> AgentResult<(bool, u64)> {
    let reclassify = matches!(category, "meeting_call" | "offline_work");
    let now = chrono::Utc::now().to_rfc3339();
    let reclassified_seconds = if reclassify { discarded_seconds } else { 0 };
    conn.execute(
        "UPDATE tracking_inactivity_periods
         SET category = ?2, reclassified_seconds = ?3, status = 'classified', updated_at = ?4
         WHERE id = ?1",
        rusqlite::params![period_id, category, reclassified_seconds, now],
    )?;
    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "category": category,
        "reclassified": reclassify,
        "durationSeconds": discarded_seconds,
        "resumedAt": now,
    });
    SyncOutbox::enqueue(conn, ENTITY_TRACKING_INACTIVITY_PERIOD, period_id, payload)?;
    Ok((reclassify, discarded_seconds))
}

pub fn discard_inactivity_period_record(
    conn: &Connection,
    period_id: &str,
) -> AgentResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE tracking_inactivity_periods SET status = 'discarded', updated_at = ?2 WHERE id = ?1",
        rusqlite::params![period_id, now],
    )?;
    let payload = serde_json::json!({
        "idlePeriodId": period_id,
        "status": "discarded",
    });
    SyncOutbox::enqueue(conn, ENTITY_TRACKING_INACTIVITY_PERIOD, period_id, payload)?;
    Ok(())
}

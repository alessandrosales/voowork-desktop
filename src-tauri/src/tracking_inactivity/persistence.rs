use crate::error::AgentResult;
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
    Ok(period_id)
}

/// Finaliza o período de inatividade ao retomar a atividade.
///
/// Retorna a **duração deste período** (wall-clock entre o início da
/// inatividade e agora), que é gravada no próprio registro como
/// `discarded_seconds`. O acumulado da sessão é responsabilidade do
/// controller (soma as durações de cada período) — ver `state.rs`.
pub fn finalize_inactivity_period_on_resume(
    conn: &Connection,
    period_id: &str,
    inactivity_started_at: &str,
) -> AgentResult<u64> {
    let resumed_at = chrono::Utc::now();
    let period_seconds = chrono::DateTime::parse_from_rfc3339(inactivity_started_at)
        .ok()
        .map(|start| {
            resumed_at
                .signed_duration_since(start.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0) as u64
        })
        .unwrap_or(0);

    let resumed_at_str = resumed_at.to_rfc3339();
    conn.execute(
        "UPDATE tracking_inactivity_periods
         SET resumed_at = ?2, duration_seconds = ?3, discarded_seconds = ?4,
             status = 'resumed', updated_at = ?2
         WHERE id = ?1",
        rusqlite::params![period_id, resumed_at_str, period_seconds, period_seconds],
    )?;
    Ok(period_seconds)
}

/// Classifica um período. Usa o `discarded_seconds` **do próprio registro**
/// (a duração daquele período) — nunca o acumulado da sessão — para calcular
/// quanto tempo é creditado de volta (`reclassified_seconds`).
///
/// Retorna `(reclassificado?, segundos_do_período)`.
pub fn classify_tracking_inactivity_period_record(
    conn: &Connection,
    period_id: &str,
    category: &str,
) -> AgentResult<(bool, u64)> {
    let period_seconds: u64 = conn
        .query_row(
            "SELECT discarded_seconds FROM tracking_inactivity_periods WHERE id = ?1",
            [period_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .map(|v| v.max(0) as u64)
        .unwrap_or(0);

    let reclassify = matches!(category, "meeting_call" | "offline_work");
    let now = chrono::Utc::now().to_rfc3339();
    let reclassified_seconds = if reclassify { period_seconds } else { 0 };
    conn.execute(
        "UPDATE tracking_inactivity_periods
         SET category = ?2, reclassified_seconds = ?3, status = 'classified', updated_at = ?4
         WHERE id = ?1",
        rusqlite::params![period_id, category, reclassified_seconds, now],
    )?;
    Ok((reclassify, period_seconds))
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
    Ok(())
}

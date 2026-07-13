use crate::error::AgentResult;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub const GENESIS_HASH: &str = "genesis";

pub fn compute_hash(prev_hash: &str, payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(payload.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn session_payload(
    session_id: &str,
    project_id: &str,
    task_id: Option<&str>,
    started_at: &str,
    monotonic_started_ns: i64,
) -> String {
    format!(
        "session|{session_id}|{project_id}|{}|{started_at}|{monotonic_started_ns}",
        task_id.unwrap_or("")
    )
}

pub fn activity_tick_payload(
    tick_id: &str,
    session_id: &str,
    bucket_start: &str,
    bucket_end: &str,
    mouse_events: i64,
    keyboard_events: i64,
    activity_score_confidence: f64,
    monotonic_elapsed_ns: i64,
    wall_clock_at_tick: &str,
) -> String {
    format!(
        "tick|{tick_id}|{session_id}|{bucket_start}|{bucket_end}|{mouse_events}|{keyboard_events}|{activity_score_confidence:.6}|{monotonic_elapsed_ns}|{wall_clock_at_tick}"
    )
}

pub fn last_session_hash(conn: &Connection) -> AgentResult<String> {
    let hash: Option<String> = conn
        .query_row(
            "SELECT record_hash FROM sessions ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(hash.unwrap_or_else(|| GENESIS_HASH.to_string()))
}

pub fn last_activity_tick_hash(conn: &Connection, session_id: &str) -> AgentResult<String> {
    let hash: Option<String> = conn
        .query_row(
            "SELECT record_hash FROM activity_ticks WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(hash.unwrap_or_else(|| GENESIS_HASH.to_string()))
}

pub fn insert_session(
    conn: &Connection,
    session_id: &str,
    project_id: &str,
    task_id: Option<&str>,
    started_at: &str,
    monotonic_started_ns: i64,
) -> AgentResult<String> {
    let prev_hash = last_session_hash(conn)?;
    let payload = session_payload(
        session_id,
        project_id,
        task_id,
        started_at,
        monotonic_started_ns,
    );
    let record_hash = compute_hash(&prev_hash, &payload);
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO sessions (id, project_id, task_id, started_at, monotonic_started_ns, status, prev_hash, record_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?7, ?8)",
        params![
            session_id,
            project_id,
            task_id,
            started_at,
            monotonic_started_ns,
            prev_hash,
            record_hash,
            now
        ],
    )?;

    Ok(record_hash)
}

pub fn finalize_session(
    conn: &Connection,
    session_id: &str,
    ended_at: &str,
    monotonic_ended_ns: i64,
    clock_skew_flags: i64,
) -> AgentResult<()> {
    conn.execute(
        "UPDATE sessions SET ended_at = ?2, monotonic_ended_ns = ?3, status = 'stopped', clock_skew_flags = ?4
         WHERE id = ?1",
        params![session_id, ended_at, monotonic_ended_ns, clock_skew_flags],
    )?;
    Ok(())
}

pub fn insert_activity_tick(
    conn: &Connection,
    tick: &ActivityTickRecord,
) -> AgentResult<String> {
    let prev_hash = last_activity_tick_hash(conn, &tick.session_id)?;
    let payload = activity_tick_payload(
        &tick.id,
        &tick.session_id,
        &tick.bucket_start,
        &tick.bucket_end,
        tick.mouse_events,
        tick.keyboard_events,
        tick.activity_score_confidence,
        tick.monotonic_elapsed_ns,
        &tick.wall_clock_at_tick,
    );
    let record_hash = compute_hash(&prev_hash, &payload);
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO activity_ticks (
            id, session_id, bucket_start, bucket_end, mouse_events, keyboard_events,
            mouse_positions_json, activity_score_confidence, automation_flags,
            monotonic_elapsed_ns, wall_clock_at_tick, clock_skew_detected,
            prev_hash, record_hash, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            tick.id,
            tick.session_id,
            tick.bucket_start,
            tick.bucket_end,
            tick.mouse_events,
            tick.keyboard_events,
            tick.mouse_positions_json,
            tick.activity_score_confidence,
            tick.automation_flags,
            tick.monotonic_elapsed_ns,
            tick.wall_clock_at_tick,
            tick.clock_skew_detected,
            prev_hash,
            record_hash,
            now
        ],
    )?;

    Ok(record_hash)
}

#[derive(Debug, Clone)]
pub struct ActivityTickRecord {
    pub id: String,
    pub session_id: String,
    pub bucket_start: String,
    pub bucket_end: String,
    pub mouse_events: i64,
    pub keyboard_events: i64,
    pub mouse_positions_json: Option<String>,
    pub activity_score_confidence: f64,
    pub automation_flags: i64,
    pub monotonic_elapsed_ns: i64,
    pub wall_clock_at_tick: String,
    pub clock_skew_detected: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChainValidationResult {
    pub valid: bool,
    pub broken_at_id: Option<String>,
    pub total_records: usize,
}

pub fn validate_activity_chain(conn: &Connection, session_id: &str) -> AgentResult<ChainValidationResult> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, bucket_start, bucket_end, mouse_events, keyboard_events,
                activity_score_confidence, monotonic_elapsed_ns, wall_clock_at_tick,
                prev_hash, record_hash
         FROM activity_ticks WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;

    let rows: Vec<_> = stmt
        .query_map(params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut expected_prev = GENESIS_HASH.to_string();

    for (
        id,
        sid,
        bucket_start,
        bucket_end,
        mouse_events,
        keyboard_events,
        confidence,
        monotonic_elapsed_ns,
        wall_clock_at_tick,
        prev_hash,
        record_hash,
    ) in &rows
    {
        if prev_hash != &expected_prev {
            return Ok(ChainValidationResult {
                valid: false,
                broken_at_id: Some(id.clone()),
                total_records: rows.len(),
            });
        }

        let payload = activity_tick_payload(
            id,
            sid,
            bucket_start,
            bucket_end,
            *mouse_events,
            *keyboard_events,
            *confidence,
            *monotonic_elapsed_ns,
            wall_clock_at_tick,
        );
        let computed = compute_hash(prev_hash, &payload);

        if &computed != record_hash {
            return Ok(ChainValidationResult {
                valid: false,
                broken_at_id: Some(id.clone()),
                total_records: rows.len(),
            });
        }

        expected_prev = record_hash.clone();
    }

    Ok(ChainValidationResult {
        valid: true,
        broken_at_id: None,
        total_records: rows.len(),
    })
}

pub fn mark_session_suspicious(conn: &Connection, session_id: &str) -> AgentResult<()> {
    conn.execute(
        "UPDATE sessions SET status = 'suspicious' WHERE id = ?1",
        params![session_id],
    )?;
    Ok(())
}

use rusqlite::OptionalExtension;

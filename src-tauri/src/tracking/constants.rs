pub const SCREENSHOT_BASE_INTERVAL_SECS: u64 = 300;
pub const SCREENSHOT_JITTER_FACTOR: f64 = 0.4;
pub const APP_FOCUS_POLL_SECS: u64 = 15;
pub const MIN_SCREENSHOT_INTERVAL_SECS: u64 = 10;
pub const WORKER_JOIN_TIMEOUT_SECS: u64 = 5;

pub const SETTING_SCREENSHOT_INTERVAL_SECS: &str = "screenshot_interval_secs";

use rusqlite::OptionalExtension;

pub fn screenshot_interval_from_env() -> Option<u64> {
    std::env::var("SCREENSHOT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&secs| secs >= MIN_SCREENSHOT_INTERVAL_SECS)
}

#[allow(dead_code)]
pub fn screenshot_interval_source_label() -> &'static str {
    if screenshot_interval_from_env().is_some() {
        "SCREENSHOT_INTERVAL_SECS (.env)"
    } else {
        "settings (SQLite)"
    }
}

pub fn load_screenshot_interval_secs(conn: &rusqlite::Connection) -> u64 {
    if let Some(secs) = screenshot_interval_from_env() {
        return secs;
    }

    if let Ok(Some(value)) = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING_SCREENSHOT_INTERVAL_SECS],
            |row| row.get::<_, String>(0),
        )
        .optional()
    {
        if let Ok(secs) = value.parse::<u64>() {
            if secs >= MIN_SCREENSHOT_INTERVAL_SECS {
                return secs;
            }
        }
    }

    SCREENSHOT_BASE_INTERVAL_SECS
}

/// Returns a randomized screenshot interval around the base.
/// Jitter = base ± (base * JITTER_FACTOR), clamped to MIN_SCREENSHOT_INTERVAL_SECS.
/// Uses a simple pseudo-random from SystemTime (no extra crate dependency).
pub fn load_randomized_screenshot_interval(conn: &rusqlite::Connection) -> u64 {
    let base = load_screenshot_interval_secs(conn) as f64;
    let jitter_range = base * SCREENSHOT_JITTER_FACTOR;
    let half = jitter_range / 2.0;
    let min_val = (base - half).max(MIN_SCREENSHOT_INTERVAL_SECS as f64);
    let max_val = base + half;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let simple_rand = ((nanos as f64) * 0.6180339887).fract();
    let interval = min_val + simple_rand * (max_val - min_val);
    interval.round() as u64
}

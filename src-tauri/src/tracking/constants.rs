pub const SCREENSHOT_BASE_INTERVAL_SECS: u64 = 300;
pub const APP_FOCUS_POLL_SECS: u64 = 15;
pub const MIN_SCREENSHOT_INTERVAL_SECS: u64 = 10;

pub const SETTING_SCREENSHOT_INTERVAL_SECS: &str = "screenshot_interval_secs";

use rusqlite::OptionalExtension;

/// Override de desenvolvimento via `.env` — tem prioridade sobre SQLite.
pub fn screenshot_interval_from_env() -> Option<u64> {
    std::env::var("SCREENSHOT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&secs| secs >= MIN_SCREENSHOT_INTERVAL_SECS)
}

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

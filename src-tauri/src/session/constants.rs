pub const SCREENSHOT_BASE_INTERVAL_SECS: u64 = 300;
pub const SCREENSHOT_JITTER_SECS: u64 = 60;
pub const FIRST_SCREENSHOT_SECS: u64 = 5;
pub const TICK_INTERVAL_SECS: u64 = 60;
pub const FIRST_TICK_SECS: u64 = 5;
pub const APP_FOCUS_POLL_SECS: u64 = 15;

pub fn screenshot_base_interval_secs() -> u64 {
    std::env::var("VOOWORK_SCREENSHOT_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|secs| *secs >= 10)
        .unwrap_or(SCREENSHOT_BASE_INTERVAL_SECS)
}

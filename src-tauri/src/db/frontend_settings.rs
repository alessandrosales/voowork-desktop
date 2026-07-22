use crate::error::{AgentError, AgentResult};
use crate::locale::LOCALE_SETTING_KEY;
use crate::screenshot::{SETTING_BLUR_ENABLED, SETTING_JPEG_QUALITY};
use crate::tracking::SETTING_SCREENSHOT_INTERVAL_SECS;
use crate::tracking_inactivity::{
    SETTING_INACTIVITY_PROFILE, SETTING_INACTIVITY_THRESHOLD_MINUTES,
};
use crate::tray::{
    SETTING_LAST_PROJECT_ID, SETTING_LAST_TASK_ID, SETTING_SELECTED_PROJECT_ID,
    SETTING_SELECTED_TASK_ID,
};
use crate::windows::SETTING_MINI_WIDGET_ENABLED;

const FRONTEND_SETTING_KEYS: &[&str] = &[
    "theme",
    LOCALE_SETTING_KEY,
    SETTING_LAST_PROJECT_ID,
    SETTING_LAST_TASK_ID,
    SETTING_SELECTED_PROJECT_ID,
    SETTING_SELECTED_TASK_ID,
    SETTING_BLUR_ENABLED,
    SETTING_JPEG_QUALITY,
    SETTING_INACTIVITY_THRESHOLD_MINUTES,
    SETTING_INACTIVITY_PROFILE,
    SETTING_SCREENSHOT_INTERVAL_SECS,
    SETTING_MINI_WIDGET_ENABLED,
];

pub fn ensure_frontend_setting_key(key: &str) -> AgentResult<()> {
    if FRONTEND_SETTING_KEYS.contains(&key) {
        return Ok(());
    }

    Err(AgentError::Other(format!(
        "setting key not allowed from frontend: {key}"
    )))
}

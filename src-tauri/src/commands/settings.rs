use crate::app_state::AppState;
use crate::error::AgentResult;
use crate::models::{IdleConfig, TrackingCapabilities, TrackingConfig};

#[tauri::command]
pub fn get_setting(state: tauri::State<'_, AppState>, key: String) -> AgentResult<Option<String>> {
    let db = state.db.lock();
    db.get_setting(&key)
}

#[tauri::command]
pub fn set_setting(
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> AgentResult<()> {
    let apply_blur = key == "screenshot_blur_enabled";
    {
        let db = state.db.lock();
        db.set_setting(&key, &value)?;
    }
    if apply_blur {
        state
            .session_manager
            .set_screenshot_blur(value == "true" || value == "1");
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn open_data_directory(state: tauri::State<'_, AppState>) -> AgentResult<String> {
    let db = state.db.lock();
    Ok(db.path().parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default())
}

#[tauri::command]
pub fn get_tracking_config(state: tauri::State<'_, AppState>) -> AgentResult<TrackingConfig> {
    let db = state.db.lock();
    let threshold_minutes = crate::idle::load_idle_threshold_minutes(db.conn());
    let profile = db
        .get_setting(crate::idle::SETTING_PROFILE)?
        .unwrap_or_else(|| "standard".into());
    Ok(TrackingConfig {
        activity_tick_interval_secs: crate::session::TICK_INTERVAL_SECS,
        first_activity_tick_secs: crate::session::FIRST_TICK_SECS,
        first_screenshot_secs: crate::session::FIRST_SCREENSHOT_SECS,
        screenshot_base_interval_secs: crate::session::screenshot_base_interval_secs(),
        screenshot_jitter_secs: crate::session::SCREENSHOT_JITTER_SECS,
        app_focus_poll_interval_secs: crate::session::APP_FOCUS_POLL_SECS,
        idle: IdleConfig {
            threshold_minutes,
            profile,
            countdown_secs: crate::idle::COUNTDOWN_SECS,
        },
    })
}

#[tauri::command]
pub fn get_tracking_capabilities() -> TrackingCapabilities {
    crate::permissions::probe_tracking_capabilities()
}

use crate::activity::TrackerMode;
use crate::app_state::AppState;
use crate::db::frontend_settings::ensure_frontend_setting_key;
use crate::error::{AgentError, AgentResult};
use crate::locale::LOCALE_SETTING_KEY;
use crate::models::{TrackingInactivityConfig, TrackingCapabilities, TrackingConfig};
use crate::screenshot::{normalize_jpeg_quality, SETTING_BLUR_ENABLED, SETTING_JPEG_QUALITY};
use crate::tracking_focus::capture_active_window;
use crate::tracking_inactivity::{
    load_inactivity_threshold_minutes, COUNTDOWN_SECS, SETTING_INACTIVITY_PROFILE,
};
use crate::tray::{
    refresh_tray_menu, schedule_tray_refresh, SETTING_SELECTED_PROJECT_ID, SETTING_SELECTED_TASK_ID,
};
use crate::windows::{self, SETTING_MINI_WIDGET_ENABLED};

/// Perfis de inatividade válidos.
pub(crate) const VALID_INACTIVITY_PROFILES: &[&str] = &[
    "standard",
    "data_entry",
    "knowledge",
    "meeting_heavy",
];

#[tauri::command]
pub fn get_setting(state: tauri::State<'_, AppState>, key: String) -> AgentResult<Option<String>> {
    ensure_frontend_setting_key(&key)?;
    let db = state.db.lock();
    db.get_setting(&key)
}

#[tauri::command]
pub fn set_setting(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> AgentResult<()> {
    ensure_frontend_setting_key(&key)?;
    let apply_blur = key == SETTING_BLUR_ENABLED;
    let apply_jpeg_quality = key == SETTING_JPEG_QUALITY;
    let apply_locale = key == LOCALE_SETTING_KEY;
    let apply_tray_selection =
        key == SETTING_SELECTED_PROJECT_ID || key == SETTING_SELECTED_TASK_ID;
    let apply_mini_widget = key == SETTING_MINI_WIDGET_ENABLED;

    // Validar perfil de inatividade
    if key == SETTING_INACTIVITY_PROFILE
        && !VALID_INACTIVITY_PROFILES.contains(&value.as_str())
    {
        return Err(AgentError::Other(format!(
            "invalid inactivity profile: {value}. Valid values: {}",
            VALID_INACTIVITY_PROFILES.join(", ")
        )));
    }

    {
        let db = state.db.lock();
        db.set_setting(&key, &value)?;
    }
    if apply_blur {
        state
            .tracking_manager
            .set_screenshot_blur(value == "true" || value == "1");
    }
    if apply_jpeg_quality {
        if let Ok(quality) = value.parse::<u8>() {
            state
                .tracking_manager
                .set_screenshot_jpeg_quality(normalize_jpeg_quality(quality));
        }
    }
    if apply_locale {
        if let Err(err) = refresh_tray_menu(&app, &value) {
            log::warn!("failed to refresh tray menu locale: {err}");
        }
    }
    if apply_tray_selection {
        schedule_tray_refresh(app.clone());
    }
    if apply_mini_widget {
        let visible = value == "true" || value == "1";
        if visible {
            // show_mini_timer verifica autenticação internamente via should_show_mini_widget
            if let Err(err) = windows::show_mini_timer(&app) {
                log::warn!("failed to show mini widget: {err}");
            }
        } else {
            windows::hide_mini_timer(&app);
        }
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
    let threshold_minutes = load_inactivity_threshold_minutes(db.conn());
    let profile = db
        .get_setting(SETTING_INACTIVITY_PROFILE)?
        .unwrap_or_else(|| "standard".into());
    Ok(TrackingConfig {
        screenshot_interval_secs: crate::tracking::load_screenshot_interval_secs(db.conn()),
        active_window_poll_interval_secs: crate::tracking::APP_FOCUS_POLL_SECS,
        inactivity: TrackingInactivityConfig {
            threshold_minutes,
            profile,
            countdown_secs: COUNTDOWN_SECS,
        },
    })
}

#[tauri::command]
pub fn get_tracking_capabilities(state: tauri::State<'_, AppState>) -> TrackingCapabilities {
    let tracker_mode = state.tracking_manager.tracker.lock().mode();
    let input_granted = matches!(tracker_mode, TrackerMode::Hardware);
    let input_label = if input_granted {
        "Captura de mouse/teclado em tempo real".into()
    } else {
        "Heartbeat — apenas threshold de inatividade".into()
    };

    let window_tracking_works = capture_active_window().is_some();
    let window_label = if window_tracking_works {
        "Captura de janela ativa".into()
    } else {
        "Indisponível (Wayland sem portal ou permissão negada)".into()
    };

    TrackingCapabilities {
        input_capture: crate::models::PermissionCheck {
            granted: input_granted,
            label: input_label,
            action: None,
        },
        window_tracking: crate::models::PermissionCheck {
            granted: window_tracking_works,
            label: window_label,
            action: None,
        },
        screenshots: crate::models::PermissionCheck {
            granted: true,
            label: "Captura de tela (xcap)".into(),
            action: None,
        },
        notes: vec![],
    }
}

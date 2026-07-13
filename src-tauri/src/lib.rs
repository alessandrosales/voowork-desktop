mod activity;
mod app_focus;
mod app_state;
mod auth;
mod clock;
mod commands;
mod crypto;
mod db;
mod env;
mod error;
mod idle;
mod integrity;
mod models;
mod navigation;
mod permissions;
mod screenshot;
mod seed;
mod session;
mod projects;
mod sync;
mod tray;

use app_state::AppState;
use auth::{get_auth_state, login, logout, validate_auth_session};
use commands::{
    classify_idle_period, confirm_manual_work, confirm_still_working, dismiss_manual_work_check,
    get_activity_chart, get_app_status, get_app_version, get_dashboard_summary, get_idle_config,
    get_session_status, get_setting, get_tracking_capabilities, get_tracking_config,
    list_activity_ticks, list_app_focus, list_projects, list_recent_sessions, list_screenshots,
    list_sessions, list_sync_queue, open_data_directory, pause_session, resume_session,
    set_setting, skip_idle_classification, start_session, stop_session, sync_projects,
};
use crypto::DeviceKeys;
use db::Database;
use navigation::external_navigation_plugin;
use screenshot::ScreenshotCapture;
use tauri::webview::PageLoadEvent;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};
use tray::setup_tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env::load();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("reqwest", log::LevelFilter::Warn)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(external_navigation_plugin())
        .setup(|app| {
            let app_data_dir = dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("voowork-desktop");

            let db = Database::open(app_data_dir.clone())?;
            let device_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "voowork-device".into());
            let device_keys = DeviceKeys::ensure(db.conn(), &device_name)?;

            if db.get_setting("theme")?.is_none() {
                db.set_setting("theme", "dark")?;
            }

            if db.get_setting("locale")?.is_none() {
                db.set_setting("locale", "pt-BR")?;
            }

            if db.get_setting(idle::SETTING_THRESHOLD_MINUTES)?.is_none() {
                db.set_setting(
                    idle::SETTING_THRESHOLD_MINUTES,
                    &idle::DEFAULT_IDLE_THRESHOLD_MINUTES.to_string(),
                )?;
            }

            seed::ensure_demo_data(&db)?;

            let screenshot_dir = app_data_dir.join("screenshots");
            let mut screenshot = ScreenshotCapture::new(screenshot_dir)?;
            let blur_enabled = db
                .get_setting("screenshot_blur_enabled")?
                .is_some_and(|v| v == "true" || v == "1");
            screenshot.set_blur(blur_enabled);

            let api_base_url = auth::configured_api_base_url();
            log::info!("Voowork API: {api_base_url}");

            let state = AppState::new(db, device_keys, screenshot, app.handle().clone());
            if let Ok(count) = state.session_manager.recover_orphaned_sessions() {
                if count > 0 {
                    log::warn!("recovered {count} orphaned session(s) from previous run");
                }
            }
            state
                .session_manager
                .set_app_handle(app.handle().clone());
            app.manage(state);

            setup_tray(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_session,
            stop_session,
            pause_session,
            resume_session,
            get_session_status,
            confirm_still_working,
            confirm_manual_work,
            dismiss_manual_work_check,
            classify_idle_period,
            skip_idle_classification,
            get_idle_config,
            get_app_status,
            get_setting,
            set_setting,
            list_projects,
            sync_projects,
            login,
            logout,
            get_auth_state,
            validate_auth_session,
            get_dashboard_summary,
            get_activity_chart,
            list_recent_sessions,
            list_sessions,
            list_activity_ticks,
            list_screenshots,
            list_sync_queue,
            get_app_version,
            open_data_directory,
            list_app_focus,
            get_tracking_config,
            get_tracking_capabilities,
        ])
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                log::info!("main webview finished loading");
                let _ = webview.window().show();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

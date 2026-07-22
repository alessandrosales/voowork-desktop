mod activity;
mod tracking_focus;
mod app_state;
mod auth;
mod commands;
mod crypto;
mod db;
mod env;
mod error;
mod icons;
mod tracking_inactivity;
mod locale;
mod models;
mod navigation;
mod screenshot;
mod projects;
mod sync;
mod trackings;
mod tracking;
mod tray;
mod windows;

use app_state::AppState;
use auth::{get_auth_state, login, logout, validate_auth_session};
use commands::{
    check_active_window_permission, check_input_monitoring_permission,
    classify_paused_inactivity_period, classify_tracking_inactivity_period, confirm_manual_work,
    confirm_still_working, dismiss_activity_buffer, dismiss_inactivity_period, dismiss_manual_work_check,
    get_activity_chart, get_app_status, get_app_version, get_dashboard_summary,
    get_platform_info, get_tracking_inactivity_config, get_tracking_screenshot_image, get_setting,
    get_task_elapsed_seconds, get_tracking_capabilities, get_tracking_config, get_tracking_status,
    list_tracking_inactivity_periods, list_tracking_peripheral_events, list_projects,
    list_sync_queue, list_tracking_apps, list_tracking_screenshots, list_tracking_sites,
    list_trackings, open_data_directory, open_external_url,
    open_system_settings_input_monitoring, open_system_settings_screen_recording, open_web_panel,
    pause_tracking, resume_tracking, restart_tracking, set_setting,
    skip_tracking_inactivity_classification, start_tracking, stop_tracking, sync_projects,
};
use crypto::DeviceKeys;
use db::Database;
use navigation::external_navigation_plugin;
use screenshot::ScreenshotCapture;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent};
use tauri_plugin_log::{Target, TargetKind};
use tray::{handle_tray_menu_event, setup_tray_from_state, spawn_refresh_loop};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use windows::{
    begin_mini_widget_drag, open_main_window, reset_mini_widget_position, setup_windows,
};

use crate::tracking_inactivity::{
    DEFAULT_INACTIVITY_THRESHOLD_MINUTES, SETTING_INACTIVITY_THRESHOLD_MINUTES,
};
use crate::tracking::{SCREENSHOT_BASE_INTERVAL_SECS, SETTING_SCREENSHOT_INTERVAL_SECS};
use crate::screenshot::{DEFAULT_JPEG_QUALITY, SETTING_JPEG_QUALITY};
use crate::sync::SYNC_FLUSH_TIMEOUT_SECS;

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
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            let app_data_dir = dirs::data_dir()
                .or_else(|| dirs::home_dir().map(|p| p.join(".local").join("share")))
                .unwrap_or_else(|| std::path::PathBuf::from("/var/lib/voowork-desktop"))
                .join("voowork-desktop");

            let db = Database::open(app_data_dir.clone())?;
            let device_name = std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .or_else(|_| std::env::var("hostname"))
                .unwrap_or_else(|_| "voowork-device".into());
            DeviceKeys::ensure(db.conn(), &device_name)?;

            if db.get_setting("theme")?.is_none() {
                db.set_setting("theme", "dark")?;
            }
            if db.get_setting("locale")?.is_none() {
                db.set_setting("locale", locale::detect_system_locale())?;
            }
            if db.get_setting(SETTING_INACTIVITY_THRESHOLD_MINUTES)?.is_none() {
                db.set_setting(
                    SETTING_INACTIVITY_THRESHOLD_MINUTES,
                    &DEFAULT_INACTIVITY_THRESHOLD_MINUTES.to_string(),
                )?;
            }
            if db.get_setting(SETTING_SCREENSHOT_INTERVAL_SECS)?.is_none() {
                db.set_setting(SETTING_SCREENSHOT_INTERVAL_SECS, &SCREENSHOT_BASE_INTERVAL_SECS.to_string())?;
            }
            if db.get_setting(windows::SETTING_MINI_WIDGET_ENABLED)?.is_none() {
                db.set_setting(windows::SETTING_MINI_WIDGET_ENABLED, "true")?;
            }

            let screenshot_dir = app_data_dir.join("screenshots");
            let mut screenshot = ScreenshotCapture::new(screenshot_dir)?;
            let jpeg_quality = db
                .get_setting(SETTING_JPEG_QUALITY)?
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(DEFAULT_JPEG_QUALITY);
            screenshot.set_jpeg_quality(jpeg_quality);

            let api_base_url = auth::configured_api_base_url();
            log::info!("Voowork API: {api_base_url}");

            let state = AppState::new(db, screenshot, app.handle().clone());
            match state.tracking_manager.initialize_session() {
                Ok(count) => {
                    if count > 0 {
                        log::warn!("discarded {count} orphaned tracking(s) from previous run");
                    }
                }
                Err(err) => {
                    log::error!("failed to initialize tracking session (orphan finalize): {err}");
                }
            }
            state.tracking_manager.set_app_handle(app.handle().clone());
            let authenticated = {
                let db = state.db.lock();
                auth::read_auth_state(&db)?.is_authenticated
            };
            state
                .tracking_manager
                .set_session_authenticated(authenticated);
            state.tracking_manager.start_background_services();
            app.manage(state);

            setup_tray_from_state(app)?;
            spawn_refresh_loop(app.handle().clone());
            setup_windows(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            } else {
                log::error!("main window not found during setup");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_tracking,
            restart_tracking,
            pause_tracking,
            resume_tracking,
            stop_tracking,
            get_tracking_status,
            get_task_elapsed_seconds,
            dismiss_activity_buffer,
            confirm_still_working,
            confirm_manual_work,
            dismiss_manual_work_check,
            dismiss_inactivity_period,
            classify_tracking_inactivity_period,
            classify_paused_inactivity_period,
            skip_tracking_inactivity_classification,
            get_tracking_inactivity_config,
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
            list_trackings,
            list_tracking_peripheral_events,
            list_tracking_inactivity_periods,
            list_tracking_screenshots,
            get_tracking_screenshot_image,
            list_sync_queue,
            list_tracking_apps,
            list_tracking_sites,
            get_app_version,
            open_data_directory,
            open_web_panel,
            open_external_url,
            open_system_settings_input_monitoring,
            open_system_settings_screen_recording,
            get_tracking_config,
            get_tracking_capabilities,
            get_platform_info,
            check_input_monitoring_permission,
            check_active_window_permission,
            open_main_window,
            begin_mini_widget_drag,
            reset_mini_widget_position,
        ])
        .on_page_load(|webview, payload| {
            if webview.label() != "main" {
                return;
            }

            log::info!(
                "main webview {:?} loading {}",
                payload.event(),
                payload.url()
            );

            static FIRST_LOAD: AtomicBool = AtomicBool::new(true);
            if payload.event() == PageLoadEvent::Started
                && FIRST_LOAD.swap(false, Ordering::AcqRel)
            {
                let window = webview.window();
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Err(err) = state.tracking_manager.shutdown_for_quit() {
                        log::error!("failed to reset tracking on exit: {err}");
                    }

                    state.sync_worker.stop();
                    let sync_worker = state.sync_worker.clone();
                    let db = state.db.clone();
                    let handle = app_handle.clone();
                    thread::spawn(move || {
                        sync_worker.flush_blocking(db, handle, SYNC_FLUSH_TIMEOUT_SECS);
                    });
                }
            }
        });
}

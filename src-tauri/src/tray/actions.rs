use crate::app_state::AppState;
use crate::auth::{perform_logout, KEY_AUTHENTICATED};
use crate::error::AgentResult;
use crate::models::TrackingStatus;
use crate::projects::ensure_can_start_tracking;
use crate::sync::SYNC_FLUSH_TIMEOUT_SECS;
use crate::tracking::prepare_before_start;
use crate::windows::{hide_mini_timer, show_main_window};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use super::refresh::{refresh_tray_ui, refresh_tray_ui_sync, request_shutdown};
use super::state::{
    SETTING_LAST_PROJECT_ID, SETTING_LAST_TASK_ID, SETTING_SELECTED_PROJECT_ID,
    SETTING_SELECTED_TASK_ID,
};
use super::EVENT_AUTH_LOGGED_OUT;

#[cfg(unix)]
fn force_process_exit(code: i32) -> ! {
    unsafe {
        libc::_exit(code);
    }
}

#[cfg(not(unix))]
fn force_process_exit(code: i32) -> ! {
    std::process::exit(code as u32);
}

pub fn handle_toggle_tracking(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        show_main_window(app);
        return;
    };

    if !is_authenticated(&state) {
        show_main_window(app);
        return;
    }

    let tracking = state.tracking_manager.status();
    if tracking.active && needs_inactivity_ui(&tracking.inactivity.phase) {
        show_main_window(app);
        let _ = refresh_tray_ui_sync(app);
        return;
    }

    let app_state = state.inner().clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = if tracking.active {
            toggle_active_session(&app_state, &tracking)
        } else {
            start_last_session(&app_state, &app_handle)
        };

        if let Err(err) = result {
            log::warn!("tray toggle tracking failed: {err}");
            show_main_window(&app_handle);
        }

        let _ = refresh_tray_ui(&app_handle);
    });
}

pub fn handle_tray_quit(app: &AppHandle) {
    log::info!("tray quit requested");
    request_shutdown();

    if let Some(state) = app.try_state::<AppState>() {
        let tracking_manager = Arc::clone(&state.tracking_manager);
        let sync_worker = Arc::clone(&state.sync_worker);
        let db = Arc::clone(&state.db);
        let app_handle = app.clone();

        std::thread::spawn(move || {

            tracking_manager.capture_final_screenshot_and_finalize();

            {
                let db_guard = db.lock();
                let _ = db_guard.set_setting(SETTING_SELECTED_PROJECT_ID, "");
                let _ = db_guard.set_setting(SETTING_SELECTED_TASK_ID, "");
                if let Err(err) = db_guard.clear_task_time_totals() {
                    log::warn!("failed to clear task time totals on quit: {err}");
                }
            }

            tracking_manager.prepare_immediate_exit();

            sync_worker.stop();

            sync_worker.flush_blocking(db, app_handle, SYNC_FLUSH_TIMEOUT_SECS);

            std::thread::sleep(std::time::Duration::from_millis(300));
            force_process_exit(0);
        });
        return;
    }

    log::warn!("tray quit: AppState not available, exiting without sync flush");
    let _ = app;
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        force_process_exit(0);
    });
}

pub fn handle_tray_menu_event(app: &AppHandle, event_id: &str) {
    match event_id {
        "show" => show_main_window(app),
        "reset_widget_position" => {
            if let Err(err) = crate::windows::reset_mini_to_default(app) {
                log::warn!("reset mini widget position failed: {err}");
            }
        }
        "toggle_tracking" => handle_toggle_tracking(app),
        "logout" => handle_tray_logout(app),
        "quit" => handle_tray_quit(app),
        other => log::debug!("unhandled tray menu event: {other}"),
    }
}

pub fn handle_tray_logout(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        show_main_window(app);
        return;
    };

    let app_state = state.inner().clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if app_state.tracking_manager.status().active {
            if let Err(err) = app_state.tracking_manager.stop_tracking() {
                log::error!("failed to stop tracking before tray logout: {err}");
            }
        }
        if let Err(err) = perform_logout(&app_state) {
            log::error!("tray logout failed: {err}");
        } else {
            let _ = app_handle.emit(EVENT_AUTH_LOGGED_OUT, ());
        }

        hide_mini_timer(&app_handle);
        show_main_window(&app_handle);
        let _ = refresh_tray_ui(&app_handle);
    });
}

fn is_authenticated(state: &AppState) -> bool {
    let db = state.db.lock();
    db.get_setting(KEY_AUTHENTICATED)
        .ok()
        .flatten()
        .is_some_and(|value| value == "true")
}

fn toggle_active_session(state: &AppState, tracking: &TrackingStatus) -> AgentResult<()> {
    let phase = tracking.inactivity.phase.as_str();
    if phase == "manual_paused" {
        state.tracking_manager.resume_tracking()
    } else {
        state.tracking_manager.pause_tracking()
    }
}

fn start_last_session(state: &AppState, app: &AppHandle) -> AgentResult<()> {
    let (project_id, task_id) = {
        let db = state.db.lock();
        selected_selection(&db)?
    };

    let (Some(project_id), Some(task_id)) = (project_id, task_id) else {
        return Err(crate::error::AgentError::Session(
            "no last project/task selected".into(),
        ));
    };

    {
        let db = state.db.lock();
        ensure_can_start_tracking(&db, &project_id)?;
        crate::projects::ensure_task_belongs_to_project(&db, &project_id, &task_id)?;
    }

    tauri::async_runtime::block_on(prepare_before_start(app, state))?;

    state
        .tracking_manager
        .start_tracking(project_id, task_id)?;
    Ok(())
}

fn needs_inactivity_ui(phase: &str) -> bool {
    matches!(
        phase,
        "warning" | "countdown" | "paused_inactivity" | "resume_prompt" | "manual_work_check"
    )
}

pub fn persist_last_selection(
    db: &crate::db::Database,
    project_id: &str,
    task_id: &str,
) {
    let _ = db.set_setting(SETTING_LAST_PROJECT_ID, project_id);
    let _ = db.set_setting(SETTING_LAST_TASK_ID, task_id);
    let _ = db.set_setting(SETTING_SELECTED_PROJECT_ID, project_id);
    let _ = db.set_setting(SETTING_SELECTED_TASK_ID, task_id);
}

pub fn selected_selection(
    db: &crate::db::Database,
) -> AgentResult<(Option<String>, Option<String>)> {
    let project_id = db
        .get_setting(SETTING_SELECTED_PROJECT_ID)?
        .filter(|value| !value.is_empty());
    let task_id = db
        .get_setting(SETTING_SELECTED_TASK_ID)?
        .filter(|value| !value.is_empty());
    Ok((project_id, task_id))
}

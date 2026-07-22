use crate::app_state::AppState;
use crate::auth::{perform_logout, KEY_AUTHENTICATED};
use crate::error::AgentResult;
use crate::models::TrackingStatus;
use crate::projects::ensure_can_start_tracking;
use crate::sync::SYNC_FLUSH_TIMEOUT_SECS;
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

    // Snapshot rápido: `status()` não toma `state_transition` — seguro na
    // main thread. Já start/pause/resume tomam `state_transition` e podem
    // demorar segundos (finalize com worker join + xcap + DB), então rodam
    // fora da main thread para não congelar a UI (regressão 2026-07-21).
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
            start_last_session(&app_state)
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
        // Tudo fora da main thread: o finalize (worker join + xcap + DB) pode
        // levar segundos e congelaria a UI durante o quit ("forçar saída").
        // Ordem preservada (A8): finalize → prepare exit → stop sync → flush.
        std::thread::spawn(move || {
            // 1. Capture final screenshot + finalize tracking BEFORE killing the
            //    process. This ensures the last work period is not lost.
            tracking_manager.capture_final_screenshot_and_finalize();

            // 2. Then signal immediate exit (drops worker handle, flags).
            tracking_manager.prepare_immediate_exit();

            // 3. Stop sync worker's background polling so it doesn't race with flush.
            sync_worker.stop();

            // 4. Flush pending sync items to backend, then exit.
            sync_worker.flush_blocking(db, app_handle, SYNC_FLUSH_TIMEOUT_SECS);
            // Brief delay for SQLite WAL checkpoint before _exit.
            std::thread::sleep(std::time::Duration::from_millis(300));
            force_process_exit(0);
        });
        return;
    }

    // Fallback: AppState not available (should not happen, but be safe)
    log::warn!("tray quit: AppState not available, exiting without sync flush");
    let _ = app;
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        force_process_exit(0);
    });
}

pub fn handle_tray_stop(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    // Fora da main thread: `stop_tracking()` segura `state_transition` durante
    // todo o finalize (worker join + xcap + DB) — na main thread isso
    // congelava a UI por segundos (regressão 2026-07-21).
    let tracking_manager = Arc::clone(&state.tracking_manager);
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if !tracking_manager.status().active {
            return;
        }
        if let Err(err) = tracking_manager.stop_tracking() {
            log::error!("tray stop tracking failed: {err}");
        }
        let _ = refresh_tray_ui(&app_handle);
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
        "stop" => handle_tray_stop(app),
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

    // Fora da main thread: stop segura `state_transition` (finalize longo) e
    // `perform_logout` toca o keyring (até ~2s) — ambos congelariam a UI se
    // rodassem no handler do tray (main thread). As operações de janela via
    // AppHandle são despachadas ao event loop (mesmo padrão do comando
    // `logout` async, que já as executa fora da main thread).
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

fn start_last_session(state: &AppState) -> AgentResult<()> {
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

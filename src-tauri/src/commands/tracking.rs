use crate::activity::tracker_mode_label;
use crate::app_state::AppState;
use crate::error::{AgentError, AgentResult};
use crate::models::{
    AppStatus, ClassifyTrackingInactivityRequest, StartTrackingRequest, StartTrackingResponse,
    TrackingInactivityConfig, TrackingStatus,
};
use crate::tray::refresh_tray_ui;

use crate::tracking_inactivity::{
    load_inactivity_threshold_minutes, COUNTDOWN_SECS, SETTING_INACTIVITY_PROFILE,
};

#[tauri::command]
pub async fn start_tracking(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: StartTrackingRequest,
) -> AgentResult<StartTrackingResponse> {
    {
        let db = state.db.lock();
        crate::projects::ensure_can_start_tracking(&db, &request.project_id)?;
        crate::projects::ensure_task_belongs_to_project(
            &db,
            &request.project_id,
            &request.task_id,
        )?;
    }

    if request.task_id.trim().is_empty() {
        return Err(crate::error::AgentError::Session(
            "task is required".into(),
        ));
    }

    let app_state = state.inner().clone();
    let project_id = request.project_id;
    let task_id = request.task_id;

    let response = tauri::async_runtime::spawn_blocking(move || -> AgentResult<StartTrackingResponse> {
        let tracking = app_state
            .tracking_manager
            .start_tracking(project_id, task_id)?;
        Ok(StartTrackingResponse {
            tracking_id: tracking.tracking_id,
            started_at: tracking.started_at,
        })
    })
    .await
    .map_err(|err| AgentError::Other(format!("start tracking worker failed: {err}")))??;

    let _ = refresh_tray_ui(&app);
    Ok(response)
}

#[tauri::command]
pub async fn restart_tracking(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: StartTrackingRequest,
) -> AgentResult<StartTrackingResponse> {
    {
        let db = state.db.lock();
        crate::projects::ensure_can_start_tracking(&db, &request.project_id)?;
        crate::projects::ensure_task_belongs_to_project(
            &db,
            &request.project_id,
            &request.task_id,
        )?;
    }

    if request.task_id.trim().is_empty() {
        return Err(crate::error::AgentError::Session(
            "task is required".into(),
        ));
    }

    let app_state = state.inner().clone();
    let project_id = request.project_id;
    let task_id = request.task_id;

    let response = tauri::async_runtime::spawn_blocking(move || -> AgentResult<StartTrackingResponse> {
        let tracking = app_state
            .tracking_manager
            .restart_tracking(project_id, task_id)?;
        Ok(StartTrackingResponse {
            tracking_id: tracking.tracking_id,
            started_at: tracking.started_at,
        })
    })
    .await
    .map_err(|err| AgentError::Other(format!("restart tracking worker failed: {err}")))??;

    let _ = refresh_tray_ui(&app);
    Ok(response)
}

#[tauri::command]
pub fn dismiss_activity_buffer(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.dismiss_activity_buffer();
    Ok(())
}

#[tauri::command]
pub fn get_task_elapsed_seconds(
    state: tauri::State<'_, AppState>,
    task_id: String,
) -> AgentResult<u64> {
    state.tracking_manager.task_elapsed_seconds(&task_id)
}

#[tauri::command]
pub async fn pause_tracking(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AgentResult<()> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app_state.tracking_manager.pause_tracking())
        .await
        .map_err(|err| AgentError::Other(format!("pause tracking worker failed: {err}")))??;
    let _ = refresh_tray_ui(&app);
    Ok(())
}

#[tauri::command]
pub async fn resume_tracking(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AgentResult<()> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app_state.tracking_manager.resume_tracking())
        .await
        .map_err(|err| AgentError::Other(format!("resume tracking worker failed: {err}")))??;
    let _ = refresh_tray_ui(&app);
    Ok(())
}

#[tauri::command]
pub async fn stop_tracking(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AgentResult<()> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app_state.tracking_manager.stop_tracking())
        .await
        .map_err(|err| AgentError::Other(format!("stop tracking worker failed: {err}")))??;
    let _ = refresh_tray_ui(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_tracking_status(state: tauri::State<'_, AppState>) -> AgentResult<TrackingStatus> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || Ok(app_state.tracking_manager.status()))
        .await
        .map_err(|err| AgentError::Other(format!("tracking status worker failed: {err}")))?
}

#[tauri::command]
pub fn get_app_status(state: tauri::State<'_, AppState>) -> AgentResult<AppStatus> {
    let tracking = state.tracking_manager.status();
    let (sync_pending, sync_failed, sync_confirmed) = {
        let db = state.db.lock();
        db.sync_queue_stats()?
    };
    let device_registered = {
        let db = state.db.lock();
        db.device_is_registered()?
    };

    let tracker_mode = tracker_mode_label(state.tracking_manager.tracker_mode()).to_string();

    Ok(AppStatus {
        tracking,
        sync_pending,
        sync_failed,
        sync_confirmed,
        device_registered,
        tracker_mode: tracker_mode.to_string(),
    })
}

#[tauri::command]
pub fn confirm_still_working(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.confirm_still_working()
}

#[tauri::command]
pub fn classify_tracking_inactivity_period(
    state: tauri::State<'_, AppState>,
    request: ClassifyTrackingInactivityRequest,
) -> AgentResult<()> {
    state
        .tracking_manager
        .classify_tracking_inactivity_period(&request.period_id, &request.category)
}

#[tauri::command]
pub fn skip_tracking_inactivity_classification(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.skip_tracking_inactivity_classification()
}

#[tauri::command]
pub fn dismiss_inactivity_period(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.dismiss_inactivity_period()
}

#[tauri::command]
pub fn classify_paused_inactivity_period(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.classify_paused_inactivity_period()
}

#[tauri::command]
pub fn confirm_manual_work(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.confirm_manual_work()
}

#[tauri::command]
pub fn dismiss_manual_work_check(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.tracking_manager.dismiss_manual_work_check()
}

/// Verifica se o app tem permissão de Monitoramento de Entrada no macOS.
/// Usado pelo tracker de atividade baseado em polling (CoreGraphics).
#[tauri::command]
pub fn check_input_monitoring_permission(state: tauri::State<'_, AppState>) -> bool {
    state.tracking_manager.tracker_has_permission()
}

/// Verifica se o app consegue capturar a janela ativa (necessário para
/// detectar meetings e trackear apps).
///
/// - Linux / Windows: sempre `true`.
/// - macOS: `true` apenas se o usuário concedeu permissão de Screen Recording.
#[tauri::command]
pub fn check_active_window_permission() -> bool {
    crate::tracking_focus::check_active_window_permission()
}

#[tauri::command]
pub fn get_tracking_inactivity_config(state: tauri::State<'_, AppState>) -> AgentResult<TrackingInactivityConfig> {
    let db = state.db.lock();
    let threshold_minutes = load_inactivity_threshold_minutes(db.conn());
    let profile = db
        .get_setting(SETTING_INACTIVITY_PROFILE)?
        .unwrap_or_else(|| "standard".into());
    Ok(TrackingInactivityConfig {
        threshold_minutes,
        profile,
        countdown_secs: COUNTDOWN_SECS,
    })
}

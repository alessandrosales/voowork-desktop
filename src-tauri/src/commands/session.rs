use crate::app_state::AppState;
use crate::error::AgentResult;
use crate::models::{
    AppStatus, ClassifyIdleRequest, IdleConfig, SessionStatus, StartSessionRequest,
    StartSessionResponse,
};

#[tauri::command]
pub fn start_session(
    state: tauri::State<'_, AppState>,
    request: StartSessionRequest,
) -> AgentResult<StartSessionResponse> {
    {
        let db = state.db.lock();
        crate::projects::ensure_can_start_session(&db, &request.project_id)?;
    }

    let session = state
        .session_manager
        .start_session(request.project_id, request.task_id)?;

    Ok(StartSessionResponse {
        session_id: session.session_id,
        started_at: session.started_at,
    })
}

#[tauri::command]
pub fn stop_session(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.stop_session()
}

#[tauri::command]
pub fn get_session_status(state: tauri::State<'_, AppState>) -> AgentResult<SessionStatus> {
    Ok(state.session_manager.status())
}

#[tauri::command]
pub fn get_app_status(state: tauri::State<'_, AppState>) -> AgentResult<AppStatus> {
    let session = state.session_manager.status();
    let (sync_pending, sync_failed, sync_confirmed) = {
        let db = state.db.lock();
        db.sync_queue_stats()?
    };
    let device_registered = {
        let db = state.db.lock();
        db.device_is_registered()?
    };

    let tracker_mode = match state.session_manager.tracker_mode() {
        crate::activity::TrackerMode::Hardware => "hardware",
        crate::activity::TrackerMode::Simulated => "simulated",
    };

    Ok(AppStatus {
        session,
        sync_pending,
        sync_failed,
        sync_confirmed,
        device_registered,
        tracker_mode: tracker_mode.to_string(),
    })
}

#[tauri::command]
pub fn pause_session(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.pause_session()
}

#[tauri::command]
pub fn resume_session(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.resume_session()
}

#[tauri::command]
pub fn confirm_still_working(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.confirm_still_working()
}

#[tauri::command]
pub fn classify_idle_period(
    state: tauri::State<'_, AppState>,
    request: ClassifyIdleRequest,
) -> AgentResult<()> {
    state
        .session_manager
        .classify_idle_period(&request.period_id, &request.category)
}

#[tauri::command]
pub fn skip_idle_classification(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.skip_idle_classification()
}

#[tauri::command]
pub fn confirm_manual_work(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.confirm_manual_work()
}

#[tauri::command]
pub fn dismiss_manual_work_check(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    state.session_manager.dismiss_manual_work_check()
}

#[tauri::command]
pub fn get_idle_config(state: tauri::State<'_, AppState>) -> AgentResult<IdleConfig> {
    let db = state.db.lock();
    let threshold_minutes = crate::idle::load_idle_threshold_minutes(db.conn());
    let profile = db
        .get_setting(crate::idle::SETTING_PROFILE)?
        .unwrap_or_else(|| "standard".into());
    Ok(IdleConfig {
        threshold_minutes,
        profile,
        countdown_secs: crate::idle::COUNTDOWN_SECS,
    })
}

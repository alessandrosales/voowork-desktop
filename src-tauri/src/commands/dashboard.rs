use crate::app_state::AppState;
use crate::error::AgentResult;
use crate::models::{
    ActivityChartPoint, ActivityTickRow, AppFocusRow, DashboardSummary, ScreenshotRow,
    SessionRow, SyncQueueRow,
};

#[tauri::command]
pub fn get_dashboard_summary(
    state: tauri::State<'_, AppState>,
) -> AgentResult<DashboardSummary> {
    let db = state.db.lock();
    db.dashboard_summary()
}

#[tauri::command]
pub fn get_activity_chart(
    state: tauri::State<'_, AppState>,
    range: String,
) -> AgentResult<Vec<ActivityChartPoint>> {
    let db = state.db.lock();
    let normalized = if range == "7d" { "7d" } else { "today" };
    db.activity_chart(normalized)
}

#[tauri::command]
pub fn list_recent_sessions(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> AgentResult<Vec<SessionRow>> {
    let db = state.db.lock();
    db.list_recent_sessions(limit.unwrap_or(20))
}

#[tauri::command]
pub fn list_sessions(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<SessionRow>> {
    let db = state.db.lock();
    db.list_sessions(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_activity_ticks(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<ActivityTickRow>> {
    let db = state.db.lock();
    db.list_activity_ticks(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_screenshots(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<ScreenshotRow>> {
    let db = state.db.lock();
    db.list_screenshots(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_sync_queue(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<SyncQueueRow>> {
    let db = state.db.lock();
    db.list_sync_queue(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_app_focus(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    session_id: Option<String>,
) -> AgentResult<Vec<AppFocusRow>> {
    let db = state.db.lock();
    db.list_app_focus(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        session_id.as_deref(),
    )
}

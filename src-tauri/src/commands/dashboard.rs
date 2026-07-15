use crate::app_state::AppState;
use crate::auth::read_access_token;
use crate::error::AgentResult;
use crate::models::{
    ActivityChartPoint, DashboardSummary, TrackingInactivityPeriodRow, SyncQueueRow, TrackingAppRow,
    TrackingPeripheralEventRow, TrackingRow, TrackingScreenshotImage, TrackingScreenshotRow,
    TrackingSiteRow,
};
use crate::screenshot::resolve_screenshot_image;

#[tauri::command]
pub fn get_dashboard_summary(state: tauri::State<'_, AppState>) -> AgentResult<DashboardSummary> {
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
pub fn list_trackings(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<TrackingRow>> {
    let db = state.db.lock();
    db.list_trackings(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_tracking_peripheral_events(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<TrackingPeripheralEventRow>> {
    let db = state.db.lock();
    db.list_tracking_peripheral_events(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn list_tracking_screenshots(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AgentResult<Vec<TrackingScreenshotRow>> {
    let db = state.db.lock();
    db.list_tracking_screenshots(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub async fn get_tracking_screenshot_image(
    state: tauri::State<'_, AppState>,
    screenshot_id: String,
) -> AgentResult<TrackingScreenshotImage> {
    let (db_path, access_token, tracking_id, path, synced_at) = {
        let db = state.db.lock();
        let screenshot = db.get_tracking_screenshot_access(&screenshot_id)?;
        let access_token = read_access_token(&db)?
            .ok_or_else(|| crate::error::AgentError::Auth("user not authenticated".into()))?;
        (
            db.path().clone(),
            access_token,
            screenshot.tracking_id,
            screenshot.path,
            screenshot.synced_at,
        )
    };

    resolve_screenshot_image(
        &state.api_base_url,
        &access_token,
        &db_path,
        &screenshot_id,
        &tracking_id,
        &path,
        synced_at.as_deref(),
    )
    .await
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
pub fn list_tracking_apps(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    tracking_id: Option<String>,
) -> AgentResult<Vec<TrackingAppRow>> {
    let db = state.db.lock();
    db.list_tracking_apps(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        tracking_id.as_deref(),
    )
}

#[tauri::command]
pub fn list_tracking_sites(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    tracking_id: Option<String>,
) -> AgentResult<Vec<TrackingSiteRow>> {
    let db = state.db.lock();
    db.list_tracking_sites(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        tracking_id.as_deref(),
    )
}

#[tauri::command]
pub fn list_tracking_inactivity_periods(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> AgentResult<Vec<TrackingInactivityPeriodRow>> {
    let db = state.db.lock();
    db.list_tracking_inactivity_periods(limit.unwrap_or(50))
}

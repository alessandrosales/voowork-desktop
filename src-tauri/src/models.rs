use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingStatus {
    pub active: bool,
    pub tracking_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub started_at: Option<String>,
    pub elapsed_seconds: u64,
    pub inactivity_seconds: u64,
    pub task_accumulated_seconds: u64,
    pub activity_buffer_seconds: u64,
    pub activity_buffer_alert: bool,
    pub mouse_events: u64,
    pub keyboard_events: u64,
    pub clock_skew_detected: bool,
    pub activity_confidence: f64,
    pub activity_score: u8,
    pub tracker_mode: Option<String>,
    pub current_app: Option<String>,
    pub current_window_title: Option<String>,
    pub screenshot_count: u64,
    pub last_screenshot_at: Option<String>,
    pub inactivity: TrackingInactivityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingInactivityStatus {
    pub phase: String,
    pub threshold_secs: u64,
    pub countdown_secs: u64,
    pub countdown_remaining_secs: Option<u64>,
    pub countdown_ends_at: Option<String>,
    pub inactivity_started_at: Option<String>,
    pub paused_at: Option<String>,
    pub away_seconds: Option<u64>,
    pub pending_period_id: Option<String>,
    pub meeting_exempt: bool,
    pub active_seconds: u64,
    pub inactivity_discarded_seconds: u64,
    pub inactivity_reclassified_seconds: u64,
}

impl Default for TrackingInactivityStatus {
    fn default() -> Self {
        Self {
            phase: "active".into(),
            threshold_secs: 120,
            countdown_secs: 60,
            countdown_remaining_secs: None,
            countdown_ends_at: None,
            inactivity_started_at: None,
            paused_at: None,
            away_seconds: None,
            pending_period_id: None,
            meeting_exempt: false,
            active_seconds: 0,
            inactivity_discarded_seconds: 0,
            inactivity_reclassified_seconds: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingInactivityConfig {
    pub threshold_minutes: u64,
    pub profile: String,
    pub countdown_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyTrackingInactivityRequest {
    pub period_id: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub tracking: TrackingStatus,
    pub sync_pending: i64,
    pub sync_failed: i64,
    pub sync_confirmed: i64,
    pub device_registered: bool,
    pub tracker_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTrackingRequest {
    pub project_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTrackingResponse {
    pub tracking_id: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOption {
    pub id: String,
    pub name: String,
    pub tasks: Vec<TaskOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    pub hours_today_seconds: u64,
    pub trackings_today: i64,
    pub avg_activity_confidence: f64,
    pub sync_pending: i64,
    pub sync_confirmed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityChartPoint {
    pub label: String,
    pub mouse: u64,
    pub keyboard: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingRow {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub task_id: String,
    pub task_name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<u64>,
    pub status: String,
    pub sync_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingInactivityPeriodRow {
    pub id: String,
    pub tracking_id: String,
    pub inactivity_started_at: String,
    pub paused_at: Option<String>,
    pub resumed_at: Option<String>,
    pub duration_seconds: u64,
    pub discarded_seconds: u64,
    pub reclassified_seconds: u64,
    pub category: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingPeripheralEventRow {
    pub id: String,
    pub tracking_id: String,
    pub event: String,
    pub count: f64,
    pub screenshot_original_id: Option<String>,
    pub started_at: String,
    pub ended_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingScreenshotRow {
    pub id: String,
    pub tracking_id: String,
    pub original_id: String,
    pub captured_at: String,
    pub path: String,
    pub remote_path: Option<String>,
    pub synced_at: Option<String>,
    pub sync_status: String,
    pub has_local_file: bool,
}

#[derive(Debug, Clone)]
pub struct TrackingScreenshotAccess {
    pub tracking_id: String,
    pub path: String,
    pub synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingScreenshotImage {
    pub source: String,
    pub file_path: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncQueueRow {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub attempts: i64,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingSiteRow {
    pub id: String,
    pub tracking_id: String,
    pub address: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingAppRow {
    pub id: String,
    pub tracking_id: String,
    pub name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingConfig {
    pub screenshot_interval_secs: u64,
    pub active_window_poll_interval_secs: u64,
    pub inactivity: TrackingInactivityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCheck {
    pub granted: bool,
    pub label: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingCapabilities {
    pub input_capture: PermissionCheck,
    pub window_tracking: PermissionCheck,
    pub screenshots: PermissionCheck,
    pub notes: Vec<String>,
}

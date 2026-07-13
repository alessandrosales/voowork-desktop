use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub active: bool,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub started_at: Option<String>,
    pub elapsed_seconds: u64,
    pub mouse_events: u64,
    pub keyboard_events: u64,
    pub clock_skew_detected: bool,
    pub activity_confidence: f64,
    pub tracker_mode: Option<String>,
    pub current_app: Option<String>,
    pub current_window_title: Option<String>,
    pub screenshot_count: u64,
    pub last_screenshot_at: Option<String>,
    pub idle: IdleStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleStatus {
    pub phase: String,
    pub threshold_secs: u64,
    pub countdown_secs: u64,
    pub countdown_remaining_secs: Option<u64>,
    pub countdown_ends_at: Option<String>,
    pub idle_started_at: Option<String>,
    pub paused_at: Option<String>,
    pub away_seconds: Option<u64>,
    pub pending_period_id: Option<String>,
    pub meeting_exempt: bool,
    pub active_seconds: u64,
    pub idle_discarded_seconds: u64,
    pub idle_reclassified_seconds: u64,
}

impl Default for IdleStatus {
    fn default() -> Self {
        Self {
            phase: "active".into(),
            threshold_secs: 120,
            countdown_secs: 60,
            countdown_remaining_secs: None,
            countdown_ends_at: None,
            idle_started_at: None,
            paused_at: None,
            away_seconds: None,
            pending_period_id: None,
            meeting_exempt: false,
            active_seconds: 0,
            idle_discarded_seconds: 0,
            idle_reclassified_seconds: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleConfig {
    pub threshold_minutes: u64,
    pub profile: String,
    pub countdown_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyIdleRequest {
    pub period_id: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub session: SessionStatus,
    pub sync_pending: i64,
    pub sync_failed: i64,
    pub sync_confirmed: i64,
    pub device_registered: bool,
    pub tracker_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionRequest {
    pub project_id: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionResponse {
    pub session_id: String,
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
    pub sessions_today: i64,
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
pub struct SessionRow {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub task_id: Option<String>,
    pub task_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<u64>,
    pub status: String,
    pub sync_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTickRow {
    pub id: String,
    pub session_id: String,
    pub bucket_start: String,
    pub bucket_end: String,
    pub mouse_events: i64,
    pub keyboard_events: i64,
    pub activity_score_confidence: f64,
    pub record_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRow {
    pub id: String,
    pub user_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub session_id: String,
    pub captured_at: String,
    pub sha256_hash: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub blur_applied: bool,
    pub sync_status: String,
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
pub struct AppFocusRow {
    pub id: String,
    pub session_id: String,
    pub app_name: String,
    pub window_title: Option<String>,
    pub process_path: Option<String>,
    pub process_id: Option<i64>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingConfig {
    pub activity_tick_interval_secs: u64,
    pub first_activity_tick_secs: u64,
    pub first_screenshot_secs: u64,
    pub screenshot_base_interval_secs: u64,
    pub screenshot_jitter_secs: u64,
    pub app_focus_poll_interval_secs: u64,
    pub idle: IdleConfig,
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

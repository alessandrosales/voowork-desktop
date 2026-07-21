pub const COUNTDOWN_SECS: u64 = 60;
pub const DEFAULT_INACTIVITY_THRESHOLD_MINUTES: u64 = 2;
pub const SETTING_INACTIVITY_THRESHOLD_MINUTES: &str = "tracking_inactivity_threshold_minutes";
pub const SETTING_INACTIVITY_PROFILE: &str = "tracking_inactivity_profile";
pub(crate) const MANUAL_PAUSE_ACTIVITY_SECS: u64 = 30;
pub(crate) const MANUAL_PAUSE_AUTO_RESUME_SECS: u64 = 300;
pub(crate) const MANUAL_INPUT_GAP_SECS: u64 = 5;
/// Threshold for detecting a suspend/resume cycle via wall-clock jump.
/// If wall-clock delta exceeds monotonic delta by more than this many
/// seconds, the system was suspended and we enter inactivity pause.
pub(crate) const SUSPEND_GAP_THRESHOLD_SECS: u64 = 90;

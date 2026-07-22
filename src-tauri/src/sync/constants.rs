
pub const BACKEND_SYNC_ENABLED: bool = true;

pub const ENTITY_TRACKING: &str = "tracking";
pub const ENTITY_TRACKING_PERIPHERAL_EVENT: &str = "tracking_peripheral_event";
pub const ENTITY_TRACKING_INACTIVITY_PERIOD: &str = "tracking_inactivity_period";
pub const ENTITY_TRACKING_SCREENSHOT: &str = "tracking_screenshot";
pub const ENTITY_TRACKING_APP: &str = "tracking_app";
pub const ENTITY_TRACKING_SITE: &str = "tracking_site";

pub const PENDING_BATCH_SIZE: usize = 10;

pub const MAX_SYNC_ATTEMPTS: i64 = 8;

pub const HTTP_TIMEOUT_SECS: u64 = 60;

pub const WORKER_IDLE_NO_TOKEN_SECS: u64 = 30;
pub const WORKER_IDLE_EMPTY_QUEUE_SECS: u64 = 5;
pub const WORKER_IDLE_AFTER_SESSION_REVOKED_SECS: u64 = 60;
pub const WORKER_IDLE_BETWEEN_BATCHES_SECS: u64 = 2;

pub const EVENT_AUTH_SESSION_EXPIRED: &str = "auth-session-expired";

pub const SYNC_FLUSH_TIMEOUT_SECS: u64 = 30;

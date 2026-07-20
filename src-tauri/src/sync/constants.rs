/// Sincronização remota com a API Rails de trackings.
pub const BACKEND_SYNC_ENABLED: bool = true;

/// Tipos de entidade enfileirados para sincronização com a API.
pub const ENTITY_TRACKING: &str = "tracking";
pub const ENTITY_TRACKING_PERIPHERAL_EVENT: &str = "tracking_peripheral_event";
pub const ENTITY_TRACKING_INACTIVITY_PERIOD: &str = "tracking_inactivity_period";
pub const ENTITY_TRACKING_SCREENSHOT: &str = "tracking_screenshot";
pub const ENTITY_TRACKING_APP: &str = "tracking_app";
pub const ENTITY_TRACKING_SITE: &str = "tracking_site";

/// Tamanho máximo de lote por ciclo do worker.
pub const PENDING_BATCH_SIZE: usize = 10;

/// Timeout HTTP para requisições de sync e upload.
pub const HTTP_TIMEOUT_SECS: u64 = 60;

/// Intervalos de espera do worker entre ciclos (segundos).
pub const WORKER_IDLE_NO_TOKEN_SECS: u64 = 30;
pub const WORKER_IDLE_EMPTY_QUEUE_SECS: u64 = 5;
pub const WORKER_IDLE_AFTER_SESSION_REVOKED_SECS: u64 = 60;
pub const WORKER_IDLE_BETWEEN_BATCHES_SECS: u64 = 2;

/// Evento Tauri emitido quando a sessão de auth expira durante o sync.
pub const EVENT_AUTH_SESSION_EXPIRED: &str = "auth-session-expired";

/// Timeout máximo para flush do sync queue durante shutdown.
pub const SYNC_FLUSH_TIMEOUT_SECS: u64 = 30;

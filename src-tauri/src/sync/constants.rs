/// Sincronização remota com o backend fica desligada até a integração ser definida.
pub const BACKEND_SYNC_ENABLED: bool = false;

/// Tipos de entidade enfileirados para sincronização com a API.
pub const ENTITY_SESSION: &str = "session";
pub const ENTITY_ACTIVITY_TICK: &str = "activity_tick";
pub const ENTITY_IDLE_PERIOD: &str = "idle_period";
pub const ENTITY_SCREENSHOT: &str = "screenshot";

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

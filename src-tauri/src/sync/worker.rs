use crate::auth::{invalidate_session, read_access_token};
use crate::db::Database;
use crate::error::AgentError;
use crate::sync::{
    constants::{
        BACKEND_SYNC_ENABLED, ENTITY_TRACKING_SCREENSHOT, EVENT_AUTH_SESSION_EXPIRED, HTTP_TIMEOUT_SECS,
        MAX_SYNC_ATTEMPTS, PENDING_BATCH_SIZE, WORKER_IDLE_AFTER_SESSION_REVOKED_SECS,
        WORKER_IDLE_BETWEEN_BATCHES_SECS, WORKER_IDLE_EMPTY_QUEUE_SECS, WORKER_IDLE_NO_TOKEN_SECS,
    },
    fetch_pending_batch, mark_tracking_screenshot_synced, tracking_screenshot_file_path, send_sync_item,
    PendingSyncItem, SyncOutbox,
};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{async_runtime, AppHandle, Emitter};
use tokio::time::sleep;

pub struct SyncWorker {
    running: Arc<AtomicBool>,
    api_base_url: String,
    /// Callback invoked when the sync worker detects a 401 (session revoked).
    /// Stored as `Option` inside a `Mutex` so it can be set after construction
    /// and accessed by both `start` and `flush_blocking`.
    on_session_revoked: parking_lot::Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl SyncWorker {
    pub fn new(api_base_url: String) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            api_base_url,
            on_session_revoked: parking_lot::Mutex::new(None),
        }
    }

    /// Set the callback to invoke when a 401 is received during sync.
    pub fn set_on_session_revoked(&self, cb: Arc<dyn Fn() + Send + Sync>) {
        *self.on_session_revoked.lock() = Some(cb);
    }

    pub fn is_enabled(&self) -> bool {
        BACKEND_SYNC_ENABLED && !self.api_base_url.trim().is_empty()
    }

    pub fn start(self: Arc<Self>, db: Arc<Mutex<Database>>, app: AppHandle) {
        if !self.is_enabled() {
            return;
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let worker = Arc::clone(&self);
        async_runtime::spawn(async move {
            let http_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
                .build()
                .expect("failed to build sync http client");

            while worker.running.load(Ordering::SeqCst) {
                let on_session_revoked = worker.on_session_revoked.lock().clone();

                let batch = load_pending_batch(&db);
                let Some(access_token) = batch.access_token else {
                    sleep(Duration::from_secs(WORKER_IDLE_NO_TOKEN_SECS)).await;
                    continue;
                };

                if batch.items.is_empty() {
                    sleep(Duration::from_secs(WORKER_IDLE_EMPTY_QUEUE_SECS)).await;
                    continue;
                }

                let session_revoked = process_batch(
                    &http_client,
                    &worker.api_base_url,
                    &access_token,
                    &db,
                    &app,
                    &on_session_revoked,
                    batch.items,
                )
                .await;

                let idle_secs = if session_revoked {
                    WORKER_IDLE_AFTER_SESSION_REVOKED_SECS
                } else {
                    WORKER_IDLE_BETWEEN_BATCHES_SECS
                };
                sleep(Duration::from_secs(idle_secs)).await;
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Processa todos os itens pendentes na sync_queue de forma síncrona,
    /// útil durante shutdown para garantir que nenhum dado seja perdido.
    pub fn flush_blocking(
        self: &Arc<Self>,
        db: Arc<Mutex<Database>>,
        app: AppHandle,
        timeout_secs: u64,
    ) {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("sync flush: failed to create runtime: {e}");
                return;
            }
        };

        let api_base_url = self.api_base_url.clone();
        let on_session_revoked = self.on_session_revoked.lock().clone();

        rt.block_on(async move {
            let http_client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    log::error!("sync flush: failed to build HTTP client: {e}");
                    return;
                }
            };

            let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
            let mut total_processed = 0usize;

            loop {
                if tokio::time::Instant::now() >= deadline {
                    let remaining = count_pending(&db);
                    log::warn!(
                        "sync flush: timeout after {timeout_secs}s — {remaining} item(s) remain for next startup"
                    );
                    break;
                }

                let batch = load_pending_batch(&db);
                if batch.access_token.is_none() {
                    log::debug!("sync flush: no access token, skipping");
                    break;
                }
                if batch.items.is_empty() {
                    break;
                }

                let count = batch.items.len();
                log::info!("sync flush: processing {count} pending item(s)");

                process_batch(
                    &http_client,
                    &api_base_url,
                    &batch.access_token.unwrap(),
                    &db,
                    &app,
                    &on_session_revoked,
                    batch.items,
                )
                .await;

                total_processed += count;
            }

            log::info!("sync flush: complete — {total_processed} item(s) processed");
        });
    }
}

struct PendingBatch {
    access_token: Option<String>,
    items: Vec<PendingSyncItem>,
}

fn load_pending_batch(db: &Arc<Mutex<Database>>) -> PendingBatch {
    let db_guard = db.lock();
    let access_token = read_access_token(&db_guard).ok().flatten();
    let items = if access_token.is_some() {
        fetch_pending_batch(db_guard.conn(), PENDING_BATCH_SIZE).unwrap_or_default()
    } else {
        Vec::new()
    };
    PendingBatch {
        access_token,
        items,
    }
}

async fn process_batch(
    http_client: &reqwest::Client,
    api_base_url: &str,
    access_token: &str,
    db: &Arc<Mutex<Database>>,
    app: &AppHandle,
    on_session_revoked: &Option<Arc<dyn Fn() + Send + Sync>>,
    items: Vec<PendingSyncItem>,
) -> bool {
    let mut session_revoked = false;

    for item in items {
        if session_revoked {
            break;
        }

        let screenshot_path = prepare_item_for_send(db, &item);
        let result = send_sync_item(
            http_client,
            api_base_url,
            access_token,
            &item,
            screenshot_path,
        )
        .await;

        session_revoked = apply_sync_result(db, app, &item, result, on_session_revoked);
    }

    session_revoked
}

fn prepare_item_for_send(
    db: &Arc<Mutex<Database>>,
    item: &PendingSyncItem,
) -> Option<String> {
    let db_guard = db.lock();
    let _ = SyncOutbox::mark_sending(db_guard.conn(), &item.id);

    if item.entity_type == ENTITY_TRACKING_SCREENSHOT || item.entity_type == "screenshot" {
        tracking_screenshot_file_path(db_guard.conn(), &item.entity_id)
            .ok()
            .flatten()
            .or_else(|| screenshot_path_from_payload(&item.payload_json))
    } else {
        None
    }
}

fn screenshot_path_from_payload(payload_json: &str) -> Option<String> {
    let payload: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    payload
        .get("filePath")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn count_pending(db: &Arc<Mutex<Database>>) -> usize {
    let db_guard = db.lock();
    db_guard
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status IN ('pending', 'failed')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .unwrap_or(0)
}

fn apply_sync_result(
    db: &Arc<Mutex<Database>>,
    app: &AppHandle,
    item: &PendingSyncItem,
    result: Result<Option<String>, AgentError>,
    on_session_revoked: &Option<Arc<dyn Fn() + Send + Sync>>,
) -> bool {
    let db_guard = db.lock();
    match result {
        Ok(remote_path) => {
            let _ = SyncOutbox::mark_confirmed(db_guard.conn(), &item.id);
            if item.entity_type == ENTITY_TRACKING_SCREENSHOT || item.entity_type == "screenshot" {
                let _ = mark_tracking_screenshot_synced(
                    db_guard.conn(),
                    &item.entity_id,
                    remote_path.as_deref(),
                );
            }
            false
        }
        Err(AgentError::Auth(message)) => {
            log::info!("sync stopped: {message}");
            let _ = invalidate_session(&db_guard);
            let _ = app.emit(EVENT_AUTH_SESSION_EXPIRED, ());
            // Dispatch the session-revoked callback (stop tracking, flag auth false)
            if let Some(cb) = on_session_revoked.clone() {
                tauri::async_runtime::spawn_blocking(move || {
                    cb();
                });
            }
            true
        }
        Err(AgentError::SyncTerminal(message)) => {
            log::warn!("sync item {} permanently rejected (dead-letter): {message}", item.id);
            let _ = SyncOutbox::mark_dead(db_guard.conn(), &item.id, &message);
            false
        }
        Err(err) => {
            let attempts = item.attempts + 1;
            if attempts >= MAX_SYNC_ATTEMPTS {
                log::warn!(
                    "sync item {} exceeded max attempts ({attempts}) → dead-letter: {err}",
                    item.id
                );
                let _ = SyncOutbox::mark_dead(db_guard.conn(), &item.id, &err.to_string());
            } else {
                log::warn!("sync item {} failed (attempt {attempts}): {err}", item.id);
                let _ = SyncOutbox::mark_failed(db_guard.conn(), &item.id, &err.to_string(), attempts);
            }
            false
        }
    }
}

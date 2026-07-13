use crate::auth::{invalidate_session, read_access_token};
use crate::db::Database;
use crate::error::AgentError;
use crate::sync::{
    constants::{
        BACKEND_SYNC_ENABLED, ENTITY_SCREENSHOT, EVENT_AUTH_SESSION_EXPIRED, HTTP_TIMEOUT_SECS, PENDING_BATCH_SIZE,
        WORKER_IDLE_AFTER_SESSION_REVOKED_SECS, WORKER_IDLE_BETWEEN_BATCHES_SECS,
        WORKER_IDLE_EMPTY_QUEUE_SECS, WORKER_IDLE_NO_TOKEN_SECS,
    },
    fetch_pending_batch, mark_screenshot_synced, screenshot_file_path, send_sync_item,
    validate_entity_before_sync, PendingSyncItem, SyncOutbox,
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
}

impl SyncWorker {
    pub fn new(api_base_url: String) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            api_base_url,
        }
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

        session_revoked = apply_sync_result(db, app, &item, result);
    }

    session_revoked
}

fn prepare_item_for_send(
    db: &Arc<Mutex<Database>>,
    item: &PendingSyncItem,
) -> Option<String> {
    let db_guard = db.lock();
    let _ = validate_entity_before_sync(db_guard.conn(), &item.entity_type, &item.entity_id);
    let _ = SyncOutbox::mark_sending(db_guard.conn(), &item.id);

    if item.entity_type == ENTITY_SCREENSHOT {
        screenshot_file_path(db_guard.conn(), &item.entity_id)
            .ok()
            .flatten()
    } else {
        None
    }
}

fn apply_sync_result(
    db: &Arc<Mutex<Database>>,
    app: &AppHandle,
    item: &PendingSyncItem,
    result: Result<(), AgentError>,
) -> bool {
    let db_guard = db.lock();
    match result {
        Ok(()) => {
            let _ = SyncOutbox::mark_confirmed(db_guard.conn(), &item.id);
            if item.entity_type == ENTITY_SCREENSHOT {
                let _ = mark_screenshot_synced(db_guard.conn(), &item.entity_id);
            }
            false
        }
        Err(AgentError::Auth(message)) => {
            log::info!("sync stopped: {message}");
            let _ = invalidate_session(&db_guard);
            let _ = app.emit(EVENT_AUTH_SESSION_EXPIRED, ());
            true
        }
        Err(err) => {
            log::warn!("sync item {} failed: {err}", item.id);
            let _ = SyncOutbox::mark_failed(
                db_guard.conn(),
                &item.id,
                &err.to_string(),
                item.attempts + 1,
            );
            false
        }
    }
}

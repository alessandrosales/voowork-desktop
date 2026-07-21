use crate::db::Database;
use crate::screenshot::ScreenshotCapture;
use crate::sync::worker::SyncWorker;
use crate::tracking::TrackingManager;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub tracking_manager: Arc<TrackingManager>,
    pub api_base_url: String,
    pub sync_worker: Arc<SyncWorker>,
}

impl AppState {
    pub fn new(
        db: Database,
        screenshot: ScreenshotCapture,
        app: AppHandle,
    ) -> Self {
        let db = Arc::new(Mutex::new(db));
        let tracking_manager = Arc::new(TrackingManager::new(
            Arc::clone(&db),
            screenshot,
        ));
        let api_base_url = crate::auth::configured_api_base_url();
        let sync_worker = Arc::new(SyncWorker::new(api_base_url.clone()));

        // Build the session-revoked callback: stop tracking first (local), then
        // flag as unauthenticated. Order: stop first (outbox enqueue is local,
        // no token needed), then flag. Second concurrent stop errors harmlessly.
        let tm = Arc::clone(&tracking_manager);
        let cb: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if tm.status().active {
                if let Err(err) = tm.stop_tracking() {
                    log::warn!("session-revoked: stop tracking failed: {err}");
                }
            } else {
                log::info!("session-revoked: no active tracking to stop");
            }
            tm.set_session_authenticated(false);
        });
        sync_worker.set_on_session_revoked(Arc::clone(&cb));

        if sync_worker.is_enabled() {
            sync_worker.clone().start(Arc::clone(&db), app);
            log::info!("tracking sync worker started for {}", api_base_url);
        } else {
            log::info!("tracking sync worker disabled (backend integration pending)");
        }

        Self {
            db,
            tracking_manager,
            api_base_url,
            sync_worker,
        }
    }
}

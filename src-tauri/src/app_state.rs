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
        }
    }
}

use crate::auth;
use crate::crypto::DeviceKeys;
use crate::db::Database;
use crate::screenshot::ScreenshotCapture;
use crate::session::SessionManager;
use crate::sync::worker::SyncWorker;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::AppHandle;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub session_manager: Arc<SessionManager>,
    pub api_base_url: String,
}

impl AppState {
    pub fn new(
        db: Database,
        device_keys: DeviceKeys,
        screenshot: ScreenshotCapture,
        app: AppHandle,
    ) -> Self {
        let db = Arc::new(Mutex::new(db));
        let device_keys = Arc::new(device_keys);
        let session_manager = Arc::new(SessionManager::new(
            Arc::clone(&db),
            Arc::clone(&device_keys),
            screenshot,
        ));
        let api_base_url = auth::configured_api_base_url();
        let sync_worker = Arc::new(SyncWorker::new(api_base_url.clone()));
        if sync_worker.is_enabled() {
            sync_worker.clone().start(Arc::clone(&db), app);
            log::info!("tracking sync worker started for {}", api_base_url);
        } else {
            log::info!("tracking sync worker disabled (backend integration pending)");
        }

        Self {
            db,
            session_manager,
            api_base_url,
        }
    }
}

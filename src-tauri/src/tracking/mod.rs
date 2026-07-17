pub use buffer::ActivityBuffer;
pub use constants::{
    load_screenshot_interval_secs, APP_FOCUS_POLL_SECS, SCREENSHOT_BASE_INTERVAL_SECS,
    SETTING_SCREENSHOT_INTERVAL_SECS,
};

use crate::activity::{ActivityTracker, TrackerMode};
use crate::tracking_focus::{
    capture_active_window, is_communication_app, open_tracking_app, should_track_active_window,
    ActiveWindowSample,
};
use crate::auth::read_session_identity;
use crate::crypto::DeviceKeys;
use crate::db::Database;
use crate::db::TIME_CATEGORY_ACTIVE;
use crate::error::{AgentError, AgentResult};
use crate::tracking_inactivity::{
    load_inactivity_threshold_minutes, TrackingInactivityController,
};
use crate::screenshot::ScreenshotCapture;
use crate::sync::{SyncOutbox, ENTITY_TRACKING};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

mod buffer;
mod capture;
mod constants;
mod inactivity_ui;
mod lifecycle;
mod notifications;
mod status_report;
mod worker;

#[derive(Debug, Clone)]
pub struct ActiveTracking {
    pub tracking_id: String,
    pub project_id: String,
    pub task_id: String,
    pub started_at: String,
    pub last_screenshot_at: Option<String>,
    pub current_period_start: String,
}

pub struct TrackingManager {
    pub(crate) db: Arc<Mutex<Database>>,
    pub(crate) tracker: Arc<Mutex<ActivityTracker>>,
    pub(crate) screenshot: Arc<Mutex<ScreenshotCapture>>,
    pub(crate) active: Arc<Mutex<Option<ActiveTracking>>>,
    pub(crate) tracking_active_flag: Arc<Mutex<bool>>,
    pub(crate) worker_running: Arc<AtomicBool>,
    pub(crate) worker_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) totals: Arc<Mutex<TrackingTotals>>,
    pub(crate) last_active_window: Arc<Mutex<Option<ActiveWindowSample>>>,
    pub(crate) active_app_id: Arc<Mutex<Option<String>>>,
    pub(crate) active_site_id: Arc<Mutex<Option<String>>>,
    pub(crate) last_site_address: Arc<Mutex<Option<String>>>,
    pub(crate) inactivity_controller: Arc<Mutex<Option<Arc<TrackingInactivityController>>>>,
    pub(crate) app_handle: Arc<Mutex<Option<AppHandle>>>,
    pub(crate) activity_buffer: ActivityBuffer,
    session_authenticated: Arc<AtomicBool>,
    buffer_eligible: Arc<AtomicBool>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct TrackingTotals {
    mouse_events: u64,
    keyboard_events: u64,
    last_confidence: f64,
    last_activity_score: u8,
}

impl TrackingManager {
    pub fn new(
        db: Arc<Mutex<Database>>,
        screenshot: ScreenshotCapture,
    ) -> Self {
        let db_for_buffer = Arc::clone(&db);
        Self {
            db,
            tracker: Arc::new(Mutex::new(ActivityTracker::new())),
            screenshot: Arc::new(Mutex::new(screenshot)),
            active: Arc::new(Mutex::new(None)),
            tracking_active_flag: Arc::new(Mutex::new(false)),
            worker_running: Arc::new(AtomicBool::new(false)),
            worker_handle: Arc::new(Mutex::new(None)),
            totals: Arc::new(Mutex::new(TrackingTotals::default())),
            last_active_window: Arc::new(Mutex::new(None)),
            active_app_id: Arc::new(Mutex::new(None)),
            active_site_id: Arc::new(Mutex::new(None)),
            last_site_address: Arc::new(Mutex::new(None)),
            inactivity_controller: Arc::new(Mutex::new(None)),
            app_handle: Arc::new(Mutex::new(None)),
            activity_buffer: ActivityBuffer::new(db_for_buffer),
            session_authenticated: Arc::new(AtomicBool::new(false)),
            buffer_eligible: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_session_authenticated(&self, authenticated: bool) {
        self.session_authenticated
            .store(authenticated, Ordering::SeqCst);
        self.buffer_eligible.store(false, Ordering::SeqCst);
        self.activity_buffer.dismiss();
    }

    pub fn start_background_services(&self) {
        self.tracker.lock().start();
        self.activity_buffer.start_watcher(
            Arc::clone(&self.tracking_active_flag),
            self.tracker.lock().last_input_at(),
            Arc::clone(&self.session_authenticated),
            Arc::clone(&self.buffer_eligible),
        );
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        self.tracker.lock().set_app_handle(handle.clone());
        *self.app_handle.lock() = Some(handle);
    }

    pub fn start_tracking(
        &self,
        project_id: String,
        task_id: String,
    ) -> AgentResult<ActiveTracking> {
        if self.active.lock().is_some() {
            return Err(AgentError::Session("tracking already active".into()));
        }
        if task_id.trim().is_empty() {
            return Err(AgentError::Session("task is required".into()));
        }

        self.buffer_eligible.store(true, Ordering::SeqCst);

        let tracking_id = Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let period_start = started_at.clone();
        let buffer_seconds = self.activity_buffer.claim();

        let (account_id, user_id, device_name) = {
            let db = self.db.lock();
            let identity = read_session_identity(&db)?
                .ok_or_else(|| AgentError::Auth("user not authenticated".into()))?;
            let device_name = DeviceKeys::device_name(db.conn())?;
            (identity.organization.id, identity.user.id, device_name)
        };

        {
            let db = self.db.lock();
            if buffer_seconds > 0 {
                db.add_task_active_seconds(&task_id, buffer_seconds)?;
                log::info!("claimed {buffer_seconds}s from activity buffer for task {task_id}");
            }
            db.insert_tracking(
                &tracking_id,
                &account_id,
                &project_id,
                &task_id,
                &user_id,
                Some(&device_name),
                &started_at,
            )?;

            SyncOutbox::enqueue(
                db.conn(),
                ENTITY_TRACKING,
                &tracking_id,
                serde_json::json!({
                    "trackingId": tracking_id,
                    "accountId": account_id,
                    "projectId": project_id,
                    "taskId": task_id,
                    "userId": user_id,
                    "startedAt": started_at,
                    "status": "active",
                    "device": device_name,
                }),
            )?;
            crate::tray::persist_last_selection(&db, &project_id, &task_id);
        }

        *self.totals.lock() = TrackingTotals::default();
        *self.last_active_window.lock() = None;
        *self.active_app_id.lock() = None;
        *self.active_site_id.lock() = None;
        *self.last_site_address.lock() = None;
        *self.tracking_active_flag.lock() = true;

        {
            let tracker = self.tracker.lock();
            let threshold_minutes = {
                let db = self.db.lock();
                load_inactivity_threshold_minutes(db.conn())
            };
            let inactivity_controller = Arc::new(TrackingInactivityController::new(
                threshold_minutes * 60,
                tracker.last_input_at(),
                tracker.last_input_wall_at(),
            ));
            *self.inactivity_controller.lock() = Some(inactivity_controller);
        }

        let tracking = ActiveTracking {
            tracking_id: tracking_id.clone(),
            project_id,
            task_id,
            started_at,
            last_screenshot_at: None,
            current_period_start: period_start,
        };
        *self.active.lock() = Some(tracking.clone());
        self.spawn_worker();
        Self::defer_initial_focus_capture(
            Arc::clone(&self.db),
            tracking_id,
            Arc::clone(&self.inactivity_controller),
            Arc::clone(&self.last_active_window),
        );

        Ok(tracking)
    }

    fn defer_initial_focus_capture(
        db: Arc<Mutex<Database>>,
        tracking_id: String,
        inactivity_controller: Arc<Mutex<Option<Arc<TrackingInactivityController>>>>,
        last_active_window: Arc<Mutex<Option<ActiveWindowSample>>>,
    ) {
        std::thread::spawn(move || {
            let Some(sample) = capture_active_window() else {
                return;
            };
            if should_track_active_window(&sample) {
                let now = chrono::Utc::now().to_rfc3339();
                let db_guard = db.lock();
                if let Err(err) =
                    open_tracking_app(db_guard.conn(), &tracking_id, &sample, &now)
                {
                    log::warn!("deferred initial focus capture failed: {err}");
                }
            }
            *last_active_window.lock() = Some(sample.clone());
            if let Some(controller) = inactivity_controller.lock().clone() {
                controller.set_meeting_exempt(is_communication_app(&sample));
            }
        });
    }

    pub fn restart_tracking(
        &self,
        project_id: String,
        task_id: String,
    ) -> AgentResult<ActiveTracking> {
        if self.active.lock().is_some() {
            if let Err(err) = status_report::persist_task_time_snapshot_state(
                &self.db,
                &self.active,
                &self.inactivity_controller,
            ) {
                log::warn!("persist task time snapshot before restart failed: {err}");
            }
            self.stop_tracking()?;
        }
        self.start_tracking(project_id, task_id)
    }

    pub fn dismiss_activity_buffer(&self) {
        self.activity_buffer.dismiss();
    }

    pub fn pause_tracking(&self) -> AgentResult<()> {
        if self.active.lock().is_none() {
            return Err(AgentError::Session("no active tracking".into()));
        }

        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            inactivity_controller.pause_manually();
        }
        if let Some(tracking) = self.active.lock().as_mut() {
            tracking.current_period_start = chrono::Utc::now().to_rfc3339();
        }

        if let Err(err) = status_report::persist_task_time_snapshot_state(
            &self.db,
            &self.active,
            &self.inactivity_controller,
        ) {
            log::warn!("persist task time snapshot on manual pause failed: {err}");
        }

        let db = Arc::clone(&self.db);
        let active = Arc::clone(&self.active);
        let screenshot = Arc::clone(&self.screenshot);
        let tracker = Arc::clone(&self.tracker);
        let totals = Arc::clone(&self.totals);
        std::thread::spawn(move || {
            if let Err(err) = capture::flush_period_screenshot(
                &db,
                &screenshot,
                &tracker,
                &totals,
                &active,
                TIME_CATEGORY_ACTIVE,
            ) {
                log::warn!("deferred flush on manual pause failed: {err}");
            }
        });

        if let Some(app) = self.app_handle.lock().clone() {
            let _ = app.emit("tracking-inactivity-changed", ());
        }
        Ok(())
    }

    pub fn resume_tracking(&self) -> AgentResult<()> {
        if self.active.lock().is_none() {
            return Err(AgentError::Session("no active tracking".into()));
        }
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            inactivity_controller.resume_manually();
        }
        if let Some(tracking) = self.active.lock().as_mut() {
            tracking.current_period_start = chrono::Utc::now().to_rfc3339();
        }
        if let Some(app) = self.app_handle.lock().clone() {
            let _ = app.emit("tracking-inactivity-changed", ());
        }
        Ok(())
    }

    pub fn stop_tracking(&self) -> AgentResult<()> {
        self.finalize_active_tracking(true)
    }

    /// Encerramento do app: finaliza tracking e não bloqueia a main thread no worker.
    pub fn shutdown_for_quit(&self) -> AgentResult<()> {
        self.abandon_active_tracking(false)?;

        let db = self.db.lock();
        crate::sync::finalize::finalize_orphaned_trackings(&db)?;
        db.clear_task_time_totals()?;
        Ok(())
    }

    /// Sinaliza parada imediata sem locks de DB (safe no callback da tray).
    pub fn prepare_immediate_exit(&self) {
        self.worker_running.store(false, Ordering::SeqCst);
        self.buffer_eligible.store(false, Ordering::SeqCst);
        self.session_authenticated.store(false, Ordering::SeqCst);
        let _ = self.worker_handle.lock().take();
    }

    /// Inicializa sessão: finaliza trackings órfãos de crash anterior e zera tempos locais.
    pub fn initialize_session(&self) -> AgentResult<u32> {
        let db = self.db.lock();
        let count = crate::sync::finalize::finalize_orphaned_trackings(&db)?;
        db.clear_task_time_totals()?;
        Ok(count)
    }

    pub fn set_screenshot_blur(&self, enabled: bool) {
        self.screenshot.lock().set_blur(enabled);
    }

    pub fn set_screenshot_jpeg_quality(&self, quality: u8) {
        self.screenshot.lock().set_jpeg_quality(quality);
    }

    pub fn confirm_still_working(&self) -> AgentResult<()> {
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            inactivity_controller.confirm_still_working()?;
        }
        Ok(())
    }

    pub fn classify_tracking_inactivity_period(
        &self,
        period_id: &str,
        category: &str,
    ) -> AgentResult<()> {
        let inactivity_controller = self
            .inactivity_controller
            .lock()
            .clone()
            .ok_or_else(|| AgentError::Session("no active tracking".into()))?;
        let db = self.db.lock();
        inactivity_controller.classify_tracking_inactivity_period(db.conn(), period_id, category)
    }

    pub fn skip_tracking_inactivity_classification(&self) -> AgentResult<()> {
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            let db = self.db.lock();
            inactivity_controller.skip_tracking_inactivity_classification(db.conn())?;
        }
        Ok(())
    }

    pub fn confirm_manual_work(&self) -> AgentResult<()> {
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            inactivity_controller.confirm_manual_work()?;
        }
        Ok(())
    }

    pub fn dismiss_manual_work_check(&self) -> AgentResult<()> {
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            inactivity_controller.dismiss_manual_work_check()?;
        }
        Ok(())
    }

    pub fn tracker_mode(&self) -> TrackerMode {
        self.tracker.lock().mode()
    }

    pub fn tracker_has_permission(&self) -> bool {
        self.tracker.lock().is_permission_granted()
    }
}

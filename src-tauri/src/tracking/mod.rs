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
use crate::db::{Database, TIME_CATEGORY_INACTIVITY};
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
use std::time::Instant;
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
    pub(crate) tracking_active_flag: Arc<AtomicBool>,
    /// Monotonic clock snapshot when tracking started — used for skew detection.
    started_at_monotonic: Mutex<Option<Instant>>,
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
    /// Outermost lock — never acquire it while holding `active`, `db`, or any
    /// field mutex; the worker thread never acquires it.
    state_transition: parking_lot::Mutex<()>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct TrackingTotals {
    mouse_events: u64,
    keyboard_events: u64,
    last_confidence: f64,
    last_activity_score: u8,
    screenshot_count: u64,
    inactivity_seconds: u64,
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
            tracking_active_flag: Arc::new(AtomicBool::new(false)),
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
            state_transition: parking_lot::Mutex::new(()),
            started_at_monotonic: Mutex::new(None),
        }
    }

    pub fn set_session_authenticated(&self, authenticated: bool) {
        self.session_authenticated
            .store(authenticated, Ordering::SeqCst);
        if authenticated {
            // Login or boot hydration: preserve any restored buffer state.
            // If there's a pending alert from a previous session, make it
            // visible. Do NOT dismiss — the buffer survives hydration.
            if self.activity_buffer.has_pending_alert() {
                self.buffer_eligible.store(true, Ordering::SeqCst);
            } else {
                self.buffer_eligible.store(false, Ordering::SeqCst);
            }
        } else {
            self.buffer_eligible.store(false, Ordering::SeqCst);
            self.activity_buffer.dismiss();
        }
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
        let _guard = self.state_transition.lock();
        self.start_tracking_inner(project_id, task_id)
    }

    fn start_tracking_inner(
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

        let (account_id, user_id, device_name) = {
            let db = self.db.lock();
            let identity = read_session_identity(&db)?
                .ok_or_else(|| AgentError::Auth("user not authenticated".into()))?;
            let device_name = DeviceKeys::device_name(db.conn())?;
            (identity.organization.id, identity.user.id, device_name)
        };

        self.buffer_eligible.store(true, Ordering::SeqCst);

        let tracking_id = Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let period_start = started_at.clone();
        let buffer_seconds = self.activity_buffer.claim();

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
        self.tracking_active_flag.store(true, Ordering::SeqCst);

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
        *self.started_at_monotonic.lock() = Some(std::time::Instant::now());
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
                let db_guard = db.lock();
                if let Err(err) = controller.set_meeting_exempt(is_communication_app(&sample), db_guard.conn()) {
                    log::warn!("set_meeting_exempt during initial focus failed: {err}");
                }
            }
        });
    }

    pub fn restart_tracking(
        &self,
        project_id: String,
        task_id: String,
    ) -> AgentResult<ActiveTracking> {
        let _guard = self.state_transition.lock();
        self.restart_tracking_inner(project_id, task_id)
    }

    fn restart_tracking_inner(
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
            self.finalize_active_tracking_inner(true)?;
        }
        self.start_tracking_inner(project_id, task_id)
    }

    pub fn dismiss_activity_buffer(&self) {
        self.activity_buffer.dismiss();
    }

    pub fn pause_tracking(&self) -> AgentResult<()> {
        let _guard = self.state_transition.lock();
        self.pause_tracking_inner()
    }

    fn pause_tracking_inner(&self) -> AgentResult<()> {
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

        if let Some(app) = self.app_handle.lock().clone() {
            let _ = app.emit("tracking-inactivity-changed", ());
        }
        Ok(())
    }

    pub fn resume_tracking(&self) -> AgentResult<()> {
        let _guard = self.state_transition.lock();
        self.resume_tracking_inner()
    }

    fn resume_tracking_inner(&self) -> AgentResult<()> {
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
        let _guard = self.state_transition.lock();
        self.finalize_active_tracking_inner(true)
    }

    /// Takes a final screenshot and properly finalizes the active tracking,
    /// so the last period of work is captured before shutdown.
    ///
    /// Called before `prepare_immediate_exit()` on tray quit, and also at the
    /// start of `shutdown_for_quit()` for the `RunEvent::Exit` path.
    ///
    /// Ordering (A8):
    /// 1. stop_worker() — signal + join (BEFORE final screenshot)
    /// 2. Final screenshot (now uncontended)
    /// 3. Close open apps/sites
    /// 4. Finalize tracking + enqueue PATCH
    /// 5. Clear active, tracking_active_flag, and focus fields
    pub fn capture_final_screenshot_and_finalize(&self) {
        let Some(tracking) = self.active.lock().clone() else {
            return;
        };

        // 1. Stop worker FIRST — releases screenshot/db locks before capture
        self.stop_worker();

        let period_start = tracking.current_period_start.clone();

        // Determine time category based on current inactivity phase
        let time_category = self
            .inactivity_controller
            .lock()
            .as_ref()
            .map(|ctrl| {
                let phase = ctrl.snapshot().phase;
                capture::screenshot_time_category(phase)
            })
            .unwrap_or(crate::db::TIME_CATEGORY_ACTIVE);

        // 2. Capture final screenshot (best-effort — never block quit on failure)
        let _ = capture::capture_screenshot(
            &self.db,
            &self.screenshot,
            &self.tracker,
            &self.totals,
            &tracking,
            &period_start,
            time_category,
        );

        // 3. Close open apps/sites
        let _ = capture::close_open_apps(&self.db, &self.active_app_id);
        let _ = capture::close_open_sites(
            &self.db,
            &self.active_site_id,
            &self.last_site_address,
        );

        // 4. Finalize tracking + enqueue PATCH
        let ended_at = chrono::Utc::now().to_rfc3339();
        if let Err(err) = (|| -> AgentResult<()> {
            let db = self.db.lock();
            let elapsed = status_report::snapshot_task_elapsed(&db, &tracking, None)?;
            db.set_task_active_seconds(&tracking.task_id, elapsed)?;
            db.finalize_tracking(&tracking.tracking_id, &ended_at)?;

            crate::sync::SyncOutbox::enqueue(
                db.conn(),
                crate::sync::ENTITY_TRACKING,
                &tracking.tracking_id,
                serde_json::json!({
                    "trackingId": tracking.tracking_id,
                    "endedAt": ended_at,
                    "status": "inactive",
                }),
            )?;

            // Enqueue any pending apps/sites to sync
            if let Some(app_id) = self.active_app_id.lock().clone() {
                if let Ok(app) = db.get_tracking_app(&app_id) {
                    crate::sync::SyncOutbox::enqueue(
                        db.conn(),
                        crate::sync::ENTITY_TRACKING_APP,
                        &app.id,
                        serde_json::json!({
                            "appId": app.id,
                            "trackingId": app.tracking_id,
                            "name": app.name,
                            "startedAt": app.started_at,
                            "endedAt": ended_at,
                        }),
                    )?;
                }
            }
            Ok(())
        })() {
            log::warn!("finalize tracking on quit failed: {err}");
        }

        // 5. Clear in-memory state — makes this idempotent
        *self.active.lock() = None;
        self.tracking_active_flag.store(false, Ordering::SeqCst);
        *self.inactivity_controller.lock() = None;
        *self.totals.lock() = TrackingTotals::default();
        *self.last_active_window.lock() = None;
        *self.active_app_id.lock() = None;
        *self.active_site_id.lock() = None;
        *self.last_site_address.lock() = None;
    }

    /// Encerramento do app: finaliza tracking e não bloqueia a main thread no worker.
    pub fn shutdown_for_quit(&self) -> AgentResult<()> {
        self.capture_final_screenshot_and_finalize();

        // Clear in-memory state (same as lifecycle::clear_tracking_memory)
        *self.active.lock() = None;
        self.tracking_active_flag.store(false, Ordering::SeqCst);
        *self.inactivity_controller.lock() = None;
        *self.totals.lock() = TrackingTotals::default();
        *self.last_active_window.lock() = None;
        *self.active_app_id.lock() = None;
        *self.active_site_id.lock() = None;
        *self.last_site_address.lock() = None;

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
        match crate::sync::requeue_stuck_sending_items(db.conn()) {
            Ok(requeued) if requeued > 0 => {
                log::warn!("requeued {requeued} sync item(s) stuck in 'sending' from previous run");
            }
            Ok(_) => {}
            Err(err) => log::warn!("failed to requeue stuck sync items: {err}"),
        }
        if let Err(err) = db.purge_confirmed_sync_items() {
            log::warn!("failed to purge old sync items: {err}");
        }
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

    /// Dismisses the paused inactivity period: discards the idle time
    /// record, keeps pre-idle work time, takes a screenshot, and returns
    /// to Active.
    pub fn dismiss_inactivity_period(&self) -> AgentResult<()> {
        let tracking = self.active.lock().clone();
        let Some(tracking) = tracking else {
            return Err(AgentError::Session("no active tracking".into()));
        };
        let period_start = tracking.current_period_start.clone();

        // 1. Persist task time snapshot before resetting controller state
        if let Err(err) = status_report::persist_task_time_snapshot_state(
            &self.db,
            &self.active,
            &self.inactivity_controller,
        ) {
            log::warn!("persist task time snapshot before inactivity dismiss failed: {err}");
        }

        // 2. Dismiss the inactivity period in controller + DB
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            let db = self.db.lock();
            inactivity_controller.dismiss_inactivity_period(db.conn())?;
        }

        // 3. Take a screenshot with inactivity category to mark end of idle period
        let result = capture::capture_screenshot(
            &self.db,
            &self.screenshot,
            &self.tracker,
            &self.totals,
            &tracking,
            &period_start,
            TIME_CATEGORY_INACTIVITY,
        );

        // 4. Update period start — both Ok and Err paths advance period_start
        match result {
            Ok(outcome) => {
                if let Some(active_tracking) = self.active.lock().as_mut() {
                    active_tracking.current_period_start = outcome.period_end;
                    if let Some(ref record) = outcome.screenshot {
                        active_tracking.last_screenshot_at = Some(record.captured_at.clone());
                    }
                }
            }
            Err(err) => {
                log::warn!("screenshot on inactivity dismiss failed: {err}");
                if let Some(active_tracking) = self.active.lock().as_mut() {
                    active_tracking.current_period_start = chrono::Utc::now().to_rfc3339();
                }
            }
        }

        // 5. Emit event so frontend refreshes
        if let Some(app) = self.app_handle.lock().clone() {
            let _ = app.emit("tracking-inactivity-changed", ());
        }

        Ok(())
    }

    /// Classifies the paused inactivity period as billable: credits the
    /// idle time to the task, takes a closing screenshot, and resumes
    /// active tracking.
    pub fn classify_paused_inactivity_period(&self) -> AgentResult<()> {
        let tracking = self.active.lock().clone();
        let Some(tracking) = tracking else {
            return Err(AgentError::Session("no active tracking".into()));
        };
        let period_start = tracking.current_period_start.clone();

        // 1. Classify from paused in controller (credits idle time,
        //    transitions to Active)
        if let Some(inactivity_controller) = self.inactivity_controller.lock().clone() {
            let db = self.db.lock();
            inactivity_controller.classify_from_paused_inactivity(db.conn())?;
        }

        // 2. Persist the updated task time (now includes credited idle seconds)
        if let Err(err) = status_report::persist_task_time_snapshot_state(
            &self.db,
            &self.active,
            &self.inactivity_controller,
        ) {
            log::warn!("persist task time after classifying paused inactivity failed: {err}");
        }

        // 3. Restart segment timer (persist cleared it via reset_billable_seconds)
        if let Some(controller) = self.inactivity_controller.lock().as_ref() {
            controller.restart_segment_timer();
        }

        // 4. Take a screenshot to mark the end of the idle period
        let result = capture::capture_screenshot(
            &self.db,
            &self.screenshot,
            &self.tracker,
            &self.totals,
            &tracking,
            &period_start,
            crate::db::TIME_CATEGORY_INACTIVITY,
        );

        // 5. Update period start — both Ok and Err paths advance period_start
        match result {
            Ok(outcome) => {
                if let Some(active_tracking) = self.active.lock().as_mut() {
                    active_tracking.current_period_start = outcome.period_end;
                    if let Some(ref record) = outcome.screenshot {
                        active_tracking.last_screenshot_at = Some(record.captured_at.clone());
                    }
                }
            }
            Err(err) => {
                log::warn!("screenshot on inactivity classify failed: {err}");
                if let Some(active_tracking) = self.active.lock().as_mut() {
                    active_tracking.current_period_start = chrono::Utc::now().to_rfc3339();
                }
            }
        }

        // 6. Emit event so frontend refreshes
        if let Some(app) = self.app_handle.lock().clone() {
            let _ = app.emit("tracking-inactivity-changed", ());
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

#[cfg(test)]
mod state_transition_tests {
    use super::*;
    use crate::screenshot::ScreenshotCapture;
    use crate::auth::store::{KEY_USER, KEY_ORGANIZATION};
    use crate::auth::KEY_AUTHENTICATED;
    use crate::crypto::DeviceKeys;
    use std::path::PathBuf;

    fn test_db_dir() -> PathBuf {
        PathBuf::from(std::env::temp_dir()).join(format!("voowork-a7-test-{}", uuid::Uuid::new_v4()))
    }

    fn setup_test_manager() -> (Arc<TrackingManager>, PathBuf) {
        let dir = test_db_dir();
        let db = {
            let db = Database::open(dir.clone()).unwrap();
            // Set up device metadata
            DeviceKeys::ensure(db.conn(), "test-device").unwrap();
            // Set up auth data
            db.set_setting(KEY_AUTHENTICATED, "true").unwrap();
            let user = serde_json::json!({
                "id": "user-1",
                "name": "Test User",
                "email": "test@example.com",
                "profile": "admin"
            });
            let org = serde_json::json!({
                "id": "org-1",
                "name": "Test Org"
            });
            db.set_setting(KEY_USER, &user.to_string()).unwrap();
            db.set_setting(KEY_ORGANIZATION, &org.to_string()).unwrap();
            db
        };
        let db = Arc::new(Mutex::new(db));
        let screenshot_dir = dir.join("screenshots");
        let screenshot = ScreenshotCapture::new(screenshot_dir).unwrap();
        let manager = Arc::new(TrackingManager::new(Arc::clone(&db), screenshot));
        (manager, dir)
    }

    #[test]
    fn double_start_fails() {
        let (manager, _dir) = setup_test_manager();

        // First start should succeed
        let result = manager.start_tracking("proj-1".into(), "task-1".into());
        assert!(result.is_ok(), "first start should succeed: {:?}", result.err());

        // Second start should fail (already active)
        let result = manager.start_tracking("proj-1".into(), "task-2".into());
        assert!(result.is_err(), "double start should fail");

        // Stop should succeed
        let result = manager.stop_tracking();
        assert!(result.is_ok(), "stop after valid start should succeed: {:?}", result.err());

        // Start again after stop should succeed
        let result = manager.start_tracking("proj-1".into(), "task-2".into());
        assert!(result.is_ok(), "start after stop should succeed: {:?}", result.err());
    }

    #[test]
    fn start_without_auth_fails_without_claiming_buffer() {
        // N1: Verifica se o buffer NÃO é reivindicado quando a auth falha.
        // Cria TrackingManager sem dados de autenticação — `read_session_identity`
        // deve falhar ANTES de buffer_eligible/claim serem chamados.
        let dir = PathBuf::from(std::env::temp_dir()).join(format!("voowork-n1-test-{}", uuid::Uuid::new_v4()));
        let db = {
            let db = Database::open(dir.clone()).unwrap();
            // Configura device metadata MAS NÃO auth
            DeviceKeys::ensure(db.conn(), "test-device").unwrap();
            db
        };
        let db = Arc::new(Mutex::new(db));
        let screenshot = ScreenshotCapture::new(dir.join("screenshots")).unwrap();
        let manager = Arc::new(TrackingManager::new(Arc::clone(&db), screenshot));

        // Tentar start tracking sem auth deve falhar
        let result = manager.start_tracking("proj-1".into(), "task-1".into());
        assert!(result.is_err(), "start without auth should fail");

        // Verificar que o buffer NÃO foi reivindicado (ainda é elegível)
        // buffer_eligible ainda é false (nunca foi setado para true)
        assert!(!manager.buffer_eligible.load(Ordering::SeqCst),
            "buffer_eligible should remain false when auth fails");

        // Verificar que o estado tracking_active_flag não foi setado
        assert!(!manager.tracking_active_flag.load(Ordering::SeqCst),
            "tracking_active_flag should remain false when auth fails");
    }
}

mod capture;
mod constants;
mod idle_ui;
mod notifications;
mod worker;

pub use constants::{
    screenshot_base_interval_secs, APP_FOCUS_POLL_SECS, FIRST_SCREENSHOT_SECS, FIRST_TICK_SECS,
    SCREENSHOT_JITTER_SECS, TICK_INTERVAL_SECS,
};

use crate::activity::{ActivityTracker, TrackerMode, tracker_mode_label};
use crate::app_focus::{
    capture_active_window, insert_app_focus, is_communication_app, should_track_app_focus,
    AppFocusSample,
};
use crate::clock::{monotonic_ns_since, ClockMonitor};
use crate::crypto::DeviceKeys;
use crate::db::Database;
use crate::error::{AgentError, AgentResult};
use crate::idle::{IdleController, load_idle_threshold_minutes};
use crate::integrity::{finalize_session, insert_session};
use crate::models::{IdleStatus, SessionStatus};
use crate::screenshot::ScreenshotCapture;
use crate::sync::{SyncOutbox, ENTITY_SESSION};
use capture::flush_tick;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;
use tauri::AppHandle;
use uuid::Uuid;
use worker::{spawn_session_worker, SessionWorkerContext};

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub session_id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub started_at: String,
    pub started_instant: Instant,
}

pub struct SessionManager {
    db: Arc<Mutex<Database>>,
    device_keys: Arc<DeviceKeys>,
    tracker: Arc<Mutex<ActivityTracker>>,
    screenshot: Arc<Mutex<ScreenshotCapture>>,
    active: Arc<Mutex<Option<ActiveSession>>>,
    clock_monitor: Arc<Mutex<ClockMonitor>>,
    worker_running: Arc<AtomicBool>,
    worker_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    totals: Arc<Mutex<SessionTotals>>,
    last_app_focus: Arc<Mutex<Option<AppFocusSample>>>,
    idle: Arc<Mutex<Option<Arc<IdleController>>>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SessionTotals {
    mouse_events: u64,
    keyboard_events: u64,
    last_confidence: f64,
    clock_skew_detected: bool,
}

impl SessionManager {
    pub fn new(
        db: Arc<Mutex<Database>>,
        device_keys: Arc<DeviceKeys>,
        screenshot: ScreenshotCapture,
    ) -> Self {
        Self {
            db,
            device_keys,
            tracker: Arc::new(Mutex::new(ActivityTracker::new())),
            screenshot: Arc::new(Mutex::new(screenshot)),
            active: Arc::new(Mutex::new(None)),
            clock_monitor: Arc::new(Mutex::new(ClockMonitor::new())),
            worker_running: Arc::new(AtomicBool::new(false)),
            worker_handle: Arc::new(Mutex::new(None)),
            totals: Arc::new(Mutex::new(SessionTotals::default())),
            last_app_focus: Arc::new(Mutex::new(None)),
            idle: Arc::new(Mutex::new(None)),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock() = Some(handle);
    }

    pub fn start_session(
        &self,
        project_id: String,
        task_id: Option<String>,
    ) -> AgentResult<ActiveSession> {
        if self.active.lock().is_some() {
            return Err(AgentError::Session("session already active".into()));
        }

        let session_id = Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let started_instant = Instant::now();
        let monotonic_started_ns = 0i64;

        {
            let db = self.db.lock();
            let device_id = DeviceKeys::device_id(db.conn())?;
            let device_public_key = DeviceKeys::public_key_b64(db.conn())?;

            insert_session(
                db.conn(),
                &session_id,
                &project_id,
                task_id.as_deref(),
                &started_at,
                monotonic_started_ns,
            )?;

            let payload = serde_json::json!({
                "sessionId": session_id,
                "projectId": project_id,
                "taskId": task_id,
                "startedAt": started_at,
                "deviceId": device_id,
                "devicePublicKey": device_public_key,
            });
            SyncOutbox::enqueue(
                db.conn(),
                ENTITY_SESSION,
                &session_id,
                payload,
                &self.device_keys,
            )?;
        }

        *self.totals.lock() = SessionTotals::default();
        *self.last_app_focus.lock() = None;
        self.clock_monitor.lock().reset();
        self.tracker.lock().start();
        log::info!("activity tracker started for session {session_id}");

        {
            let tracker = self.tracker.lock();
            let threshold_minutes = {
                let db = self.db.lock();
                load_idle_threshold_minutes(db.conn())
            };
            let idle = Arc::new(IdleController::new(
                threshold_minutes * 60,
                tracker.last_input_at(),
                tracker.last_input_wall_at(),
            ));
            *self.idle.lock() = Some(idle);
        }

        {
            let db = self.db.lock();
            if let Some(sample) = capture_active_window() {
                if should_track_app_focus(&sample) {
                    insert_app_focus(db.conn(), &session_id, &sample)?;
                }
                *self.last_app_focus.lock() = Some(sample.clone());
                if let Some(idle) = self.idle.lock().clone() {
                    idle.set_meeting_exempt(is_communication_app(&sample));
                }
            }
        }

        self.spawn_worker();

        let session = ActiveSession {
            session_id,
            project_id,
            task_id,
            started_at,
            started_instant,
        };
        *self.active.lock() = Some(session.clone());
        Ok(session)
    }

    pub fn stop_session(&self) -> AgentResult<()> {
        let session = self
            .active
            .lock()
            .take()
            .ok_or_else(|| AgentError::Session("no active session".into()))?;

        log::info!("stopping session {}", session.session_id);
        self.worker_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.lock().take() {
            let _ = handle.join();
        }

        self.flush_activity_tick(&session)?;
        self.tracker.lock().stop();
        *self.idle.lock() = None;

        let ended_at = chrono::Utc::now().to_rfc3339();
        let monotonic_ended_ns = monotonic_ns_since(session.started_instant);
        let clock_skew_flags = self.clock_monitor.lock().skew_events() as i64;

        {
            let db = self.db.lock();
            finalize_session(
                db.conn(),
                &session.session_id,
                &ended_at,
                monotonic_ended_ns,
                clock_skew_flags,
            )?;

            let payload = serde_json::json!({
                "sessionId": session.session_id,
                "endedAt": ended_at,
                "monotonicEndedNs": monotonic_ended_ns,
                "clockSkewFlags": clock_skew_flags,
            });
            SyncOutbox::enqueue(
                db.conn(),
                ENTITY_SESSION,
                &session.session_id,
                payload,
                &self.device_keys,
            )?;
        }

        Ok(())
    }

    pub fn recover_orphaned_sessions(&self) -> AgentResult<u32> {
        let db = self.db.lock();
        let conn = db.conn();
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, monotonic_started_ns FROM sessions WHERE status = 'active' AND ended_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut recovered = 0u32;
        for row in rows {
            let (session_id, monotonic_started_ns) = row?;
            log::warn!("recovering orphaned active session {session_id}");
            finalize_session(
                conn,
                &session_id,
                &now,
                monotonic_started_ns.max(0),
                0,
            )?;
            recovered += 1;
        }

        Ok(recovered)
    }

    pub fn set_screenshot_blur(&self, enabled: bool) {
        self.screenshot.lock().set_blur(enabled);
    }

    pub fn confirm_still_working(&self) -> AgentResult<()> {
        if let Some(idle) = self.idle.lock().clone() {
            idle.confirm_still_working()?;
        }
        Ok(())
    }

    pub fn classify_idle_period(&self, period_id: &str, category: &str) -> AgentResult<()> {
        let idle = self
            .idle
            .lock()
            .clone()
            .ok_or_else(|| AgentError::Session("no active session".into()))?;
        let db = self.db.lock();
        idle.classify_idle_period(db.conn(), period_id, category, &self.device_keys)
    }

    pub fn skip_idle_classification(&self) -> AgentResult<()> {
        if let Some(idle) = self.idle.lock().clone() {
            let db = self.db.lock();
            idle.skip_idle_classification(db.conn(), &self.device_keys)?;
        }
        Ok(())
    }

    pub fn confirm_manual_work(&self) -> AgentResult<()> {
        if let Some(idle) = self.idle.lock().clone() {
            idle.confirm_manual_work()?;
        }
        Ok(())
    }

    pub fn dismiss_manual_work_check(&self) -> AgentResult<()> {
        if let Some(idle) = self.idle.lock().clone() {
            idle.dismiss_manual_work_check()?;
        }
        Ok(())
    }

    pub fn pause_session(&self) -> AgentResult<()> {
        if self.active.lock().is_none() {
            return Err(AgentError::Session("no active session".into()));
        }
        self.tracker.lock().drain_bucket();
        if let Some(idle) = self.idle.lock().clone() {
            idle.pause_manually();
        }
        Ok(())
    }

    pub fn resume_session(&self) -> AgentResult<()> {
        if self.active.lock().is_none() {
            return Err(AgentError::Session("no active session".into()));
        }
        if let Some(idle) = self.idle.lock().clone() {
            idle.resume_manually();
        }
        Ok(())
    }

    pub fn status(&self) -> SessionStatus {
        let active = self.active.lock().clone();
        let totals = self.totals.lock().clone();
        let idle_snapshot = self
            .idle
            .lock()
            .as_ref()
            .map(|idle| idle.snapshot());

        if let Some(session) = active {
            let idle = idle_snapshot
                .map(idle_status_from_snapshot)
                .unwrap_or_default();
            let elapsed = idle.active_seconds;
            let last_focus = self.last_app_focus.lock().clone();
            let (screenshot_count, last_screenshot_at) = {
                let db = self.db.lock();
                db.screenshot_stats_for_session(&session.session_id)
                    .unwrap_or((0, None))
            };
            return SessionStatus {
                active: true,
                session_id: Some(session.session_id),
                project_id: Some(session.project_id),
                task_id: session.task_id,
                started_at: Some(session.started_at),
                elapsed_seconds: elapsed,
                mouse_events: totals.mouse_events,
                keyboard_events: totals.keyboard_events,
                clock_skew_detected: totals.clock_skew_detected,
                activity_confidence: totals.last_confidence,
                tracker_mode: Some(tracker_mode_label(self.tracker.lock().mode()).into()),
                current_app: last_focus.as_ref().map(|f| f.app_name.clone()),
                current_window_title: last_focus.map(|f| f.window_title),
                screenshot_count,
                last_screenshot_at,
                idle,
            };
        }

        SessionStatus {
            active: false,
            session_id: None,
            project_id: None,
            task_id: None,
            started_at: None,
            elapsed_seconds: 0,
            mouse_events: 0,
            keyboard_events: 0,
            clock_skew_detected: false,
            activity_confidence: 1.0,
            tracker_mode: Some(tracker_mode_label(self.tracker.lock().mode()).into()),
            current_app: None,
            current_window_title: None,
            screenshot_count: 0,
            last_screenshot_at: None,
            idle: IdleStatus::default(),
        }
    }

    pub fn tracker_mode(&self) -> TrackerMode {
        self.tracker.lock().mode()
    }

    fn spawn_worker(&self) {
        self.worker_running.store(true, Ordering::SeqCst);

        let handle = spawn_session_worker(SessionWorkerContext {
            worker_running: Arc::clone(&self.worker_running),
            active: Arc::clone(&self.active),
            tracker: Arc::clone(&self.tracker),
            db: Arc::clone(&self.db),
            device_keys: Arc::clone(&self.device_keys),
            screenshot: Arc::clone(&self.screenshot),
            clock_monitor: Arc::clone(&self.clock_monitor),
            totals: Arc::clone(&self.totals),
            last_app_focus: Arc::clone(&self.last_app_focus),
            idle: Arc::clone(&self.idle),
            app_handle: Arc::clone(&self.app_handle),
        });

        *self.worker_handle.lock() = Some(handle);
    }

    fn flush_activity_tick(&self, session: &ActiveSession) -> AgentResult<()> {
        flush_tick(
            &self.db,
            &self.device_keys,
            &self.tracker,
            &self.totals,
            session,
            constants::TICK_INTERVAL_SECS,
        )
    }
}

fn idle_status_from_snapshot(snapshot: crate::idle::IdleSnapshot) -> IdleStatus {
    IdleStatus {
        phase: snapshot.phase.as_str().into(),
        threshold_secs: snapshot.threshold_secs,
        countdown_secs: snapshot.countdown_secs,
        countdown_remaining_secs: snapshot.countdown_remaining_secs,
        countdown_ends_at: snapshot.countdown_ends_at,
        idle_started_at: snapshot.idle_started_at,
        paused_at: snapshot.paused_at,
        away_seconds: snapshot.away_seconds,
        pending_period_id: snapshot.pending_period_id,
        meeting_exempt: snapshot.meeting_exempt,
        active_seconds: snapshot.active_seconds,
        idle_discarded_seconds: snapshot.idle_discarded_seconds,
        idle_reclassified_seconds: snapshot.idle_reclassified_seconds,
    }
}

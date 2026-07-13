use crate::activity::ActivityTracker;
use crate::app_focus::AppFocusSample;
use crate::clock::ClockMonitor;
use crate::crypto::DeviceKeys;
use crate::db::Database;
use crate::idle::{IdleController, IdlePhase};
use crate::screenshot::ScreenshotCapture;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::AppHandle;

use super::capture::{capture_screenshot, flush_tick, record_app_focus};
use super::constants::{
    screenshot_base_interval_secs, APP_FOCUS_POLL_SECS, FIRST_SCREENSHOT_SECS, FIRST_TICK_SECS,
    TICK_INTERVAL_SECS,
};
use super::idle_ui::handle_idle_phase_transition;
use super::{ActiveSession, SessionTotals};
use crate::screenshot::random_interval_secs;

pub(crate) struct SessionWorkerContext {
    pub worker_running: Arc<AtomicBool>,
    pub active: Arc<Mutex<Option<ActiveSession>>>,
    pub tracker: Arc<Mutex<ActivityTracker>>,
    pub db: Arc<Mutex<Database>>,
    pub device_keys: Arc<DeviceKeys>,
    pub screenshot: Arc<Mutex<ScreenshotCapture>>,
    pub clock_monitor: Arc<Mutex<ClockMonitor>>,
    pub totals: Arc<Mutex<SessionTotals>>,
    pub last_app_focus: Arc<Mutex<Option<AppFocusSample>>>,
    pub idle: Arc<Mutex<Option<Arc<IdleController>>>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
}

pub(crate) fn spawn_session_worker(ctx: SessionWorkerContext) -> JoinHandle<()> {
    let SessionWorkerContext {
        worker_running,
        active,
        tracker,
        db,
        device_keys,
        screenshot,
        clock_monitor,
        totals,
        last_app_focus,
        idle,
        app_handle,
    } = ctx;

    thread::spawn(move || {
        let mut tick_elapsed = Duration::ZERO;
        let mut screenshot_elapsed = Duration::ZERO;
        let mut app_focus_elapsed = Duration::ZERO;
        let mut first_tick_done = false;
        let tick_interval = Duration::from_secs(TICK_INTERVAL_SECS);
        let first_tick_interval = Duration::from_secs(FIRST_TICK_SECS);
        let app_focus_interval = Duration::from_secs(APP_FOCUS_POLL_SECS);
        let screenshot_base_interval = screenshot_base_interval_secs();
        let mut screenshot_interval = Duration::from_secs(FIRST_SCREENSHOT_SECS);

        while worker_running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
            tick_elapsed += Duration::from_secs(1);
            screenshot_elapsed += Duration::from_secs(1);
            app_focus_elapsed += Duration::from_secs(1);

            let session = active.lock().clone();
            let Some(session) = session else {
                break;
            };

            let skew = clock_monitor.lock().check_skew(Duration::from_secs(30));
            if skew {
                totals.lock().clock_skew_detected = true;
            }

            let idle_phase = idle
                .lock()
                .clone()
                .map(|ctrl| ctrl.snapshot().phase)
                .unwrap_or(IdlePhase::Active);
            let capture_suspended = matches!(
                idle_phase,
                IdlePhase::PausedIdle
                    | IdlePhase::ManualPaused
                    | IdlePhase::ManualWorkCheck
            );

            if app_focus_elapsed >= app_focus_interval {
                app_focus_elapsed = Duration::ZERO;
                if let Err(err) = record_app_focus(&db, &session, &last_app_focus, &idle) {
                    log::warn!("failed to record app focus: {err}");
                }
            }

            let idle_transition = if let Some(idle_ctrl) = idle.lock().clone() {
                let phase_before = idle_ctrl.snapshot().phase;
                let tick_result = {
                    let db_guard = db.lock();
                    idle_ctrl.tick(db_guard.conn(), &session.session_id, &device_keys)
                };
                if let Err(err) = tick_result {
                    log::warn!("idle tick failed: {err}");
                    None
                } else if phase_before != idle_ctrl.snapshot().phase {
                    Some((phase_before, idle_ctrl.snapshot().phase))
                } else {
                    None
                }
            } else {
                None
            };

            if let (Some(app), Some((before, after))) =
                (app_handle.lock().clone(), idle_transition)
            {
                if after == IdlePhase::PausedIdle {
                    tracker.lock().drain_bucket();
                }
                handle_idle_phase_transition(&app, before, after);
            }

            let should_flush_tick = !capture_suspended
                && if !first_tick_done {
                    tick_elapsed >= first_tick_interval
                } else {
                    tick_elapsed >= tick_interval
                };

            if should_flush_tick {
                let bucket_secs = if first_tick_done {
                    TICK_INTERVAL_SECS
                } else {
                    FIRST_TICK_SECS
                };
                tick_elapsed = Duration::ZERO;
                first_tick_done = true;
                if let Err(err) = flush_tick(
                    &db,
                    &device_keys,
                    &tracker,
                    &totals,
                    &session,
                    bucket_secs,
                ) {
                    log::error!("failed to flush activity tick: {err}");
                }
            }

            if !capture_suspended && screenshot_elapsed >= screenshot_interval {
                screenshot_elapsed = Duration::ZERO;
                screenshot_interval =
                    Duration::from_secs(random_interval_secs(screenshot_base_interval));
                if let Err(err) = capture_screenshot(
                    &db,
                    &device_keys,
                    &screenshot,
                    &session,
                ) {
                    log::warn!("screenshot capture failed: {err}");
                }
            }
        }
    })
}

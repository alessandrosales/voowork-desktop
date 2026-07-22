use crate::activity::ActivityTracker;
use crate::tracking_focus::ActiveWindowSample;
use crate::db::Database;
use crate::tracking_inactivity::{TrackingInactivityController, TrackingInactivityPhase};
use crate::screenshot::ScreenshotCapture;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::AppHandle;

use super::capture::{
    capture_screenshot, close_open_apps, close_open_apps_at, close_open_sites,
    close_open_sites_at, record_tracking_app_and_site, screenshot_time_category,
};
use super::constants::{
    load_randomized_screenshot_interval, APP_FOCUS_POLL_SECS,
};
use super::inactivity_ui::handle_inactivity_phase_transition;
use super::{ActiveTracking, TrackingTotals};

pub(crate) struct TrackingWorkerContext {
    pub worker_running: Arc<AtomicBool>,
    pub active: Arc<Mutex<Option<ActiveTracking>>>,
    pub tracker: Arc<Mutex<ActivityTracker>>,
    pub db: Arc<Mutex<Database>>,
    pub screenshot: Arc<Mutex<ScreenshotCapture>>,
    pub totals: Arc<Mutex<TrackingTotals>>,
    pub last_active_window: Arc<Mutex<Option<ActiveWindowSample>>>,
    pub active_app_id: Arc<Mutex<Option<String>>>,
    pub active_site_id: Arc<Mutex<Option<String>>>,
    pub last_site_address: Arc<Mutex<Option<String>>>,
    pub inactivity_controller: Arc<Mutex<Option<Arc<TrackingInactivityController>>>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
}

pub(crate) fn spawn_tracking_worker(ctx: TrackingWorkerContext) -> JoinHandle<()> {
    let TrackingWorkerContext {
        worker_running,
        active,
        tracker,
        db,
        screenshot,
        totals,
        last_active_window,
        active_app_id,
        active_site_id,
        last_site_address,
        inactivity_controller,
        app_handle,
    } = ctx;

    thread::spawn(move || {
        let mut screenshot_elapsed = Duration::ZERO;
        let mut tracking_focus_elapsed = Duration::ZERO;
        let tracking_focus_interval = Duration::from_secs(APP_FOCUS_POLL_SECS);
        let mut period_start = chrono::Utc::now().to_rfc3339();

        while worker_running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
            screenshot_elapsed += Duration::from_secs(1);
            tracking_focus_elapsed += Duration::from_secs(1);

            let tracking = active.lock().clone();
            let Some(tracking) = tracking else {
                break;
            };

            if tracking_focus_elapsed >= tracking_focus_interval {
                tracking_focus_elapsed = Duration::ZERO;
                if let Err(err) = record_tracking_app_and_site(
                    &db,
                    &tracking,
                    &last_active_window,
                    &active_app_id,
                    &active_site_id,
                    &last_site_address,
                    &inactivity_controller,
                ) {
                    log::warn!("failed to record tracking app/site focus: {err}");
                }
            }

            let inactivity_transition = if let Some(idle_ctrl) = inactivity_controller.lock().clone() {
                let phase_before = idle_ctrl.snapshot().phase;
                let tick_result = {
                    let db_guard = db.lock();
                    idle_ctrl.tick(db_guard.conn(), &tracking.tracking_id)
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
                (app_handle.lock().clone(), inactivity_transition)
            {
                handle_inactivity_phase_transition(&app, before, after);
            }

            if let Some(Some((ref idle_ctrl, after))) =
                inactivity_transition.map(|(_, after)| {
                    inactivity_controller
                        .lock()
                        .clone()
                        .map(|ctrl| (ctrl, after))
                })
            {
                if after == TrackingInactivityPhase::PausedInactivity {
                    if let Some(ref started_at) = idle_ctrl.inactivity_started_at() {
                        // Captura imediata ao entrar em PausedInactivity:
                        // snapshot com time_category=inactivity e period_start=inactivity_started_at
                        let time_category = screenshot_time_category(TrackingInactivityPhase::PausedInactivity);
                        match capture_screenshot(
                            &db,
                            &screenshot,
                            &tracker,
                            &totals,
                            &tracking,
                            started_at,
                            time_category,
                        ) {
                            Ok(outcome) => {
                                let captured_at = outcome.period_end.clone();
                                period_start = captured_at.clone();
                                if let Some(active_tracking) = active.lock().as_mut() {
                                    active_tracking.current_period_start = captured_at.clone();
                                    if let Some(ref record) = outcome.screenshot {
                                        active_tracking.last_screenshot_at =
                                            Some(record.captured_at.clone());
                                        active_tracking.last_screenshot_hash =
                                            Some(record.sha256_hash.clone());
                                    }
                                }
                                screenshot_elapsed = Duration::ZERO;
                            }
                            Err(err) => log::warn!("inactivity snapshot screenshot failed: {err}"),
                        }

                        // Use current time, NOT inactivity_started_at, for closing apps.
                        // The focus poll may have opened a new tracking_app record
                        // during the countdown (e.g. window title change), whose
                        // started_at would be AFTER inactivity_started_at — causing
                        // ended_at < started_at in the database.
                        let paused_now = chrono::Utc::now().to_rfc3339();
                        let _ = close_open_apps_at(&db, &active_app_id, &paused_now);
                        let _ = close_open_sites_at(&db, &active_site_id, &last_site_address, &paused_now);
                    }
                }
            }

            let screenshot_interval = {
                let db_guard = db.lock();
                Duration::from_secs(load_randomized_screenshot_interval(db_guard.conn()))
            };

            if screenshot_elapsed >= screenshot_interval {
                screenshot_elapsed = Duration::ZERO;
                let screenshot_phase = inactivity_controller
                    .lock()
                    .clone()
                    .map(|ctrl| ctrl.snapshot().phase)
                    .unwrap_or(TrackingInactivityPhase::Active);

                if matches!(
                    screenshot_phase,
                    TrackingInactivityPhase::ManualPaused
                        | TrackingInactivityPhase::ManualWorkCheck
                ) {
                    period_start = chrono::Utc::now().to_rfc3339();
                    continue;
                }

                let time_category = screenshot_time_category(screenshot_phase);
                match capture_screenshot(
                    &db,
                    &screenshot,
                    &tracker,
                    &totals,
                    &tracking,
                    &period_start,
                    time_category,
                ) {
                    Ok(outcome) => {
                        let period_end = outcome.period_end;
                        period_start = period_end.clone();
                        if let Some(active_tracking) = active.lock().as_mut() {
                            active_tracking.current_period_start = period_end;
                            if let Some(ref record) = outcome.screenshot {
                                active_tracking.last_screenshot_at =
                                    Some(record.captured_at.clone());
                                active_tracking.last_screenshot_hash =
                                    Some(record.sha256_hash.clone());
                            }
                        }
                    }
                    Err(err) => log::warn!("screenshot capture failed: {err}"),
                }
            }
        }

        let _ = close_open_apps(&db, &active_app_id);
        let _ = close_open_sites(&db, &active_site_id, &last_site_address);
    })
}

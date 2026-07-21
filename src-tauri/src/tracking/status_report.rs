use crate::activity::tracker_mode_label;
use crate::db::{Database, TIME_CATEGORY_ACTIVE, TIME_CATEGORY_INACTIVITY};
use crate::error::{AgentError, AgentResult};
use crate::models::{TrackingInactivityStatus, TrackingStatus};
use crate::tracking_inactivity::TrackingInactivityPhase;

use super::{ActiveTracking, TrackingManager};

impl TrackingManager {
    pub fn status(&self) -> TrackingStatus {
        let active = self.active.lock().clone();
        let totals = self.totals.lock().clone();
        let inactivity_snapshot = self
            .inactivity_controller
            .lock()
            .as_ref()
            .map(|inactivity_controller| inactivity_controller.snapshot());
        let buffer = self.activity_buffer.snapshot();

        if let Some(tracking) = active {
            let last_focus = self.last_active_window.lock().clone();
            let session_active_secs = inactivity_snapshot
                .as_ref()
                .map(|snapshot| snapshot.active_seconds);
            let inactivity_phase = inactivity_snapshot.as_ref().map(|s| s.phase);
            let (screenshot_count, last_screenshot_at, elapsed_seconds, inactivity_seconds) = {
                let db = self.db.lock();
                let task_base = db.get_task_active_seconds(&tracking.task_id).unwrap_or(0);
                drop(db);
                let session_active = session_active_secs.unwrap_or(0);
                let elapsed = task_base + session_active;
                let extra_idle = match inactivity_phase {
                    Some(TrackingInactivityPhase::PausedInactivity)
                    | Some(TrackingInactivityPhase::ResumePrompt) => {
                        let now = chrono::Utc::now().to_rfc3339();
                        crate::db::period_duration_seconds(
                            &tracking.current_period_start,
                            &now,
                        )
                        .unwrap_or(0)
                    }
                    _ => 0,
                };
                (
                    totals.screenshot_count,
                    tracking.last_screenshot_at.clone(),
                    elapsed,
                    totals.inactivity_seconds + extra_idle,
                )
            };
            let inactivity_status = inactivity_snapshot
                .as_ref()
                .map(|snapshot| inactivity_status_from_snapshot(snapshot.clone()))
                .unwrap_or_default();

            let clock_skew_detected = self.started_at_monotonic.lock().as_ref().is_some_and(|start| {
                let monotonic_elapsed = start.elapsed().as_secs();
                monotonic_elapsed.abs_diff(elapsed_seconds) > 60
            });

            return TrackingStatus {
                active: true,
                tracking_id: Some(tracking.tracking_id),
                project_id: Some(tracking.project_id),
                task_id: Some(tracking.task_id),
                started_at: Some(tracking.started_at),
                elapsed_seconds,
                inactivity_seconds,
                task_accumulated_seconds: elapsed_seconds,
                activity_buffer_seconds: buffer.seconds,
                activity_buffer_alert: buffer.alert_pending,
                mouse_events: totals.mouse_events,
                keyboard_events: totals.keyboard_events,
                clock_skew_detected,
                activity_confidence: totals.last_confidence,
                activity_score: totals.last_activity_score,
                tracker_mode: Some(tracker_mode_label(self.tracker.lock().mode()).into()),
                current_app: last_focus.as_ref().map(|f| f.app_name.clone()),
                current_window_title: last_focus.map(|f| f.window_title),
                screenshot_count,
                last_screenshot_at,
                inactivity: inactivity_status,
            };
        }

        TrackingStatus {
            active: false,
            tracking_id: None,
            project_id: None,
            task_id: None,
            started_at: None,
            elapsed_seconds: 0,
            inactivity_seconds: 0,
            task_accumulated_seconds: 0,
            activity_buffer_seconds: buffer.seconds,
            activity_buffer_alert: buffer.alert_pending,
            mouse_events: 0,
            keyboard_events: 0,
            clock_skew_detected: false,
            activity_confidence: 1.0,
            activity_score: 0,
            tracker_mode: Some(tracker_mode_label(self.tracker.lock().mode()).into()),
            current_app: None,
            current_window_title: None,
            screenshot_count: 0,
            last_screenshot_at: None,
            inactivity: TrackingInactivityStatus::default(),
        }
    }

    pub fn task_elapsed_seconds(&self, task_id: &str) -> AgentResult<u64> {
        let active = self.active.lock().clone();
        let Some(tracking) = active else {
            let db = self.db.lock();
            return db.get_task_active_seconds(task_id);
        };

        if tracking.task_id != task_id {
            let db = self.db.lock();
            return db.get_task_active_seconds(task_id);
        }

        let inactivity_snapshot = self
            .inactivity_controller
            .lock()
            .as_ref()
            .map(|controller| controller.snapshot());
        let session_active = inactivity_snapshot.as_ref().map(|snapshot| snapshot.active_seconds).unwrap_or(0);
        let db = self.db.lock();
        let task_base = db.get_task_active_seconds(&tracking.task_id)?;
        Ok(task_base + session_active)
    }
}

pub(crate) fn persist_task_time_snapshot_state(
    db: &std::sync::Arc<parking_lot::Mutex<Database>>,
    active: &std::sync::Arc<parking_lot::Mutex<Option<ActiveTracking>>>,
    inactivity_controller: &std::sync::Arc<
        parking_lot::Mutex<Option<std::sync::Arc<crate::tracking_inactivity::TrackingInactivityController>>>,
    >,
) -> AgentResult<u64> {
    let tracking = active
        .lock()
        .clone()
        .ok_or_else(|| AgentError::Session("no active tracking".into()))?;

    let controller = inactivity_controller.lock().clone();
    let session_active = controller
        .as_ref()
        .map(|value| value.snapshot().active_seconds)
        .unwrap_or(0);
    let elapsed = {
        let db_guard = db.lock();
        let elapsed = snapshot_task_elapsed_fast(&db_guard, &tracking, session_active)?;
        db_guard.set_task_active_seconds(&tracking.task_id, elapsed)?;
        elapsed
    };

    if let Some(controller) = controller {
        controller.reset_billable_seconds();
    }

    Ok(elapsed)
}

pub(crate) fn snapshot_task_elapsed_fast(
    db: &Database,
    tracking: &ActiveTracking,
    session_active: u64,
) -> AgentResult<u64> {
    let task_base = db.get_task_active_seconds(&tracking.task_id)?;
    Ok(task_base + session_active)
}

pub(crate) fn snapshot_task_elapsed(
    db: &Database,
    tracking: &ActiveTracking,
    inactivity_controller: Option<&crate::tracking_inactivity::TrackingInactivityController>,
) -> AgentResult<u64> {
    let snapshot = inactivity_controller.map(|controller| controller.snapshot());
    let phase = snapshot.as_ref().map(|value| value.phase);
    let session_active = snapshot.as_ref().map(|value| value.active_seconds);
    let (elapsed, _) = compute_display_times(db, tracking, phase, session_active)?;
    Ok(elapsed)
}

fn compute_display_times(
    db: &Database,
    tracking: &ActiveTracking,
    inactivity_phase: Option<TrackingInactivityPhase>,
    session_active_from_controller: Option<u64>,
) -> AgentResult<(u64, u64)> {
    use crate::db::period_duration_seconds;

    let task_base = db.get_task_active_seconds(&tracking.task_id)?;
    let session_idle =
        db.sum_screenshot_seconds(&tracking.tracking_id, Some(TIME_CATEGORY_INACTIVITY))?;

    let phase = inactivity_phase.unwrap_or(TrackingInactivityPhase::Active);
    let session_active = if let Some(active_secs) = session_active_from_controller {
        active_secs
    } else {
        let flushed_active =
            db.sum_screenshot_seconds(&tracking.tracking_id, Some(TIME_CATEGORY_ACTIVE))?;
        let now = chrono::Utc::now().to_rfc3339();
        let in_progress = period_duration_seconds(&tracking.current_period_start, &now)?;
        match phase {
            TrackingInactivityPhase::PausedInactivity
            | TrackingInactivityPhase::ResumePrompt
            | TrackingInactivityPhase::ManualPaused
            | TrackingInactivityPhase::ManualWorkCheck => flushed_active,
            _ => flushed_active + in_progress,
        }
    };

    let extra_idle = match phase {
        TrackingInactivityPhase::PausedInactivity | TrackingInactivityPhase::ResumePrompt => {
            let now = chrono::Utc::now().to_rfc3339();
            period_duration_seconds(&tracking.current_period_start, &now)?
        }
        _ => 0,
    };

    Ok((task_base + session_active, session_idle + extra_idle))
}

fn inactivity_status_from_snapshot(
    snapshot: crate::tracking_inactivity::TrackingInactivitySnapshot,
) -> TrackingInactivityStatus {
    TrackingInactivityStatus {
        phase: snapshot.phase.as_str().into(),
        threshold_secs: snapshot.threshold_secs,
        countdown_secs: snapshot.countdown_secs,
        countdown_remaining_secs: snapshot.countdown_remaining_secs,
        countdown_ends_at: snapshot.countdown_ends_at,
        inactivity_started_at: snapshot.inactivity_started_at,
        paused_at: snapshot.paused_at,
        away_seconds: snapshot.away_seconds,
        pending_period_id: snapshot.pending_period_id,
        meeting_exempt: snapshot.meeting_exempt,
        active_seconds: snapshot.active_seconds,
        inactivity_discarded_seconds: snapshot.inactivity_discarded_seconds,
        inactivity_reclassified_seconds: snapshot.inactivity_reclassified_seconds,
    }
}

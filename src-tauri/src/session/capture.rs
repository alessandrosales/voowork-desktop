use crate::activity::ActivityTracker;
use crate::app_focus::{
    capture_active_window, insert_app_focus, is_communication_app, should_track_app_focus,
    AppFocusSample,
};
use crate::auth::read_authenticated_user_id;
use crate::clock::monotonic_ns_since;
use crate::crypto::DeviceKeys;
use crate::db::Database;
use crate::error::AgentResult;
use crate::idle::IdleController;
use crate::integrity::{insert_activity_tick, ActivityTickRecord};
use crate::screenshot::{ScreenshotCapture, ScreenshotCaptureContext, ScreenshotRecord};
use crate::sync::{SyncOutbox, ENTITY_ACTIVITY_TICK, ENTITY_SCREENSHOT};
use parking_lot::Mutex;
use std::sync::Arc;
use uuid::Uuid;

use super::{ActiveSession, SessionTotals};

pub(crate) fn record_app_focus(
    db: &Arc<Mutex<Database>>,
    session: &ActiveSession,
    last_app_focus: &Arc<Mutex<Option<AppFocusSample>>>,
    idle: &Arc<Mutex<Option<Arc<IdleController>>>>,
) -> AgentResult<()> {
    let Some(sample) = capture_active_window() else {
        return Ok(());
    };

    if let Some(idle_ctrl) = idle.lock().clone() {
        idle_ctrl.set_meeting_exempt(is_communication_app(&sample));
    }

    let should_insert = should_track_app_focus(&sample)
        && {
            let last = last_app_focus.lock();
            last.as_ref().map_or(true, |prev| {
                prev.app_name != sample.app_name || prev.window_title != sample.window_title
            })
        };

    if should_insert {
        let db_guard = db.lock();
        insert_app_focus(db_guard.conn(), &session.session_id, &sample)?;
    }
    *last_app_focus.lock() = Some(sample);

    Ok(())
}

pub(crate) fn flush_tick(
    db: &Arc<Mutex<Database>>,
    device_keys: &Arc<DeviceKeys>,
    tracker: &Arc<Mutex<ActivityTracker>>,
    totals: &Arc<Mutex<SessionTotals>>,
    session: &ActiveSession,
    bucket_secs: u64,
) -> AgentResult<()> {
    let bucket = tracker.lock().drain_bucket();
    let tick_id = Uuid::new_v4().to_string();
    let bucket_end = chrono::Utc::now().to_rfc3339();
    let bucket_start = (
        chrono::Utc::now() - chrono::Duration::seconds(bucket_secs as i64)
    )
        .to_rfc3339();
    let monotonic_elapsed_ns = monotonic_ns_since(session.started_instant);
    let wall_clock_at_tick = chrono::Utc::now().to_rfc3339();

    let positions_json = if bucket.positions.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&bucket.positions)?)
    };

    let record = ActivityTickRecord {
        id: tick_id.clone(),
        session_id: session.session_id.clone(),
        bucket_start,
        bucket_end,
        mouse_events: bucket.mouse_events as i64,
        keyboard_events: bucket.keyboard_events as i64,
        mouse_positions_json: positions_json,
        activity_score_confidence: bucket.confidence,
        automation_flags: bucket.automation_flags as i64,
        monotonic_elapsed_ns,
        wall_clock_at_tick,
        clock_skew_detected: if totals.lock().clock_skew_detected {
            1
        } else {
            0
        },
    };

    {
        let mut totals_guard = totals.lock();
        totals_guard.mouse_events += bucket.mouse_events;
        totals_guard.keyboard_events += bucket.keyboard_events;
        totals_guard.last_confidence = bucket.confidence;
    }

    {
        let db_guard = db.lock();
        let record_hash = insert_activity_tick(db_guard.conn(), &record)?;

        let payload = serde_json::json!({
            "tickId": tick_id,
            "sessionId": session.session_id,
            "bucketStart": record.bucket_start,
            "bucketEnd": record.bucket_end,
            "mouseEvents": bucket.mouse_events,
            "keyboardEvents": bucket.keyboard_events,
            "activityScoreConfidence": bucket.confidence,
            "automationFlags": bucket.automation_flags,
            "monotonicElapsedNs": monotonic_elapsed_ns,
            "wallClockAtTick": record.wall_clock_at_tick,
            "recordHash": record_hash,
        });
        SyncOutbox::enqueue(
            db_guard.conn(),
            ENTITY_ACTIVITY_TICK,
            &tick_id,
            payload,
            device_keys,
        )?;
    }

    Ok(())
}

pub(crate) fn capture_screenshot(
    db: &Arc<Mutex<Database>>,
    device_keys: &Arc<DeviceKeys>,
    screenshot: &Arc<Mutex<ScreenshotCapture>>,
    session: &ActiveSession,
) -> AgentResult<ScreenshotRecord> {
    let user_id = {
        let db_guard = db.lock();
        read_authenticated_user_id(&db_guard)?
    };

    let (width, height, image_bytes) = {
        let screenshot_guard = screenshot.lock();
        screenshot_guard.capture_pixels()?
    };

    let context = ScreenshotCaptureContext {
        user_id: &user_id,
        project_id: &session.project_id,
        task_id: session.task_id.as_deref(),
        session_id: &session.session_id,
        activity_tick_id: None,
    };

    let record = {
        let db_guard = db.lock();
        let screenshot_guard = screenshot.lock();
        screenshot_guard.persist_capture(
            db_guard.conn(),
            &context,
            width,
            height,
            &image_bytes,
        )?
    };

    let payload = serde_json::json!({
        "screenshotId": record.id,
        "userId": record.user_id,
        "projectId": record.project_id,
        "taskId": record.task_id,
        "sessionId": record.session_id,
        "capturedAt": record.captured_at,
        "sha256Hash": record.sha256_hash,
        "width": record.width,
        "height": record.height,
        "activityTickId": record.activity_tick_id,
        "blurApplied": record.blur_applied,
    });

    let db_guard = db.lock();
    SyncOutbox::enqueue(
        db_guard.conn(),
        ENTITY_SCREENSHOT,
        &record.id,
        payload,
        device_keys,
    )?;

    Ok(record)
}

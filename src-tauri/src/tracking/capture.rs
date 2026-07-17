use crate::activity::{apply_activity_confidence, compute_activity_score, ActivityTracker};
use crate::tracking_focus::{
    capture_active_window, close_active_app, close_active_site, extract_site_address,
    is_communication_app, open_tracking_app, open_tracking_site, should_track_active_window,
    ActiveWindowSample,
};
use crate::db::{Database, TIME_CATEGORY_ACTIVE, TIME_CATEGORY_INACTIVITY};
use crate::error::AgentResult;
use crate::tracking_inactivity::{TrackingInactivityController, TrackingInactivityPhase};
use crate::screenshot::{ScreenshotCapture, TrackingScreenshotCaptureContext, TrackingScreenshotRecord};
use crate::sync::{
    SyncOutbox, ENTITY_TRACKING_APP, ENTITY_TRACKING_PERIPHERAL_EVENT, ENTITY_TRACKING_SCREENSHOT,
    ENTITY_TRACKING_SITE,
};
use parking_lot::Mutex;
use std::sync::Arc;

use super::{ActiveTracking, TrackingTotals};

pub(crate) fn record_tracking_app_and_site(
    db: &Arc<Mutex<Database>>,
    tracking: &ActiveTracking,
    last_active_window: &Arc<Mutex<Option<ActiveWindowSample>>>,
    active_app_id: &Arc<Mutex<Option<String>>>,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
    inactivity_controller: &Arc<Mutex<Option<Arc<TrackingInactivityController>>>>,
) -> AgentResult<()> {
    let Some(sample) = capture_active_window() else {
        return Ok(());
    };

    if let Some(controller) = inactivity_controller.lock().clone() {
        controller.set_meeting_exempt(is_communication_app(&sample));
    }

    let app_changed = should_track_active_window(&sample)
        && {
            let last = last_active_window.lock();
            last.as_ref().is_none_or(|prev| {
                prev.app_name != sample.app_name || prev.window_title != sample.window_title
            })
        };

    let now = chrono::Utc::now().to_rfc3339();
    let db_guard = db.lock();

    if app_changed {
        if let Some(app_id) = active_app_id.lock().take() {
            if let Ok(app) = db_guard.get_tracking_app(&app_id) {
                close_active_app(db_guard.conn(), &app_id, &now)?;
                SyncOutbox::enqueue(
                    db_guard.conn(),
                    ENTITY_TRACKING_APP,
                    &app.id,
                    serde_json::json!({
                        "appId": app.id,
                        "trackingId": app.tracking_id,
                        "name": app.name,
                        "startedAt": app.started_at,
                        "endedAt": now,
                    }),
                )?;
            } else {
                close_active_app(db_guard.conn(), &app_id, &now)?;
            }
        }
        let app_id = open_tracking_app(db_guard.conn(), &tracking.tracking_id, &sample, &now)?;
        *active_app_id.lock() = Some(app_id);
    }

    record_site_focus(
        &db_guard,
        tracking,
        &sample,
        &now,
        active_site_id,
        last_site_address,
    )?;
    *last_active_window.lock() = Some(sample);

    Ok(())
}

fn record_site_focus(
    db_guard: &Database,
    tracking: &ActiveTracking,
    sample: &ActiveWindowSample,
    now: &str,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    let address = extract_site_address(sample);
    let site_changed = address.as_deref() != last_site_address.lock().as_deref();
    if !site_changed {
        return Ok(());
    }

    if let Some(site_id) = active_site_id.lock().take() {
        if let Ok(site) = db_guard.get_tracking_site(&site_id) {
            close_active_site(db_guard.conn(), &site_id, now)?;
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_SITE,
                &site.id,
                serde_json::json!({
                    "siteId": site.id,
                    "trackingId": site.tracking_id,
                    "address": site.address,
                    "startedAt": site.started_at,
                    "endedAt": now,
                }),
            )?;
        } else {
            close_active_site(db_guard.conn(), &site_id, now)?;
        }
    }

    if let Some(address) = address {
        let site_id =
            open_tracking_site(db_guard.conn(), &tracking.tracking_id, &address, now)?;
        *active_site_id.lock() = Some(site_id);
    }

    *last_site_address.lock() = extract_site_address(sample);
    Ok(())
}

pub(crate) fn screenshot_time_category(inactivity_phase: TrackingInactivityPhase) -> &'static str {
    match inactivity_phase {
        TrackingInactivityPhase::PausedInactivity | TrackingInactivityPhase::ResumePrompt
        | TrackingInactivityPhase::ManualPaused | TrackingInactivityPhase::ManualWorkCheck => TIME_CATEGORY_INACTIVITY,
        _ => TIME_CATEGORY_ACTIVE,
    }
}

pub(crate) fn capture_screenshot(
    db: &Arc<Mutex<Database>>,
    screenshot: &Arc<Mutex<ScreenshotCapture>>,
    tracker: &Arc<Mutex<ActivityTracker>>,
    totals: &Arc<Mutex<TrackingTotals>>,
    tracking: &ActiveTracking,
    period_start: &str,
    time_category: &str,
) -> AgentResult<TrackingScreenshotRecord> {
    let bucket = tracker.lock().drain_bucket();
    let raw_score = compute_activity_score(bucket.mouse_events, bucket.keyboard_events);
    let activity_score = apply_activity_confidence(raw_score, bucket.confidence);
    {
        let mut totals_guard = totals.lock();
        totals_guard.mouse_events += bucket.mouse_events;
        totals_guard.keyboard_events += bucket.keyboard_events;
        totals_guard.last_confidence = bucket.confidence;
        totals_guard.last_activity_score = activity_score;
    }

    let (width, height, image_bytes) = {
        let screenshot_guard = screenshot.lock();
        screenshot_guard.capture_pixels()?
    };

    let context = TrackingScreenshotCaptureContext {
        tracking_id: &tracking.tracking_id,
        period_start,
        time_category,
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

    let period_end = record.captured_at.clone();
    {
        let db_guard = db.lock();
        let peripheral_events = db_guard.flush_tracking_peripheral_events_for_period(
            &tracking.tracking_id,
            &record.original_id,
            period_start,
            &period_end,
            bucket.mouse_events,
            bucket.keyboard_events,
        )?;

        for (event_id, event_type) in peripheral_events {
            let count = match event_type.as_str() {
                "keyboard_activity" => bucket.keyboard_events as f64,
                _ => bucket.mouse_events as f64,
            };
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_PERIPHERAL_EVENT,
                &event_id,
                serde_json::json!({
                    "eventId": event_id,
                    "trackingId": tracking.tracking_id,
                    "event": event_type,
                    "count": count,
                    "screenshotOriginalId": record.original_id,
                    "startedAt": period_start,
                    "endedAt": period_end,
                }),
            )?;
        }
    }

    SyncOutbox::enqueue(
        db.lock().conn(),
        ENTITY_TRACKING_SCREENSHOT,
        &record.id,
        serde_json::json!({
            "screenshotId": record.id,
            "trackingId": record.tracking_id,
            "originalId": record.original_id,
            "capturedAt": record.captured_at,
            "filePath": record.file_path,
            "sha256Hash": record.sha256_hash,
            "width": record.width,
            "height": record.height,
            "blurApplied": record.blur_applied,
        }),
    )?;

    Ok(record)
}

/// Lightweight period drain: drains the tracker bucket and persists
/// peripheral events to SQLite, but does NOT capture a screenshot or
/// enqueue sync items. This is used on pause/stop to save activity
/// data without the expensive xcap capture + file I/O.
///
/// Callers should ensure the worker is stopped before this to avoid
/// lock contention on `screenshot` and `tracker`.
pub(crate) fn drain_activity_period(
    db: &Arc<Mutex<Database>>,
    tracker: &Arc<Mutex<ActivityTracker>>,
    totals: &Arc<Mutex<TrackingTotals>>,
    tracking: &ActiveTracking,
    period_start: &str,
) -> AgentResult<()> {
    let bucket = tracker.lock().drain_bucket();
    let raw_score = compute_activity_score(bucket.mouse_events, bucket.keyboard_events);
    let activity_score = apply_activity_confidence(raw_score, bucket.confidence);
    {
        let mut totals_guard = totals.lock();
        totals_guard.mouse_events += bucket.mouse_events;
        totals_guard.keyboard_events += bucket.keyboard_events;
        totals_guard.last_confidence = bucket.confidence;
        totals_guard.last_activity_score = activity_score;
    }

    let period_end = chrono::Utc::now().to_rfc3339();
    let db_guard = db.lock();
    db_guard.flush_tracking_peripheral_events_for_period(
        &tracking.tracking_id,
        "no-screenshot",
        period_start,
        &period_end,
        bucket.mouse_events,
        bucket.keyboard_events,
    )?;

    Ok(())
}

pub(crate) fn close_open_sites(
    db: &Arc<Mutex<Database>>,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    close_open_sites_inner(db, active_site_id, last_site_address)
}

fn close_open_sites_inner(
    db: &Arc<Mutex<Database>>,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let db_guard = db.lock();
    if let Some(site_id) = active_site_id.lock().take() {
        if let Ok(site) = db_guard.get_tracking_site(&site_id) {
            close_active_site(db_guard.conn(), &site_id, &now)?;
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_SITE,
                &site.id,
                serde_json::json!({
                    "siteId": site.id,
                    "trackingId": site.tracking_id,
                    "address": site.address,
                    "startedAt": site.started_at,
                    "endedAt": now,
                }),
            )?;
        } else {
            close_active_site(db_guard.conn(), &site_id, &now)?;
        }
    }
    *last_site_address.lock() = None;
    Ok(())
}

pub(crate) fn close_open_apps(
    db: &Arc<Mutex<Database>>,
    active_app_id: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    close_open_apps_inner(db, active_app_id)
}

fn close_open_apps_inner(
    db: &Arc<Mutex<Database>>,
    active_app_id: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let db_guard = db.lock();
    if let Some(app_id) = active_app_id.lock().take() {
        if let Ok(app) = db_guard.get_tracking_app(&app_id) {
            close_active_app(db_guard.conn(), &app_id, &now)?;
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_APP,
                &app.id,
                serde_json::json!({
                    "appId": app.id,
                    "trackingId": app.tracking_id,
                    "name": app.name,
                    "startedAt": app.started_at,
                    "endedAt": now,
                }),
            )?;
        } else {
            close_active_app(db_guard.conn(), &app_id, &now)?;
        }
    }
    Ok(())
}

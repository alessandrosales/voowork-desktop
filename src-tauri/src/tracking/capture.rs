use crate::activity::constants::activity_level_from_score;
use crate::activity::tracker::ActivityBucket;
use crate::activity::{apply_activity_confidence, compute_activity_score, ActivityTracker};
use crate::tracking_focus::{
    capture_active_window, close_active_app, close_active_site, extract_site_address,
    is_communication_app, open_tracking_app, open_tracking_site, should_track_active_window,
    ActiveWindowSample,
};
use crate::db::{Database, TIME_CATEGORY_ACTIVE, TIME_CATEGORY_INACTIVITY};
use crate::db::period_duration_seconds;
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

#[cfg(test)]
mod flush_tests {
    use super::*;
    use crate::db::Database;
    use crate::sync::ENTITY_TRACKING_PERIPHERAL_EVENT;

    use std::path::PathBuf;

    fn test_db_dir() -> PathBuf {
        PathBuf::from(std::env::temp_dir())
            .join(format!("voowork-a4-flush-test-{}", uuid::Uuid::new_v4()))
    }

    fn insert_tracking(conn: &rusqlite::Connection, id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO trackings (id, account_id, project_id, task_id, user_id, status, started_at, created_at, updated_at)
             VALUES (?1, 'acc', 'proj', 'task', 'user', 'active', ?2, ?2, ?2)",
            rusqlite::params![id, now],
        )
        .unwrap();
    }

    #[test]
    fn flush_activity_period_enqueues_events_without_screenshot() {
        let dir = test_db_dir();
        let db = Database::open(dir).unwrap();
        let db = Arc::new(Mutex::new(db));
        insert_tracking(db.lock().conn(), "t1");

        {
            let db_guard = db.lock();
            db_guard.flush_tracking_peripheral_events_for_period(
                "t1", "no-screenshot", "2000-01-01T00:00:00Z", "2000-01-01T00:01:00Z",
                5,
                3,
            ).unwrap();
        }

        let tracking = ActiveTracking {
            tracking_id: "t1".into(),
            project_id: "proj".into(),
            task_id: "task".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            last_screenshot_at: None,
            last_screenshot_hash: None,
            current_period_start: chrono::Utc::now().to_rfc3339(),
        };

        let bucket = ActivityBucket {
            mouse_events: 10,
            keyboard_events: 7,
            confidence: 1.0,
            positions: Vec::new(),
            automation_flags: 0,
        };

        let period_start = chrono::Utc::now().to_rfc3339();
        let period_end = chrono::Utc::now().to_rfc3339();

        flush_activity_period(&db, &tracking, &period_start, &period_end, &bucket, None).unwrap();

        let db_guard = db.lock();
        let count: i64 = db_guard.conn()
            .query_row(
                "SELECT COUNT(*) FROM sync_queue WHERE entity_type = ?1",
                rusqlite::params![ENTITY_TRACKING_PERIPHERAL_EVENT],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2, "should enqueue mouse + keyboard peripheral events");
    }
}

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
        let db_guard = db.lock();
        if let Err(err) = controller.set_meeting_exempt(is_communication_app(&sample), db_guard.conn()) {
            log::warn!("set_meeting_exempt failed: {err}");
        }
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

/// Determine time_category for a **finalization** screenshot (quit/stop).
///
/// Unlike `screenshot_time_category` (used during live tracking), this
/// considers actual user activity level alongside the inactivity phase.
/// The phase alone can be stale at quit time — e.g. `PausedInactivity`
/// or `ResumePrompt` from a previous idle burst, even though the user
/// just moved the mouse to click Quit.
///
/// Rule:
/// - `Active` / `Warning` / `Countdown` → always `active`
/// - `ManualPaused` / `ManualWorkCheck` → user explicitly paused; respect
///   intent UNLESS there is medium/high activity (user demonstrably working)
/// - `PausedInactivity` / `ResumePrompt` → system-detected idle; check
///   actual activity level to distinguish "genuinely away" from "back but
///   didn't dismiss the prompt"
pub(crate) fn finalize_screenshot_time_category(
    inactivity_phase: TrackingInactivityPhase,
    activity_level: &str,
) -> &'static str {
    // Billable phases → always active
    if matches!(
        inactivity_phase,
        TrackingInactivityPhase::Active
            | TrackingInactivityPhase::Warning
            | TrackingInactivityPhase::Countdown
    ) {
        return TIME_CATEGORY_ACTIVE;
    }

    // For inactivity phases: check actual activity level
    if activity_level == "none" || activity_level == "low" {
        TIME_CATEGORY_INACTIVITY
    } else {
        TIME_CATEGORY_ACTIVE
    }
}

pub(crate) struct CaptureOutcome {
    pub screenshot: Option<TrackingScreenshotRecord>,
    pub period_end: String,
}

pub(crate) fn flush_activity_period(
    db: &Arc<Mutex<Database>>,
    tracking: &ActiveTracking,
    period_start: &str,
    period_end: &str,
    bucket: &ActivityBucket,
    screenshot_original_id: Option<&str>,
) -> AgentResult<()> {
    let db_guard = db.lock();
    let screenshot_id_for_db = screenshot_original_id.unwrap_or("");
    let peripheral_events = db_guard.flush_tracking_peripheral_events_for_period(
        &tracking.tracking_id,
        screenshot_id_for_db,
        period_start,
        period_end,
        bucket.mouse_events,
        bucket.keyboard_events,
    )?;

    for (event_id, event_type) in peripheral_events {
        let count = match event_type.as_str() {
            "keyboard_activity" => bucket.keyboard_events as f64,
            _ => bucket.mouse_events as f64,
        };
        let mut event = serde_json::json!({
            "eventId": event_id,
            "trackingId": tracking.tracking_id,
            "event": event_type,
            "count": count,
            "startedAt": period_start,
            "endedAt": period_end,
        });
        if let Some(original_id) = screenshot_original_id {
            event["screenshotOriginalId"] = serde_json::json!(original_id);
        }
        SyncOutbox::enqueue(
            db_guard.conn(),
            ENTITY_TRACKING_PERIPHERAL_EVENT,
            &event_id,
            event,
        )?;
    }

    Ok(())
}

pub(crate) fn capture_screenshot(
    db: &Arc<Mutex<Database>>,
    screenshot: &Arc<Mutex<ScreenshotCapture>>,
    tracker: &Arc<Mutex<ActivityTracker>>,
    totals: &Arc<Mutex<TrackingTotals>>,
    tracking: &ActiveTracking,
    period_start: &str,
    time_category: &str,
) -> AgentResult<CaptureOutcome> {
    let bucket = tracker.lock().drain_bucket();
    let raw_score = compute_activity_score(bucket.mouse_events, bucket.keyboard_events);
    let activity_score = apply_activity_confidence(raw_score, bucket.confidence);
    let activity_level = activity_level_from_score(activity_score);
    {
        let mut totals_guard = totals.lock();
        totals_guard.mouse_events += bucket.mouse_events;
        totals_guard.keyboard_events += bucket.keyboard_events;
        totals_guard.last_confidence = bucket.confidence;
        totals_guard.last_activity_score = activity_score;
    }

    let capture_result = {
        let screenshot_guard = screenshot.lock();
        screenshot_guard.capture_pixels()
    };

    match capture_result {
        Ok((width, height, image_bytes)) => {
            let jpeg_quality = screenshot.lock().jpeg_quality();
            let hash_before_capture = {
                let (_, _, stored_bytes) = crate::screenshot::process_raw_rgba(
                    &image_bytes,
                    width,
                    height,
                    jpeg_quality,
                ).unwrap_or((width, height, image_bytes.clone()));
                crate::crypto::DeviceKeys::hash_bytes(&stored_bytes)
            };

            let is_duplicate = tracking.last_screenshot_hash.as_deref() == Some(&hash_before_capture);

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
                    is_duplicate,
                    activity_level,
                )?
            };

            let period_end = record.captured_at.clone();

            let duration_secs = period_duration_seconds(period_start, &period_end)?;
            {
                let mut t = totals.lock();
                t.screenshot_count += 1;
                if time_category == TIME_CATEGORY_INACTIVITY {
                    t.inactivity_seconds += duration_secs;
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
                    "isDuplicate": record.is_duplicate,
                    "activityLevel": record.activity_level,
                    "timeCategory": time_category,
                    "periodStartedAt": period_start,
                }),
            )?;

            flush_activity_period(db, tracking, period_start, &period_end, &bucket, Some(&record.original_id))?;

            Ok(CaptureOutcome {
                screenshot: Some(record),
                period_end,
            })
        }
        Err(err) => {
            log::warn!("screenshot capture failed: {err} — flushing activity without screenshot");
            let period_end = chrono::Utc::now().to_rfc3339();
            flush_activity_period(db, tracking, period_start, &period_end, &bucket, None)?;
            Ok(CaptureOutcome {
                screenshot: None,
                period_end,
            })
        }
    }
}

pub(crate) fn close_open_sites(
    db: &Arc<Mutex<Database>>,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    close_open_sites_at(db, active_site_id, last_site_address, &chrono::Utc::now().to_rfc3339())
}

pub(crate) fn close_open_sites_at(
    db: &Arc<Mutex<Database>>,
    active_site_id: &Arc<Mutex<Option<String>>>,
    last_site_address: &Arc<Mutex<Option<String>>>,
    ended_at: &str,
) -> AgentResult<()> {
    let db_guard = db.lock();
    if let Some(site_id) = active_site_id.lock().take() {
        if let Ok(site) = db_guard.get_tracking_site(&site_id) {
            close_active_site(db_guard.conn(), &site_id, ended_at)?;
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_SITE,
                &site.id,
                serde_json::json!({
                    "siteId": site.id,
                    "trackingId": site.tracking_id,
                    "address": site.address,
                    "startedAt": site.started_at,
                    "endedAt": ended_at,
                }),
            )?;
        } else {
            close_active_site(db_guard.conn(), &site_id, ended_at)?;
        }
    }
    *last_site_address.lock() = None;
    Ok(())
}

pub(crate) fn close_open_apps(
    db: &Arc<Mutex<Database>>,
    active_app_id: &Arc<Mutex<Option<String>>>,
) -> AgentResult<()> {
    close_open_apps_at(db, active_app_id, &chrono::Utc::now().to_rfc3339())
}

pub(crate) fn close_open_apps_at(
    db: &Arc<Mutex<Database>>,
    active_app_id: &Arc<Mutex<Option<String>>>,
    ended_at: &str,
) -> AgentResult<()> {
    let db_guard = db.lock();
    if let Some(app_id) = active_app_id.lock().take() {
        if let Ok(app) = db_guard.get_tracking_app(&app_id) {
            close_active_app(db_guard.conn(), &app_id, ended_at)?;
            SyncOutbox::enqueue(
                db_guard.conn(),
                ENTITY_TRACKING_APP,
                &app.id,
                serde_json::json!({
                    "appId": app.id,
                    "trackingId": app.tracking_id,
                    "name": app.name,
                    "startedAt": app.started_at,
                    "endedAt": ended_at,
                }),
            )?;
        } else {
            close_active_app(db_guard.conn(), &app_id, ended_at)?;
        }
    }
    Ok(())
}

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::db::TIME_CATEGORY_ACTIVE;
use crate::error::{AgentError, AgentResult};
use crate::sync::{finalize, SyncOutbox, ENTITY_TRACKING};
use crate::tracking_inactivity::TrackingInactivityPhase;

use super::capture::{close_open_apps, close_open_sites, flush_period_screenshot, screenshot_time_category};
use super::worker::{spawn_tracking_worker, TrackingWorkerContext};
use super::TrackingManager;

impl TrackingManager {
    pub(crate) fn abandon_active_tracking(&self, join_worker: bool) -> AgentResult<()> {
        self.activity_buffer.dismiss();

        let tracking = match self.active.lock().clone() {
            Some(tracking) => tracking,
            None => {
                self.clear_tracking_memory();
                return Ok(());
            }
        };

        log::info!(
            "finalizing tracking {} on {}",
            tracking.tracking_id,
            if join_worker { "shutdown" } else { "quit" }
        );
        let _ = self.flush_open_period(self.open_period_category());
        if join_worker {
            self.stop_worker();
        } else {
            self.stop_worker_without_join();
        }
        let _ = close_open_apps(&self.db, &self.active_app_id);
        let _ = close_open_sites(
            &self.db,
            &self.active_site_id,
            &self.last_site_address,
        );

        {
            let db = self.db.lock();
            finalize::finalize_tracking_remotely(&db, &tracking.tracking_id)?;
        }

        self.clear_tracking_memory();
        Ok(())
    }

    pub(crate) fn finalize_active_tracking(
        &self,
        clear_inactivity_controller: bool,
    ) -> AgentResult<()> {
        self.activity_buffer.dismiss();
        let tracking = self
            .active
            .lock()
            .clone()
            .ok_or_else(|| AgentError::Session("no active tracking".into()))?;

        let skip_period_flush = self
            .inactivity_controller
            .lock()
            .as_ref()
            .is_some_and(|controller| {
                matches!(
                    controller.snapshot().phase,
                    TrackingInactivityPhase::ManualPaused | TrackingInactivityPhase::ManualWorkCheck
                )
            });

        if !skip_period_flush {
            self.flush_open_period(self.open_period_category())?;
        }
        log::info!("pausing tracking {}", tracking.tracking_id);
        self.stop_worker();

        let _ = close_open_apps(&self.db, &self.active_app_id);
        let _ = close_open_sites(&self.db, &self.active_site_id, &self.last_site_address);
        if clear_inactivity_controller {
            *self.inactivity_controller.lock() = None;
        }

        let ended_at = chrono::Utc::now().to_rfc3339();
        {
            let db = self.db.lock();
            let controller = self.inactivity_controller.lock().clone();
            let elapsed =
                super::status_report::snapshot_task_elapsed(&db, &tracking, controller.as_deref())?;
            db.set_task_active_seconds(&tracking.task_id, elapsed)?;
            db.finalize_tracking(&tracking.tracking_id, &ended_at)?;

            SyncOutbox::enqueue(
                db.conn(),
                ENTITY_TRACKING,
                &tracking.tracking_id,
                serde_json::json!({
                    "trackingId": tracking.tracking_id,
                    "endedAt": ended_at,
                    "status": "inactive",
                }),
            )?;
        }

        *self.active.lock() = None;
        *self.tracking_active_flag.lock() = false;
        Ok(())
    }

    pub(crate) fn flush_open_period(&self, time_category: &str) -> AgentResult<()> {
        flush_period_screenshot(
            &self.db,
            &self.screenshot,
            &self.tracker,
            &self.totals,
            &self.active,
            time_category,
        )
    }

    pub(crate) fn open_period_category(&self) -> &'static str {
        self.inactivity_controller
            .lock()
            .as_ref()
            .map(|inactivity_controller| {
                screenshot_time_category(inactivity_controller.snapshot().phase)
            })
            .unwrap_or(TIME_CATEGORY_ACTIVE)
    }

    pub(crate) fn spawn_worker(&self) {
        self.worker_running.store(true, Ordering::SeqCst);
        let handle = spawn_tracking_worker(TrackingWorkerContext {
            worker_running: Arc::clone(&self.worker_running),
            active: Arc::clone(&self.active),
            tracker: Arc::clone(&self.tracker),
            db: Arc::clone(&self.db),
            screenshot: Arc::clone(&self.screenshot),
            totals: Arc::clone(&self.totals),
            last_active_window: Arc::clone(&self.last_active_window),
            active_app_id: Arc::clone(&self.active_app_id),
            active_site_id: Arc::clone(&self.active_site_id),
            last_site_address: Arc::clone(&self.last_site_address),
            inactivity_controller: Arc::clone(&self.inactivity_controller),
            app_handle: Arc::clone(&self.app_handle),
        });
        *self.worker_handle.lock() = Some(handle);
    }

    fn stop_worker(&self) {
        self.worker_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.lock().take() {
            std::thread::spawn(move || {
                let _ = handle.join();
            });
        }
    }

    fn stop_worker_without_join(&self) {
        self.worker_running.store(false, Ordering::SeqCst);
        let _ = self.worker_handle.lock().take();
    }

    fn clear_tracking_memory(&self) {
        *self.active.lock() = None;
        *self.tracking_active_flag.lock() = false;
        *self.inactivity_controller.lock() = None;
        *self.totals.lock() = super::TrackingTotals::default();
        *self.last_active_window.lock() = None;
        *self.active_app_id.lock() = None;
        *self.active_site_id.lock() = None;
        *self.last_site_address.lock() = None;
    }
}

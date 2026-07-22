use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{AgentError, AgentResult};
use crate::sync::{SyncOutbox, ENTITY_TRACKING};

use super::capture::{close_open_apps, close_open_sites};
use super::constants::WORKER_JOIN_TIMEOUT_SECS;
use super::worker::{spawn_tracking_worker, TrackingWorkerContext};
use super::TrackingManager;

impl TrackingManager {

    pub(super) fn finalize_active_tracking_inner(
        &self,
        clear_inactivity_controller: bool,
    ) -> AgentResult<()> {
        self.activity_buffer.dismiss();
        let tracking = self
            .active
            .lock()
            .clone()
            .ok_or_else(|| AgentError::Session("no active tracking".into()))?;

        self.stop_worker();

        let period_start = tracking.current_period_start.clone();
        let time_category = self
            .inactivity_controller
            .lock()
            .as_ref()
            .map(|ctrl| {
                let phase = ctrl.snapshot().phase;
                super::capture::screenshot_time_category(phase)
            })
            .unwrap_or(crate::db::TIME_CATEGORY_ACTIVE);

        super::capture::capture_screenshot(
            &self.db,
            &self.screenshot,
            &self.tracker,
            &self.totals,
            &tracking,
            &period_start,
            time_category,
            &self.last_active_window,
        )?;

        let _ = close_open_apps(&self.db, &self.active_app_id);
        let _ = close_open_sites(&self.db, &self.active_site_id, &self.last_site_address);

        let ended_at = chrono::Utc::now().to_rfc3339();
        {

            let controller = self.inactivity_controller.lock().clone();
            let db = self.db.lock();
            let elapsed = super::status_report::snapshot_task_elapsed(
                &db,
                &tracking,
                controller.as_deref(),
            )?;
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

        if clear_inactivity_controller {
            *self.inactivity_controller.lock() = None;
        }

        *self.active.lock() = None;
        self.tracking_active_flag.store(false, Ordering::SeqCst);
        Ok(())
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

    pub(super) fn stop_worker(&self) {
        self.worker_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.lock().take() {

            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            if rx
                .recv_timeout(Duration::from_secs(WORKER_JOIN_TIMEOUT_SECS))
                .is_err()
            {
                log::warn!(
                    "worker did not stop within {}s — proceeding anyway",
                    WORKER_JOIN_TIMEOUT_SECS
                );
            }
        }
    }
}

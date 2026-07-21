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
    /// Public wrapper that acquires the state-transition guard before
    /// delegating to the inner implementation.
    #[allow(dead_code)]
    pub(crate) fn finalize_active_tracking(
        &self,
        clear_inactivity_controller: bool,
    ) -> AgentResult<()> {
        let _guard = self.state_transition.lock();
        self.finalize_active_tracking_inner(clear_inactivity_controller)
    }

    /// Inner implementation — no lock on `state_transition`; callers must
    /// hold it if concurrency safety is required.
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

        // Stop worker FIRST to release any locks it holds (screenshot, db).
        self.stop_worker();

        // Capture final screenshot + flush activity period with real original_id.
        // The screenshot category (active vs inactivity) is determined by the
        // current inactivity phase. Screenshot capture must succeed (fail-loud).
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
        )?;

        let _ = close_open_apps(&self.db, &self.active_app_id);
        let _ = close_open_sites(&self.db, &self.active_site_id, &self.last_site_address);

        let ended_at = chrono::Utc::now().to_rfc3339();
        {
            // Usa o mesmo contador (controller de inatividade) que
            // `persist_task_time_snapshot_state`, evitando double-count:
            // no fluxo de troca de task o persist já gravou `base + session`
            // e resetou o controller, então aqui `billable_seconds() == 0` e
            // `elapsed == base`. Passar `None` cairia no fallback por
            // screenshots, somando o tempo ativo (e o ocioso) uma segunda vez.
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
            // Synchronous join with timeout — never block quit forever.
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

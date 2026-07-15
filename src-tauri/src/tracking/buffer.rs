use crate::db::Database;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const BUFFER_THRESHOLD_SECS: u64 = 120;
pub const BUFFER_INPUT_GAP_SECS: u64 = 60;

pub const SETTING_BUFFER_SECONDS: &str = "activity_buffer_seconds";
pub const SETTING_BUFFER_STARTED_AT: &str = "activity_buffer_started_at";

#[derive(Debug, Default, Clone)]
pub struct BufferSnapshot {
    pub seconds: u64,
    pub alert_pending: bool,
}

pub struct ActivityBuffer {
    db: Arc<Mutex<Database>>,
    running: Arc<AtomicBool>,
    seconds: Arc<Mutex<u64>>,
    alert_pending: Arc<Mutex<bool>>,
    started_at: Arc<Mutex<Option<String>>>,
    last_input_at: Arc<Mutex<Instant>>,
    eligible: Mutex<Option<Arc<AtomicBool>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ActivityBuffer {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        let buffer = Self {
            db,
            running: Arc::new(AtomicBool::new(false)),
            seconds: Arc::new(Mutex::new(0)),
            alert_pending: Arc::new(Mutex::new(false)),
            started_at: Arc::new(Mutex::new(None)),
            last_input_at: Arc::new(Mutex::new(Instant::now())),
            eligible: Mutex::new(None),
            handle: Mutex::new(None),
        };
        buffer.restore_from_db();
        buffer
    }

    fn restore_from_db(&self) {
        let db = self.db.lock();
        if let Ok(Some(value)) = db.get_setting(SETTING_BUFFER_SECONDS) {
            if let Ok(seconds) = value.parse::<u64>() {
                *self.seconds.lock() = seconds;
                if seconds >= BUFFER_THRESHOLD_SECS {
                    *self.alert_pending.lock() = true;
                }
            }
        }
        if let Ok(Some(value)) = db.get_setting(SETTING_BUFFER_STARTED_AT) {
            if !value.is_empty() {
                *self.started_at.lock() = Some(value);
            }
        }
    }

    fn persist_state(&self) {
        let seconds = *self.seconds.lock();
        let started_at = self.started_at.lock().clone().unwrap_or_default();
        let db = self.db.lock();
        let _ = db.set_setting(SETTING_BUFFER_SECONDS, &seconds.to_string());
        let _ = db.set_setting(SETTING_BUFFER_STARTED_AT, &started_at);
    }

    fn clear_persisted(&self) {
        let db = self.db.lock();
        let _ = db.set_setting(SETTING_BUFFER_SECONDS, "0");
        let _ = db.set_setting(SETTING_BUFFER_STARTED_AT, "");
    }

    fn is_eligible(&self) -> bool {
        self.eligible
            .lock()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    pub fn snapshot(&self) -> BufferSnapshot {
        if !self.is_eligible() {
            return BufferSnapshot::default();
        }

        BufferSnapshot {
            seconds: *self.seconds.lock(),
            alert_pending: *self.alert_pending.lock(),
        }
    }

    pub fn claim(&self) -> u64 {
        let seconds = *self.seconds.lock();
        self.clear();
        seconds
    }

    pub fn dismiss(&self) {
        self.clear();
    }

    pub fn clear(&self) {
        *self.seconds.lock() = 0;
        *self.alert_pending.lock() = false;
        *self.started_at.lock() = None;
        self.clear_persisted();
    }

    pub fn start_watcher(
        &self,
        tracking_active: Arc<Mutex<bool>>,
        input_at: Arc<Mutex<Instant>>,
        session_authenticated: Arc<AtomicBool>,
        buffer_eligible: Arc<AtomicBool>,
    ) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        *self.eligible.lock() = Some(Arc::clone(&buffer_eligible));

        let running = Arc::clone(&self.running);
        let seconds = Arc::clone(&self.seconds);
        let alert_pending = Arc::clone(&self.alert_pending);
        let started_at = Arc::clone(&self.started_at);
        let last_input_at = Arc::clone(&self.last_input_at);
        let eligible = Arc::clone(&buffer_eligible);
        let db = Arc::clone(&self.db);

        let handle = thread::spawn(move || {
            let buffer = ActivityBuffer {
                db,
                running,
                seconds,
                alert_pending,
                started_at,
                last_input_at,
                eligible: Mutex::new(Some(eligible)),
                handle: Mutex::new(None),
            };
            let mut last_tick_input = Instant::now();

            while buffer.running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));

                if !session_authenticated.load(Ordering::SeqCst) {
                    buffer.clear();
                    continue;
                }

                if *tracking_active.lock() {
                    continue;
                }

                if !buffer.is_eligible() {
                    continue;
                }

                let current_input = *input_at.lock();
                if current_input > last_tick_input {
                    *buffer.last_input_at.lock() = current_input;
                    last_tick_input = current_input;
                }

                let now = Instant::now();
                let gap = now.duration_since(*buffer.last_input_at.lock());
                if gap > Duration::from_secs(BUFFER_INPUT_GAP_SECS) {
                    buffer.clear();
                    continue;
                }

                if buffer.started_at.lock().is_none() {
                    *buffer.started_at.lock() = Some(chrono::Utc::now().to_rfc3339());
                }

                *buffer.seconds.lock() += 1;
                buffer.persist_state();

                if *buffer.seconds.lock() >= BUFFER_THRESHOLD_SECS {
                    *buffer.alert_pending.lock() = true;
                }
            }
        });

        *self.handle.lock() = Some(handle);
    }

}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use std::path::PathBuf;

    fn test_db() -> Arc<Mutex<Database>> {
        let dir = PathBuf::from(std::env::temp_dir()).join(format!(
            "voowork-buffer-test-{}",
            uuid::Uuid::new_v4()
        ));
        Arc::new(Mutex::new(Database::open(dir).unwrap()))
    }

    #[test]
    fn buffer_survives_restart() {
        let db = test_db();
        {
            let buffer = ActivityBuffer::new(Arc::clone(&db));
            *buffer.seconds.lock() = 90;
            *buffer.started_at.lock() = Some("2026-07-14T00:00:00Z".into());
            buffer.persist_state();
        }
        {
            let eligible = Arc::new(AtomicBool::new(true));
            let buffer = ActivityBuffer::new(db);
            *buffer.eligible.lock() = Some(eligible);
            assert_eq!(buffer.snapshot().seconds, 90);
            assert_eq!(
                buffer.started_at.lock().as_deref(),
                Some("2026-07-14T00:00:00Z")
            );
        }
    }

    #[test]
    fn snapshot_hidden_until_timer_started() {
        let db = test_db();
        let eligible = Arc::new(AtomicBool::new(false));
        let buffer = ActivityBuffer::new(Arc::clone(&db));
        *buffer.eligible.lock() = Some(Arc::clone(&eligible));
        *buffer.seconds.lock() = 180;
        *buffer.alert_pending.lock() = true;

        assert!(!buffer.snapshot().alert_pending);
        assert_eq!(buffer.snapshot().seconds, 0);

        eligible.store(true, Ordering::SeqCst);
        assert!(buffer.snapshot().alert_pending);
        assert_eq!(buffer.snapshot().seconds, 180);
    }

    #[test]
    fn claim_clears_persisted_buffer() {
        let db = test_db();
        let buffer = ActivityBuffer::new(Arc::clone(&db));
        *buffer.seconds.lock() = 45;
        buffer.persist_state();
        assert_eq!(buffer.claim(), 45);
        assert_eq!(buffer.snapshot().seconds, 0);

        let restored = ActivityBuffer::new(db);
        assert_eq!(restored.snapshot().seconds, 0);
    }
}

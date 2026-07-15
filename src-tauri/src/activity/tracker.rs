use super::automation::SampleBuffer;
use super::constants::{HARDWARE_LISTENER_POLL_MS, MAX_MOUSE_POSITIONS, SAMPLE_BUFFER_CAPACITY};
use parking_lot::Mutex;
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackerMode {
    Hardware,
}

#[derive(Debug, Clone, Default)]
pub struct ActivityBucket {
    pub mouse_events: u64,
    pub keyboard_events: u64,
    pub positions: Vec<(f64, f64)>,
    pub confidence: f64,
    pub automation_flags: u32,
}

pub struct ActivityTracker {
    running: Arc<AtomicBool>,
    bucket: Arc<Mutex<ActivityBucket>>,
    mode: Arc<Mutex<TrackerMode>>,
    last_input_at: Arc<Mutex<Instant>>,
    last_input_wall_at: Arc<Mutex<String>>,
    handle: Option<JoinHandle<()>>,
}

impl ActivityTracker {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            bucket: Arc::new(Mutex::new(ActivityBucket::default())),
            mode: Arc::new(Mutex::new(TrackerMode::Hardware)),
            last_input_at: Arc::new(Mutex::new(now)),
            last_input_wall_at: Arc::new(Mutex::new(chrono::Utc::now().to_rfc3339())),
            handle: None,
        }
    }

    pub fn last_input_at(&self) -> Arc<Mutex<Instant>> {
        Arc::clone(&self.last_input_at)
    }

    pub fn last_input_wall_at(&self) -> Arc<Mutex<String>> {
        Arc::clone(&self.last_input_wall_at)
    }

    fn touch_input(last_input_at: &Arc<Mutex<Instant>>, last_input_wall_at: &Arc<Mutex<String>>) {
        *last_input_at.lock() = Instant::now();
        *last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
    }

    pub fn mode(&self) -> TrackerMode {
        *self.mode.lock()
    }

    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        *self.bucket.lock() = ActivityBucket::default();
        *self.mode.lock() = TrackerMode::Hardware;
        Self::touch_input(&self.last_input_at, &self.last_input_wall_at);

        let running = Arc::clone(&self.running);
        let bucket = Arc::clone(&self.bucket);
        let mode = Arc::clone(&self.mode);
        let last_input_at = Arc::clone(&self.last_input_at);
        let last_input_wall_at = Arc::clone(&self.last_input_wall_at);

        let handle = thread::spawn(move || {
            let running_hw = Arc::clone(&running);
            let bucket_hw = Arc::clone(&bucket);
            let mode_hw = Arc::clone(&mode);
            let last_input_at_hw = Arc::clone(&last_input_at);
            let last_input_wall_at_hw = Arc::clone(&last_input_wall_at);

            let _hw_thread = thread::spawn(move || {
                let mut sample_buffer = SampleBuffer::new(SAMPLE_BUFFER_CAPACITY);
                let result = listen(move |event: Event| {
                    if !running_hw.load(Ordering::SeqCst) {
                        return;
                    }
                    *mode_hw.lock() = TrackerMode::Hardware;
                    ActivityTracker::touch_input(&last_input_at_hw, &last_input_wall_at_hw);
                    handle_event(&bucket_hw, &mut sample_buffer, event);
                });
                if let Err(err) = result {
                    log::error!(
                        "rdev listen failed: {err:?}. Mouse/keyboard capture requires Linux input group access (sudo usermod -aG input $USER)."
                    );
                }
            });

            while running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(HARDWARE_LISTENER_POLL_MS));
            }
        });

        self.handle = Some(handle);
    }

    pub fn drain_bucket(&self) -> ActivityBucket {
        let mut guard = self.bucket.lock();
        let drained = guard.clone();
        *guard = ActivityBucket::default();
        drained
    }
}

fn handle_event(bucket: &Arc<Mutex<ActivityBucket>>, sample_buffer: &mut SampleBuffer, event: Event) {
    let mut guard = bucket.lock();
    match event.event_type {
        EventType::MouseMove { x, y } => {
            guard.mouse_events += 1;
            if guard.positions.len() < MAX_MOUSE_POSITIONS {
                guard.positions.push((x, y));
            }
            sample_buffer.push(Some((x, y)));
        }
        EventType::ButtonPress(_) | EventType::ButtonRelease(_) | EventType::Wheel { .. } => {
            guard.mouse_events += 1;
            sample_buffer.push(None);
        }
        EventType::KeyPress(key) | EventType::KeyRelease(key) => {
            if !is_modifier_key(key) {
                guard.keyboard_events += 1;
                sample_buffer.push(None);
            }
        }
    }

    let analysis = sample_buffer.analyze();
    guard.confidence = analysis.confidence;
    guard.automation_flags = analysis.flags;
}

fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::MetaLeft
            | Key::MetaRight
            | Key::CapsLock
    )
}

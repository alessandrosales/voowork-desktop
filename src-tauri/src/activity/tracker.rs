use super::automation::SampleBuffer;
use super::constants::{
    HARDWARE_LISTENER_POLL_MS, HARDWARE_PROBE_SECS, MAX_MOUSE_POSITIONS,
    SAMPLE_BUFFER_CAPACITY, SIMULATED_CONFIDENCE, SIMULATED_TICK_MS,
};
use parking_lot::Mutex;
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackerMode {
    Hardware,
    Simulated,
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
            mode: Arc::new(Mutex::new(TrackerMode::Simulated)),
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
        *self.mode.lock() = TrackerMode::Simulated;
        Self::touch_input(&self.last_input_at, &self.last_input_wall_at);

        let running = Arc::clone(&self.running);
        let bucket = Arc::clone(&self.bucket);
        let mode = Arc::clone(&self.mode);
        let last_input_at = Arc::clone(&self.last_input_at);
        let last_input_wall_at = Arc::clone(&self.last_input_wall_at);

        let handle = thread::spawn(move || {
            let hardware_used = Arc::new(AtomicBool::new(false));
            let hw_flag = Arc::clone(&hardware_used);
            let bucket_hw = Arc::clone(&bucket);
            let running_hw = Arc::clone(&running);
            let running_wait = Arc::clone(&running);
            let mode_hw = Arc::clone(&mode);
            let last_input_at_hw = Arc::clone(&last_input_at);
            let last_input_wall_at_hw = Arc::clone(&last_input_wall_at);
            let running_sim = Arc::clone(&running);
            let bucket_sim = Arc::clone(&bucket);

            let _hw_thread = thread::spawn(move || {
                let mut sample_buffer = SampleBuffer::new(SAMPLE_BUFFER_CAPACITY);
                let result = listen(move |event: Event| {
                    if !running_hw.load(Ordering::SeqCst) {
                        return;
                    }
                    hw_flag.store(true, Ordering::SeqCst);
                    *mode_hw.lock() = TrackerMode::Hardware;
                    ActivityTracker::touch_input(&last_input_at_hw, &last_input_wall_at_hw);
                    handle_event(&bucket_hw, &mut sample_buffer, event);
                });
                if result.is_err() {
                    log::warn!("rdev listen failed: {:?}", result);
                }
            });

            thread::sleep(Duration::from_secs(HARDWARE_PROBE_SECS));
            if !hardware_used.load(Ordering::SeqCst) {
                log::info!("hardware tracker unavailable, using simulated mode");
                simulated_listener(running_sim, bucket_sim);
            } else {
                // Keep the listener alive until running=false; don't join a blocking rdev thread.
                while running_wait.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(HARDWARE_LISTENER_POLL_MS));
                }
            }
        });

        self.handle = Some(handle);
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // rdev::listen blocks indefinitely — detach instead of joining to avoid freezing stop_session.
        if let Some(handle) = self.handle.take() {
            drop(handle);
        }
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

fn simulated_listener(running: Arc<AtomicBool>, bucket: Arc<Mutex<ActivityBucket>>) {
    let mut rng_state = 0u64;
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(SIMULATED_TICK_MS));
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);

        let mut guard = bucket.lock();
        if rng_state % 3 == 0 {
            guard.keyboard_events += 1;
        } else {
            guard.mouse_events += 1;
            let x = (rng_state % 1920) as f64;
            let y = ((rng_state >> 16) % 1080) as f64;
            if guard.positions.len() < MAX_MOUSE_POSITIONS {
                guard.positions.push((x, y));
            }
        }
        guard.confidence = SIMULATED_CONFIDENCE;
    }
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

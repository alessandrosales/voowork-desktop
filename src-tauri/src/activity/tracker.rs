use super::automation::SampleBuffer;
use super::constants::{HARDWARE_LISTENER_POLL_MS, MAX_MOUSE_POSITIONS, SAMPLE_BUFFER_CAPACITY};
use super::platform;
use crate::tracking_inactivity::DEFAULT_INACTIVITY_THRESHOLD_MINUTES;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// ActivityTracker
// ---------------------------------------------------------------------------

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
    /// true se a permissão de Input Monitoring foi verificada (só macOS).
    permission_granted: Arc<AtomicBool>,
    last_mouse_pos: Arc<Mutex<Option<(f64, f64)>>>,
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
            permission_granted: Arc::new(AtomicBool::new(true)),
            last_mouse_pos: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&self, _handle: tauri::AppHandle) {
        // Sem uso por enquanto (eventos são via polling, não CGEventTap)
    }

    /// Retorna `true` se a permissão de Input Monitoring foi verificada.
    /// No macOS 14+, usa `CGPreflightListenEventAccess`.
    pub fn is_permission_granted(&self) -> bool {
        self.permission_granted.load(Ordering::SeqCst)
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

        // Verifica permissão no macOS (apenas informativo)
        let granted = platform::check_permission();
        self.permission_granted.store(granted, Ordering::SeqCst);
        if !granted {
            log::warn!(
                "Input Monitoring permission not granted. \
                 macOS: System Settings → Privacy & Security → Input Monitoring → enable Voowork"
            );
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
        let last_mouse_pos = Arc::clone(&self.last_mouse_pos);

        let handle = thread::spawn(move || {
            let mut sample_buffer = SampleBuffer::new(SAMPLE_BUFFER_CAPACITY);

            while running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(HARDWARE_LISTENER_POLL_MS));
                let now = Instant::now();

                // Poll posição do mouse (macOS)
                if let Some(pos) = platform::poll_mouse_position() {
                    let mut bucket_guard = bucket.lock();
                    let mut last_pos_guard = last_mouse_pos.lock();

                    let is_new = match *last_pos_guard {
                        Some((lx, ly)) => (pos.0 - lx).abs() > 1.0 || (pos.1 - ly).abs() > 1.0,
                        None => true,
                    };

                    if is_new {
                        bucket_guard.mouse_events += 1;
                        if bucket_guard.positions.len() < MAX_MOUSE_POSITIONS {
                            bucket_guard.positions.push(pos);
                        }
                        *mode.lock() = TrackerMode::Hardware;
                        Self::touch_input(&last_input_at, &last_input_wall_at);
                        sample_buffer.push(Some(pos));
                    }

                    *last_pos_guard = Some(pos);

                    // Análise anti-automação
                    let analysis = sample_buffer.analyze();
                    bucket_guard.confidence = analysis.confidence;
                    bucket_guard.automation_flags = analysis.flags;
                }

                // A12(a): no macOS, seconds_since_last_input usa
                // kCGAnyInputEventType (teclado + mouse), inflando
                // keyboard_events. Ideal seria kCGEventKeyboardEventType
                // para métrica separada, mas a API atual não distingue.
                // Pendente: implementar para macOS quando disponível.
                let secs = platform::seconds_since_last_input();

                if secs.is_finite() && secs < (HARDWARE_LISTENER_POLL_MS as f64 / 1000.0) * 2.0 {
                    let mut bucket_guard = bucket.lock();
                    bucket_guard.keyboard_events += 1;
                    *mode.lock() = TrackerMode::Hardware;
                    Self::touch_input(&last_input_at, &last_input_wall_at);
                    sample_buffer.push(None);

                    let analysis = sample_buffer.analyze();
                    bucket_guard.confidence = analysis.confidence;
                    bucket_guard.automation_flags = analysis.flags;
                } else if !secs.is_finite() || secs > 1_000_000_000.0 {
                    // A API do SO não consegue determinar o tempo desde
                    // o último input (ex: macOS sem permissão de Input
                    // Monitoring retorna f64::MAX/kCGNever). Neste caso
                    // não podemos detectar teclado — apenas mouse.
                    //
                    // Usamos um heartbeat curto (threshold de inatividade)
                    // para dar ao usuário que só digita um buffer antes
                    // de entrar em inatividade, sem fingir input para
                    // sempre. Após o heartbeat, a inatividade dispara
                    // normalmente pela falta de movimento do mouse.
                    let elapsed = last_input_at.lock().elapsed();
                    let heartbeat_secs = Duration::from_secs(
                        DEFAULT_INACTIVITY_THRESHOLD_MINUTES * 60,
                    );
                    if elapsed >= heartbeat_secs {
                        Self::touch_input(&last_input_at, &last_input_wall_at);
                    }
                }

                let _ = now; // usado implicitamente
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

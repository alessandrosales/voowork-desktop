use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ClockSnapshot {
    pub instant: Instant,
    pub wall_clock_ms: i64,
}

impl ClockSnapshot {
    pub fn now() -> Self {
        Self {
            instant: Instant::now(),
            wall_clock_ms: system_time_millis(),
        }
    }
}

pub fn system_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone, Default)]
pub struct ClockMonitor {
    baseline: Option<ClockSnapshot>,
    skew_events: u32,
}

impl ClockMonitor {
    pub fn new() -> Self {
        Self {
            baseline: Some(ClockSnapshot::now()),
            skew_events: 0,
        }
    }

    /// Compares monotonic elapsed time vs wall-clock delta.
    /// Returns true if a significant skew was detected (possible manual clock change).
    pub fn check_skew(&mut self, tolerance: Duration) -> bool {
        let current = ClockSnapshot::now();
        let Some(ref baseline) = self.baseline else {
            self.baseline = Some(current);
            return false;
        };

        let mono_delta = current.instant.duration_since(baseline.instant);
        let wall_delta_ms = current.wall_clock_ms - baseline.wall_clock_ms;
        let wall_delta = Duration::from_millis(wall_delta_ms.max(0) as u64);

        let diff = if mono_delta > wall_delta {
            mono_delta - wall_delta
        } else {
            wall_delta - mono_delta
        };

        if diff > tolerance {
            self.skew_events += 1;
            self.baseline = Some(current);
            return true;
        }

        false
    }

    pub fn skew_events(&self) -> u32 {
        self.skew_events
    }

    pub fn reset(&mut self) {
        self.baseline = Some(ClockSnapshot::now());
        self.skew_events = 0;
    }
}

pub fn monotonic_ns_since(start: Instant) -> i64 {
    start.elapsed().as_nanos() as i64
}

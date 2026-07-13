use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct EventSample {
    pub delta_ms: u64,
    pub position: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct AutomationAnalysis {
    pub confidence: f64,
    pub flags: u32,
}

const FLAG_REGULAR_INTERVALS: u32 = 1;
const FLAG_IDENTICAL_POSITIONS: u32 = 2;
const FLAG_LOW_VARIANCE: u32 = 4;

pub fn analyze_samples(samples: &[EventSample]) -> AutomationAnalysis {
    if samples.len() < 5 {
        return AutomationAnalysis {
            confidence: 1.0,
            flags: 0,
        };
    }

    let mut flags = 0u32;
    let mut confidence = 1.0f64;

    let deltas: Vec<u64> = samples.iter().map(|s| s.delta_ms).collect();
    if deltas.len() >= 3 {
        let first = deltas[0];
        let all_same = deltas.iter().all(|d| (*d as i64 - first as i64).abs() <= 2);
        if all_same && first > 0 {
            flags |= FLAG_REGULAR_INTERVALS;
            confidence -= 0.3;
        }
    }

    let positions: Vec<(f64, f64)> = samples
        .iter()
        .filter_map(|s| s.position)
        .collect();

    if positions.len() >= 3 {
        let all_same_pos = positions.windows(2).all(|w| {
            (w[0].0 - w[1].0).abs() < 0.5 && (w[0].1 - w[1].1).abs() < 0.5
        });
        if all_same_pos {
            flags |= FLAG_IDENTICAL_POSITIONS;
            confidence -= 0.25;
        }
    }

    if deltas.len() >= 5 {
        let mean = deltas.iter().map(|d| *d as f64).sum::<f64>() / deltas.len() as f64;
        let variance = deltas
            .iter()
            .map(|d| {
                let diff = *d as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / deltas.len() as f64;

        if variance < 1.0 && mean > 0.0 {
            flags |= FLAG_LOW_VARIANCE;
            confidence -= 0.2;
        }
    }

    AutomationAnalysis {
        confidence: confidence.clamp(0.1, 1.0),
        flags,
    }
}

pub struct SampleBuffer {
    max_size: usize,
    samples: VecDeque<EventSample>,
    last_event_at: Option<std::time::Instant>,
}

impl SampleBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            samples: VecDeque::with_capacity(max_size),
            last_event_at: None,
        }
    }

    pub fn push(&mut self, position: Option<(f64, f64)>) {
        let now = std::time::Instant::now();
        let delta_ms = self
            .last_event_at
            .map(|t| now.duration_since(t).as_millis() as u64)
            .unwrap_or(0);
        self.last_event_at = Some(now);

        if self.samples.len() >= self.max_size {
            self.samples.pop_front();
        }
        self.samples.push_back(EventSample { delta_ms, position });
    }

    pub fn analyze(&self) -> AutomationAnalysis {
        analyze_samples(
            &self.samples.iter().cloned().collect::<Vec<_>>(),
        )
    }
}

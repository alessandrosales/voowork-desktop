pub mod automation;
pub mod constants;
pub mod platform;
pub mod tracker;

pub use automation::{apply_activity_confidence, compute_activity_score};
pub use tracker::{ActivityTracker, TrackerMode};

pub fn tracker_mode_label(mode: TrackerMode) -> &'static str {
    match mode {
        TrackerMode::Hardware => "hardware",
    }
}

pub mod automation;
pub mod constants;
pub mod tracker;

pub use tracker::{ActivityTracker, TrackerMode};

pub fn tracker_mode_label(mode: TrackerMode) -> &'static str {
    match mode {
        TrackerMode::Hardware => "hardware",
        TrackerMode::Simulated => "simulated",
    }
}

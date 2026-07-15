mod constants;
mod persistence;
mod state;

pub use constants::{
    COUNTDOWN_SECS, DEFAULT_INACTIVITY_THRESHOLD_MINUTES, SETTING_INACTIVITY_PROFILE,
    SETTING_INACTIVITY_THRESHOLD_MINUTES,
};
pub use persistence::load_inactivity_threshold_minutes;
pub use state::{TrackingInactivityController, TrackingInactivityPhase, TrackingInactivitySnapshot};

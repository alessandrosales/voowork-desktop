mod constants;
mod persistence;
mod state;

pub use constants::{
    COUNTDOWN_SECS, DEFAULT_IDLE_THRESHOLD_MINUTES, SETTING_PROFILE, SETTING_THRESHOLD_MINUTES,
};
pub use persistence::load_idle_threshold_minutes;
pub use state::{IdleController, IdlePhase, IdleSnapshot};

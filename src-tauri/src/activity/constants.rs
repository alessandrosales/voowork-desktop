
pub const ACTIVITY_SCORE_THRESHOLD: u64 = 500;

pub const ACTIVITY_LEVEL_LOW_MAX: u8 = 33;
pub const ACTIVITY_LEVEL_MED_MAX: u8 = 66;

pub const SAMPLE_BUFFER_CAPACITY: usize = 32;
pub const MAX_MOUSE_POSITIONS: usize = 20;
pub const HARDWARE_LISTENER_POLL_MS: u64 = 200;

pub fn activity_level_from_score(score: u8) -> &'static str {
    if score <= ACTIVITY_LEVEL_LOW_MAX {
        "low"
    } else if score <= ACTIVITY_LEVEL_MED_MAX {
        "medium"
    } else {
        "high"
    }
}

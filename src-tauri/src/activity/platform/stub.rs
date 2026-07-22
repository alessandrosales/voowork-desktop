

pub fn poll_mouse_position() -> Option<(f64, f64)> {
    None
}

pub fn seconds_since_last_input() -> f64 {
    f64::MAX
}

pub fn check_permission() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_poll_mouse_position() {
        assert_eq!(poll_mouse_position(), None);
    }

    #[test]
    fn test_stub_seconds_since_last_input() {
        assert_eq!(seconds_since_last_input(), f64::MAX);
    }

    #[test]
    fn test_stub_check_permission() {
        assert!(check_permission());
    }
}

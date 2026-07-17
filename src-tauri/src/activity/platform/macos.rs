// ---------------------------------------------------------------------------
// macOS: polling-based activity detection
//
// Uses CoreGraphics polling APIs (NO CGEventTap / global hooks).
//   - CGEventGetLocation() — current mouse position
//   - CGEventSourceSecondsSinceLastEventType() — idle time since last input
//   - CGPreflightListenEventAccess() — Input Monitoring permission check
//
// No special permissions required for polling APIs.
// ---------------------------------------------------------------------------

use core_graphics::geometry::CGPoint;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGEventGetLocation(event: *mut std::ffi::c_void) -> CGPoint;
    fn CFRelease(cf: *mut std::ffi::c_void);
    fn CGEventSourceSecondsSinceLastEventType(source_state_id: i32, event_type: u32) -> f64;
}

const CG_EVENT_SOURCE_STATE_PRIVATE: i32 = -1;
/// kCGAnyInputEventType = all input event masks (0xFFFFFFFF)
const CG_ANY_INPUT_EVENT_TYPE: u32 = !0u32;

/// Returns the current mouse cursor position via CoreGraphics.
pub fn poll_mouse_position() -> Option<(f64, f64)> {
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        Some((point.x, point.y))
    }
}

/// Returns seconds since the last input event (mouse or keyboard).
/// Small values (< 0.5s) indicate recent activity.
pub fn seconds_since_last_input() -> f64 {
    unsafe { CGEventSourceSecondsSinceLastEventType(CG_EVENT_SOURCE_STATE_PRIVATE, CG_ANY_INPUT_EVENT_TYPE) }
}

/// Checks if Input Monitoring permission is granted (macOS 14+).
/// Polling APIs still work without this permission; it's informational.
pub fn check_permission() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightListenEventAccess() -> u8;
    }
    unsafe { CGPreflightListenEventAccess() != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_seconds_since_last_input_is_not_negative() {
        let idle = seconds_since_last_input();
        assert!(idle >= 0.0 || idle == f64::MAX);
    }

    #[test]
    fn test_macos_poll_mouse_position_type() {
        // Just verify the function exists and returns the right type signature.
        let _result: Option<(f64, f64)> = poll_mouse_position();
    }
}

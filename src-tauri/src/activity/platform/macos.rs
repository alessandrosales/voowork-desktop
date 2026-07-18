// ---------------------------------------------------------------------------
// macOS: polling-based activity detection
//
// Uses CoreGraphics polling APIs (NO CGEventTap / global hooks).
//
//   Input detection (seconds_since_last_input):
//     CGEventSourceSecondsSinceLastEventType() with
//     kCGEventSourceStateCombinedSessionState (0) — queries global input
//     state across ALL processes in the current login session.
//     Detects ANY input event (mouse clicks, keyboard presses, scroll,
//     tablet) regardless of which app has focus.
//
//     IMPORTANT: We use CombinedSessionState instead of Private (-1).
//     Private state tracks events only for the calling process. Since
//     Voowork runs in the background (menubar + mini-timer) and the
//     polling happens on a background thread (not the main run loop),
//     Private state would NEVER see input events, making ALL activity
//     detection impossible.
//
//     On macOS 14+ without Input Monitoring permission, this function
//     returns sentinel values (f64::MAX / kCGNever ≈ 1e20). The sentinel
//     handler in tracker.rs injects a 15s heartbeat to prevent false
//     inactivity alerts when the user is typing without moving the mouse.
//
//   Mouse position (poll_mouse_position):
//     CGEventCreate(NULL) + CGEventGetLocation() — queries current cursor
//     coordinates. This is best-effort: if CGEventCreate returns NULL (can
//     happen on macOS 14+ with hardened runtime when Input Monitoring is
//     not recognized), we return None and the tracker skips mouse-movement
//     counting. The user is still detected as active via seconds_since_last_input.
//
//   Permission check (check_permission):
//     CGPreflightListenEventAccess() — macOS 14+ only. Resolved via dlsym
//     to avoid crashing on macOS 13 where the symbol doesn't exist.
// ---------------------------------------------------------------------------

use core_graphics::geometry::CGPoint;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CFRelease(cf: *mut c_void);
    fn CGEventSourceSecondsSinceLastEventType(source_state_id: i32, event_type: u32) -> f64;
}

/// kCGEventSourceStateCombinedSessionState — global input state across all
/// processes in the current login session. This is the correct choice for
/// a background activity polling thread.
const CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;

/// kCGAnyInputEventType = all input events combined (0xFFFFFFFF)
const CG_ANY_INPUT_EVENT_TYPE: u32 = !0u32;

// ---------------------------------------------------------------------------
// Permission check (CGPreflightListenEventAccess, macOS 14+)
// Resolved via dlsym to avoid crash on macOS 13.
// ---------------------------------------------------------------------------

static CG_PREFLIGHT_CHECKED: AtomicBool = AtomicBool::new(false);
static CG_PREFLIGHT_AVAILABLE: AtomicBool = AtomicBool::new(false);
static mut CG_PREFLIGHT_FN: Option<
    unsafe extern "C" fn() -> u8,
> = None;

/// Attempt to resolve CGPreflightListenEventAccess at runtime.
/// Runs ONCE, caches the result permanently.
/// Returns true if the symbol was found (macOS 14+), false otherwise.
fn resolve_cg_preflight() -> bool {
    // Fast path: already checked
    if CG_PREFLIGHT_CHECKED.load(Ordering::SeqCst) {
        return CG_PREFLIGHT_AVAILABLE.load(Ordering::SeqCst);
    }

    // dlsym from CoreGraphics framework
    let handle = unsafe {
        libc::dlopen(
            c"/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics".as_ptr(),
            libc::RTLD_LAZY | libc::RTLD_NOLOAD,
        )
    };
    let handle = if handle.is_null() {
        // CoreGraphics isn't loaded yet (shouldn't happen on macOS) —
        // fall back to RTLD_DEFAULT to search all loaded images.
        libc::RTLD_DEFAULT
    } else {
        handle
    };

    let symbol = unsafe {
        libc::dlsym(
            handle,
            c"CGPreflightListenEventAccess".as_ptr(),
        )
    };

    let found = !symbol.is_null();

    if found {
        unsafe {
            CG_PREFLIGHT_FN = Some(
                std::mem::transmute::<*mut libc::c_void, unsafe extern "C" fn() -> u8>(
                    symbol,
                ),
            );
        }
        CG_PREFLIGHT_AVAILABLE.store(true, Ordering::SeqCst);
    }

    CG_PREFLIGHT_CHECKED.store(true, Ordering::SeqCst);
    found
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns the current mouse cursor position via CoreGraphics.
///
/// Uses `CGEventCreate(NULL) + CGEventGetLocation()`. This is the only way
/// to query cursor position from a background thread without AppKit.
///
/// On macOS 14+ with hardened runtime, `CGEventCreate()` may return NULL
/// if Input Monitoring permission has not been granted. In that case we
/// return `None` and the tracker skips mouse-movement counting — the user
/// is still detected as active via `seconds_since_last_input()`.
pub fn poll_mouse_position() -> Option<(f64, f64)> {
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);

        if point.x.is_finite() && point.y.is_finite() {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

/// Returns seconds since the last input event (mouse or keyboard) in the
/// ENTIRE user session (all processes). Small values (< 0.5s) indicate
/// recent activity.
///
/// Uses `kCGEventSourceStateCombinedSessionState` (0) so that ANY input
/// event in the session (mouse click, keypress, scroll in any app) is
/// detected — not just events sent to Voowork.
pub fn seconds_since_last_input() -> f64 {
    unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CG_EVENT_SOURCE_STATE_COMBINED_SESSION,
            CG_ANY_INPUT_EVENT_TYPE,
        )
    }
}

/// Checks if Input Monitoring permission is granted (macOS 14+).
///
/// On macOS 13 and earlier, this function is unavailable and we return
/// `true` (the polling APIs work without Input Monitoring on those
/// versions). On macOS 14+, the permission IS required for
/// `CGEventSourceSecondsSinceLastEventType` to return real values
/// instead of sentinel constants.
///
/// Uses `dlsym` internally to avoid crashing on macOS 13 where the
/// `CGPreflightListenEventAccess` symbol does not exist.
pub fn check_permission() -> bool {
    if !resolve_cg_preflight() {
        // macOS < 14 — polling APIs work without Input Monitoring
        return true;
    }

    unsafe {
        if let Some(func) = CG_PREFLIGHT_FN {
            func() != 0
        } else {
            true
        }
    }
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

    #[test]
    fn test_permission_check_does_not_crash() {
        // Must not crash on any macOS version — this was a latent bug.
        let _granted = check_permission();
    }
}



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

const CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;

const CG_ANY_INPUT_EVENT_TYPE: u32 = !0u32;

static CG_PREFLIGHT_CHECKED: AtomicBool = AtomicBool::new(false);
static CG_PREFLIGHT_AVAILABLE: AtomicBool = AtomicBool::new(false);
static mut CG_PREFLIGHT_FN: Option<
    unsafe extern "C" fn() -> u8,
> = None;

fn resolve_cg_preflight() -> bool {

    if CG_PREFLIGHT_CHECKED.load(Ordering::SeqCst) {
        return CG_PREFLIGHT_AVAILABLE.load(Ordering::SeqCst);
    }

    let handle = unsafe {
        libc::dlopen(
            c"/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics".as_ptr(),
            libc::RTLD_LAZY | libc::RTLD_NOLOAD,
        )
    };
    let handle = if handle.is_null() {

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

pub fn seconds_since_last_input() -> f64 {
    unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CG_EVENT_SOURCE_STATE_COMBINED_SESSION,
            CG_ANY_INPUT_EVENT_TYPE,
        )
    }
}

pub fn check_permission() -> bool {
    if !resolve_cg_preflight() {

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

        let _result: Option<(f64, f64)> = poll_mouse_position();
    }

    #[test]
    fn test_permission_check_does_not_crash() {

        let _granted = check_permission();
    }
}

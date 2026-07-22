

use std::mem::MaybeUninit;

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct LASTINPUTINFO {
    cb_size: u32,
    dw_time: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lp_point: *mut POINT) -> i32;
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
}

pub fn poll_mouse_position() -> Option<(f64, f64)> {
    unsafe {
        let mut pt = MaybeUninit::<POINT>::uninit();
        if GetCursorPos(pt.as_mut_ptr()) != 0 {
            let pt = pt.assume_init();
            Some((pt.x as f64, pt.y as f64))
        } else {
            None
        }
    }
}

pub fn seconds_since_last_input() -> f64 {
    unsafe {
        let mut info = MaybeUninit::<LASTINPUTINFO>::uninit();
        (*info.as_mut_ptr()).cb_size = size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(info.as_mut_ptr()) != 0 {
            let tick = (*info.as_ptr()).dw_time;
            let current = GetTickCount();

            let elapsed = current.wrapping_sub(tick);
            elapsed as f64 / 1000.0
        } else {
            f64::MAX
        }
    }
}

pub fn check_permission() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_seconds_since_last_input_is_not_negative() {
        let idle = seconds_since_last_input();
        assert!(idle >= 0.0 || idle == f64::MAX);
    }

    #[test]
    fn test_windows_check_permission() {
        assert!(check_permission());
    }
}

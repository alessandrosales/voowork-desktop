

use std::sync::OnceLock;

#[allow(non_camel_case_types)]
type Display = std::ffi::c_void;
type Window = u64;
type Bool = i32;

struct X11Display(*mut Display);
unsafe impl Send for X11Display {}
unsafe impl Sync for X11Display {}

impl Drop for X11Display {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                XCloseDisplay(self.0);
            }
        }
    }
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(display_name: *const std::os::raw::c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display);
    fn XDefaultRootWindow(display: *mut Display) -> Window;
    fn XQueryPointer(
        display: *mut Display,
        window: Window,
        root_return: *mut Window,
        child_return: *mut Window,
        root_x_return: *mut i32,
        root_y_return: *mut i32,
        win_x_return: *mut i32,
        win_y_return: *mut i32,
        mask_return: *mut u32,
    ) -> Bool;
}

pub fn seconds_since_last_input() -> f64 {
    match user_idle3::UserIdle::get_time() {
        Ok(idle) => idle.duration().as_secs_f64(),
        Err(err) => {
            log::warn!("user-idle3 error: {err}");
            f64::MAX
        }
    }
}

fn x11_display() -> Option<*mut Display> {
    static DISPLAY: OnceLock<Option<X11Display>> = OnceLock::new();
    DISPLAY
        .get_or_init(|| {
            unsafe {
                let display = XOpenDisplay(std::ptr::null());
                if display.is_null() {
                    log::warn!("XOpenDisplay failed — no X11 display available (Wayland-only?)");
                    None
                } else {
                    Some(X11Display(display))
                }
            }
        })
        .as_ref()
        .map(|wrapper| wrapper.0)
}

pub fn poll_mouse_position() -> Option<(f64, f64)> {
    let display = x11_display()?;
    unsafe {
        let root = XDefaultRootWindow(display);
        let mut root_x: i32 = 0;
        let mut root_y: i32 = 0;
        let mut win_x: i32 = 0;
        let mut win_y: i32 = 0;
        let mut mask: u32 = 0;
        let mut child_ret: Window = 0;
        let mut root_ret: Window = 0;

        let status = XQueryPointer(
            display,
            root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );

        if status != 0 {
            Some((root_x as f64, root_y as f64))
        } else {
            None
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
    fn test_linux_seconds_since_last_input_is_not_negative() {

        let idle = seconds_since_last_input();
        assert!(idle >= 0.0 || idle == f64::MAX);
    }

    #[test]
    fn test_linux_check_permission() {

        assert!(check_permission());
    }
}

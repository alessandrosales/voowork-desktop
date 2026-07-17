// ---------------------------------------------------------------------------
// Platform activity detection dispatch
//
// Each platform module implements three functions:
//   - poll_mouse_position() -> Option<(f64, f64)>
//   - seconds_since_last_input() -> f64
//   - check_permission() -> bool
//
// A fallback stub exists for unsupported platforms (FreeBSD, etc.).
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

// Fallback for unsupported platforms (FreeBSD, etc.)
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod stub;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use stub::*;

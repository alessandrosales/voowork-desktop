mod commands;

use crate::app_state::AppState;
use crate::auth::{read_session_identity, KEY_AUTHENTICATED};
use crate::icons::app_icon;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Size, WebviewWindow};
use tauri::window::Color;

pub use commands::{
    begin_mini_widget_drag, open_main_window, reset_mini_widget_position,
};

pub const MINI_WINDOW_LABEL: &str = "mini-timer";
pub const MINI_WINDOW_WIDTH: u32 = 200;
pub const MINI_WINDOW_HEIGHT: u32 = 20;
pub const SETTING_MINI_WIDGET_ENABLED: &str = "mini_widget_enabled";
pub const SETTING_MINI_WIDGET_X: &str = "mini_widget_x";
pub const SETTING_MINI_WIDGET_Y: &str = "mini_widget_y";

const DEFAULT_MINI_X: i32 = 80;
const DEFAULT_MINI_Y: i32 = 80;

static MINI_POSITION_CLAMPING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MiniPositionBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

fn mini_position_bounds(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
    window_width: i32,
    window_height: i32,
) -> MiniPositionBounds {
    let min_x = monitor_position.x;
    let min_y = monitor_position.y;
    let max_x = monitor_position.x + monitor_size.width as i32 - window_width;
    let max_y = monitor_position.y + monitor_size.height as i32 - window_height;

    MiniPositionBounds {
        min_x,
        min_y,
        max_x: max_x.max(min_x),
        max_y: max_y.max(min_y),
    }
}

fn clamp_to_bounds(x: i32, y: i32, bounds: MiniPositionBounds) -> (i32, i32) {
    (
        x.clamp(bounds.min_x, bounds.max_x),
        y.clamp(bounds.min_y, bounds.max_y),
    )
}

fn clamp_mini_position(mini: &WebviewWindow, x: i32, y: i32) -> tauri::Result<(i32, i32)> {
    let window_size = mini.outer_size()?;
    let width = window_size.width as i32;
    let height = window_size.height as i32;

    let monitor = match mini.current_monitor() {
        Ok(Some(monitor)) => Some(monitor),
        Ok(None) => mini.primary_monitor().ok().flatten(),
        Err(_) => mini.primary_monitor().ok().flatten(),
    };

    let Some(monitor) = monitor else {
        return Ok((x, y));
    };

    let bounds = mini_position_bounds(*monitor.position(), *monitor.size(), width, height);
    Ok(clamp_to_bounds(x, y, bounds))
}

fn apply_mini_position(app: &AppHandle, mini: &WebviewWindow, x: i32, y: i32) -> tauri::Result<()> {
    let (x, y) = clamp_mini_position(mini, x, y)?;
    mini.set_position(PhysicalPosition::new(x, y))?;
    persist_mini_position(app, x, y);
    Ok(())
}

pub fn setup_windows(app: &tauri::App) -> tauri::Result<()> {
    let icon = app_icon();

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_icon(icon.clone());
        let main_clone = main.clone();
        main.on_window_event(move |event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let app = main_clone.app_handle();
                    if let Err(err) = enter_background_mode(app) {
                        log::warn!("enter background mode failed: {err}");
                        let _ = main_clone.hide();
                    }
                }
                tauri::WindowEvent::Focused(false) => {
                    let app = main_clone.app_handle();
                    if should_show_mini_widget(app) {
                        if let Err(err) = show_mini_timer_quiet(app) {
                            log::warn!("show mini widget on focus lost failed: {err}");
                        }
                    }
                }
                tauri::WindowEvent::Focused(true) => {
                    if let Some(mini) = main_clone.app_handle().get_webview_window(MINI_WINDOW_LABEL) {
                        let _ = mini.hide();
                    }
                }
                _ => {}
            }
        });
    }

    if let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) {
        let _ = mini.set_icon(icon);
        let _ = mini.set_background_color(Some(Color(0, 0, 0, 0)));
        let _ = mini.set_resizable(true);
        let _ = mini.set_size(Size::Logical(LogicalSize::new(
            MINI_WINDOW_WIDTH as f64,
            MINI_WINDOW_HEIGHT as f64,
        )));
        let mini_clone = mini.clone();
        mini.on_window_event(move |event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = mini_clone.hide();
                }
                tauri::WindowEvent::Moved(position) => {
                    if MINI_POSITION_CLAMPING.load(Ordering::Relaxed) {
                        return;
                    }

                    let app = mini_clone.app_handle();
                    match clamp_mini_position(&mini_clone, position.x, position.y) {
                        Ok((x, y)) => {
                            if x != position.x || y != position.y {
                                MINI_POSITION_CLAMPING.store(true, Ordering::Relaxed);
                                let _ = mini_clone.set_position(PhysicalPosition::new(x, y));
                                MINI_POSITION_CLAMPING.store(false, Ordering::Relaxed);
                            }
                            persist_mini_position(app, x, y);
                        }
                        Err(err) => {
                            log::debug!("mini widget clamp failed: {err}");
                            persist_mini_position(app, position.x, position.y);
                        }
                    }
                }
                _ => {}
            }
        });
    }

    Ok(())
}

pub fn enter_background_mode(app: &AppHandle) -> tauri::Result<()> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    if should_show_mini_widget(app) {
        show_mini_timer(app)?;
    }

    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) {
        let _ = mini.hide();
    }

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

/// Like `show_mini_timer()` but does NOT call `set_focus()` — used when the
/// user switches away from the app (minimized / Cmd+Tab) so the mini-widget
/// appears without stealing focus from the other app.
pub fn show_mini_timer_quiet(app: &AppHandle) -> tauri::Result<()> {
    let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) else {
        return Ok(());
    };

    restore_mini_position(app, &mini)?;
    mini.set_always_on_top(true)?;
    let _ = mini.show();
    // Intentionally skip set_focus() to avoid stealing focus on macOS

    Ok(())
}

pub fn hide_mini_timer(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) {
        let _ = mini.hide();
    }
}

fn should_show_mini_widget(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };

    let db = state.db.lock();
    if !db
        .get_setting(KEY_AUTHENTICATED)
        .ok()
        .flatten()
        .is_some_and(|value| value == "true")
    {
        return false;
    }

    if !is_mini_widget_enabled(&db) {
        return false;
    }

    read_session_identity(&db).ok().flatten().is_some()
}

fn is_mini_widget_enabled(db: &crate::db::Database) -> bool {
    db.get_setting(SETTING_MINI_WIDGET_ENABLED)
        .ok()
        .flatten()
        .map(|value| value != "false" && value != "0")
        .unwrap_or(true)
}

pub fn show_mini_timer(app: &AppHandle) -> tauri::Result<()> {
    let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) else {
        return Ok(());
    };

    restore_mini_position(app, &mini)?;
    mini.set_always_on_top(true)?;
    let _ = mini.show();
    let _ = mini.set_focus();

    Ok(())
}

fn restore_mini_position(app: &AppHandle, mini: &WebviewWindow) -> tauri::Result<()> {
    let (x, y) = app
        .try_state::<AppState>()
        .map(|state| {
            let db = state.db.lock();
            let x = db
                .get_setting(SETTING_MINI_WIDGET_X)
                .ok()
                .flatten()
                .and_then(|value| value.parse::<i32>().ok());
            let y = db
                .get_setting(SETTING_MINI_WIDGET_Y)
                .ok()
                .flatten()
                .and_then(|value| value.parse::<i32>().ok());
            (x, y)
        })
        .unwrap_or((None, None));

    let x = x.unwrap_or(DEFAULT_MINI_X);
    let y = y.unwrap_or(DEFAULT_MINI_Y);
    apply_mini_position(app, mini, x, y)?;
    Ok(())
}

pub fn reset_mini_to_default(app: &AppHandle) -> tauri::Result<()> {
    let Some(mini) = app.get_webview_window(MINI_WINDOW_LABEL) else {
        return Ok(());
    };

    apply_mini_position(app, &mini, DEFAULT_MINI_X, DEFAULT_MINI_Y)?;
    mini.set_always_on_top(true)?;
    let _ = mini.show();
    let _ = mini.set_focus();

    Ok(())
}

fn persist_mini_position(app: &AppHandle, x: i32, y: i32) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let db = state.db.lock();
    let _ = db.set_setting(SETTING_MINI_WIDGET_X, &x.to_string());
    let _ = db.set_setting(SETTING_MINI_WIDGET_Y, &y.to_string());
}

#[cfg(test)]
mod tests {
    use super::{clamp_to_bounds, mini_position_bounds, MINI_WINDOW_HEIGHT, MINI_WINDOW_WIDTH};
    use tauri::{PhysicalPosition, PhysicalSize};

    #[test]
    fn mini_position_bounds_uses_full_monitor_area() {
        let bounds = mini_position_bounds(
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1920, 1080),
            MINI_WINDOW_WIDTH as i32,
            MINI_WINDOW_HEIGHT as i32,
        );

        assert_eq!(bounds.min_x, 0);
        assert_eq!(bounds.min_y, 0);
        assert_eq!(bounds.max_x, 1720);
        assert_eq!(bounds.max_y, 1060);
    }

    #[test]
    fn clamp_to_bounds_keeps_widget_inside_monitor() {
        let bounds = super::MiniPositionBounds {
            min_x: 0,
            min_y: 0,
            max_x: 1720,
            max_y: 1032,
        };

        assert_eq!(clamp_to_bounds(120, 1100, bounds), (120, 1032));
        assert_eq!(clamp_to_bounds(-20, 10, bounds), (0, 10));
    }
}

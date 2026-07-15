use tauri::{AppHandle, WebviewWindow};

use super::{reset_mini_to_default, show_main_window};

#[tauri::command]
pub fn open_main_window(app: AppHandle) {
    show_main_window(&app);
}

#[tauri::command]
pub fn begin_mini_widget_drag(window: WebviewWindow) -> Result<(), String> {
    raise_mini_for_drag(&window).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn reset_mini_widget_position(app: AppHandle) -> Result<(), String> {
    reset_mini_to_default(&app).map_err(|err| err.to_string())
}

fn raise_mini_for_drag(window: &WebviewWindow) -> tauri::Result<()> {
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();
    window.start_dragging()
}

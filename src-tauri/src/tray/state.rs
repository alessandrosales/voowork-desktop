use parking_lot::Mutex;
use tauri::{menu::MenuItem, Wry};

pub const SETTING_LAST_PROJECT_ID: &str = "last_project_id";
pub const SETTING_LAST_TASK_ID: &str = "last_task_id";
pub const SETTING_SELECTED_PROJECT_ID: &str = "selected_project_id";
pub const SETTING_SELECTED_TASK_ID: &str = "selected_task_id";

pub struct TrayState {
    pub status: MenuItem<Wry>,
    pub toggle: MenuItem<Wry>,
    pub show: MenuItem<Wry>,
    pub reset_widget_position: MenuItem<Wry>,
    pub logout: MenuItem<Wry>,
    pub quit: MenuItem<Wry>,
    pub locale: Mutex<String>,
}

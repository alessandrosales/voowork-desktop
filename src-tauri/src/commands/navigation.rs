use tauri::AppHandle;

use crate::error::AgentResult;
use crate::navigation::{configured_web_panel_url, open_allowed_url};

#[tauri::command]
pub fn open_web_panel(app: AppHandle) -> AgentResult<()> {
    open_allowed_url(&app, &configured_web_panel_url())
}

#[tauri::command]
pub fn open_external_url(app: AppHandle, url: String) -> AgentResult<()> {
    open_allowed_url(&app, &url)
}

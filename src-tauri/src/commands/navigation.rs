#[cfg(target_os = "macos")]
use std::process::Command;

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

/// Abre os Ajustes do macOS na tela de Monitoramento de Entrada.
/// No macOS 26, a permissão de Input Monitoring é necessária para o rdev
/// capturar eventos globais de mouse/teclado.
#[tauri::command]
pub fn open_system_settings_input_monitoring() -> AgentResult<()> {
    #[cfg(target_os = "macos")]
    {
        let outcome = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .output()
            .map_err(|e| crate::error::AgentError::Other(format!("failed to open System Settings: {e}")))?;

        if !outcome.status.success() {
            log::warn!("open System Settings may have failed: {:?}", outcome.status);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        log::info!("open_system_settings_input_monitoring is macOS-only");
    }

    Ok(())
}

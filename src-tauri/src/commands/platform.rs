use crate::models::PlatformInfo;

#[tauri::command]
pub fn get_platform_info() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();

    #[cfg(target_os = "linux")]
    let desktop_env: Option<String> = {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            Some("wayland".into())
        } else if std::env::var("DISPLAY").is_ok() {
            Some("x11".into())
        } else {
            None
        }
    };

    #[cfg(not(target_os = "linux"))]
    let desktop_env: Option<String> = None;

    match os.as_str() {
        "macos" => PlatformInfo {
            os,
            desktop_env,
            needs_input_monitoring_permission: true,
            needs_screen_recording_permission: true,
            always_allows_window_tracking: false,
            note: Some(
                "macOS requires Input Monitoring and Screen Recording permissions in System Settings."
                    .into(),
            ),
        },
        "linux" => {
            let (always_allows, note) = match desktop_env.as_deref() {
                Some("wayland") => (
                    false,
                    Some(
                        "On Wayland, active window detection is limited. \
                         Screen capture may trigger a portal permission dialog \
                         (xdg-desktop-portal)."
                            .into(),
                    ),
                ),
                _ => (true, None),
            };
            PlatformInfo {
                os,
                desktop_env,
                needs_input_monitoring_permission: false,
                needs_screen_recording_permission: false,
                always_allows_window_tracking: always_allows,
                note,
            }
        }
        "windows" => PlatformInfo {
            os,
            desktop_env,
            needs_input_monitoring_permission: false,
            needs_screen_recording_permission: false,
            always_allows_window_tracking: true,
            note: None,
        },
        other => PlatformInfo {
            os: other.into(),
            desktop_env,
            needs_input_monitoring_permission: false,
            needs_screen_recording_permission: false,
            always_allows_window_tracking: false,
            note: Some(format!("Platform '{other}' has limited tracking support.")),
        },
    }
}

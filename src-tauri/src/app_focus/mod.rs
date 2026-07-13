use crate::error::{guard_native, AgentError, AgentResult};
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppFocusSample {
    pub app_name: String,
    pub window_title: String,
    pub process_path: Option<String>,
    pub process_id: Option<i64>,
}

pub fn capture_active_window() -> Option<AppFocusSample> {
    match guard_native("active_window", || {
        let window = active_win_pos_rs::get_active_window().map_err(|()| {
            AgentError::Other("active window unavailable".into())
        })?;
        let process_path = window.process_path.to_string_lossy().to_string();

        Ok(Some(AppFocusSample {
            app_name: if window.app_name.is_empty() {
                "unknown".into()
            } else {
                window.app_name
            },
            window_title: window.title,
            process_path: if process_path.is_empty() {
                None
            } else {
                Some(process_path)
            },
            process_id: Some(window.process_id as i64),
        }))
    }) {
        Ok(sample) => sample,
        Err(err) => {
            log::warn!("active window capture failed: {err}");
            None
        }
    }
}

/// Returns false for the Voowork agent itself, file managers, and system pickers.
pub fn should_track_app_focus(sample: &AppFocusSample) -> bool {
    !is_self_app(sample) && !is_excluded_system_ui(sample)
}

pub fn insert_app_focus(
    conn: &Connection,
    session_id: &str,
    sample: &AppFocusSample,
) -> AgentResult<String> {
    let id = Uuid::new_v4().to_string();
    let captured_at = chrono::Utc::now().to_rfc3339();
    let now = captured_at.clone();

    conn.execute(
        "INSERT INTO app_focus_events (
            id, session_id, app_name, window_title, process_path, process_id, captured_at, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            session_id,
            sample.app_name,
            sample.window_title,
            sample.process_path,
            sample.process_id,
            captured_at,
            now,
        ],
    )?;

    Ok(id)
}

fn normalize_app_name(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

fn is_self_app(sample: &AppFocusSample) -> bool {
    let app = normalize_app_name(&sample.app_name);
    if app == "tauri-native" || app == "voowork-desktop" || app == "voowork" {
        return true;
    }

    if let Some(path) = &sample.process_path {
        let path_lower = path.to_lowercase();
        if path_lower.contains("tauri-native")
            || path_lower.contains("voowork-desktop")
            || path_lower.contains("com.voowork")
        {
            return true;
        }
    }

    sample.window_title.eq_ignore_ascii_case("voowork")
}

fn is_excluded_system_ui(sample: &AppFocusSample) -> bool {
    let app = normalize_app_name(&sample.app_name);

    const FILE_MANAGERS: &[&str] = &[
        "nautilus",
        "org.gnome.nautilus",
        "dolphin",
        "thunar",
        "nemo",
        "pcmanfm",
        "caja",
        "konqueror",
        "krusader",
        "spacefm",
        "doublecmd",
    ];

    for fm in FILE_MANAGERS {
        if app == *fm || app.ends_with(&format!(".{fm}")) {
            return true;
        }
    }

    if app.starts_with("xdg-desktop-portal") {
        return true;
    }

    matches!(app.as_str(), "org.gnome.shell" | "plasmashell")
}

pub fn is_communication_app(sample: &AppFocusSample) -> bool {
    let app = normalize_app_name(&sample.app_name);
    if matches!(
        app.as_str(),
        "zoom"
            | "teams"
            | "microsoft-teams"
            | "slack"
            | "discord"
            | "webex"
            | "skype"
            | "telegram"
            | "whatsapp"
            | "signal"
    ) {
        return true;
    }

    let title = sample.window_title.to_lowercase();
    if title.contains("zoom meeting")
        || title.contains("google meet")
        || title.contains("microsoft teams")
        || title.contains("slack |")
        || title.contains("discord")
        || title.contains("webex")
    {
        return true;
    }

    matches!(
        app.as_str(),
        "google-chrome" | "chromium" | "firefox" | "brave" | "microsoft-edge"
    ) && (title.contains("meet") || title.contains("zoom") || title.contains("teams"))
}

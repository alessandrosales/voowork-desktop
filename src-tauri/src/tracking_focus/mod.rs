use crate::error::{guard_native, AgentError, AgentResult};
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActiveWindowSample {
    pub app_name: String,
    pub window_title: String,
    pub process_path: Option<String>,
    #[allow(dead_code)]
    pub process_id: Option<i64>,
}

pub fn capture_active_window() -> Option<ActiveWindowSample> {
    match guard_native("active_window", || {
        let window = active_win_pos_rs::get_active_window().map_err(|()| {
            AgentError::Other("active window unavailable".into())
        })?;
        let process_path = window.process_path.to_string_lossy().to_string();

        Ok(Some(ActiveWindowSample {
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

/// Returns false for the Voowork desktop app itself, file managers, and system pickers.
pub fn should_track_active_window(sample: &ActiveWindowSample) -> bool {
    !is_self_app(sample) && !is_excluded_system_ui(sample)
}


pub const TRACKING_APP_NAME_MAX_LEN: usize = 200;

pub fn format_tracking_app_name(sample: &ActiveWindowSample) -> String {
    let title = sanitize_window_title(sample.window_title.trim());
    let name = if title.is_empty() || title.eq_ignore_ascii_case(sample.app_name.trim()) {
        sample.app_name.clone()
    } else {
        format!("{} — {}", sample.app_name, title)
    };
    truncate_tracking_app_name(name)
}

fn sanitize_window_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut digit_run = 0usize;

    for ch in title.chars() {
        if ch.is_ascii_digit() {
            digit_run += 1;
            if digit_run >= 8 {
                if digit_run == 8 {
                    out.push_str("****");
                }
                continue;
            }
            out.push(ch);
        } else {
            digit_run = 0;
            out.push(ch);
        }
    }

    out
}

fn truncate_tracking_app_name(name: String) -> String {
    if name.chars().count() <= TRACKING_APP_NAME_MAX_LEN {
        return name;
    }

    name.chars()
        .take(TRACKING_APP_NAME_MAX_LEN.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

pub fn open_tracking_app(
    conn: &Connection,
    tracking_id: &str,
    sample: &ActiveWindowSample,
    started_at: &str,
) -> AgentResult<String> {
    let id = Uuid::new_v4().to_string();
    let name = format_tracking_app_name(sample);
    conn.execute(
        "INSERT INTO tracking_apps (
            id, name, tracking_id, started_at, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, tracking_id, started_at, started_at],
    )?;
    Ok(id)
}

pub fn close_active_app(conn: &Connection, app_id: &str, ended_at: &str) -> AgentResult<()> {
    conn.execute(
        "UPDATE tracking_apps SET ended_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![app_id, ended_at],
    )?;
    Ok(())
}

pub fn is_browser_app(sample: &ActiveWindowSample) -> bool {
    const BROWSERS: &[&str] = &[
        "google-chrome",
        "chromium",
        "chromium-browser",
        "firefox",
        "brave",
        "brave-browser",
        "microsoft-edge",
        "opera",
        "vivaldi",
        "safari",
    ];

    let app = normalize_app_name(&sample.app_name);
    BROWSERS
        .iter()
        .any(|browser| app == *browser || app.ends_with(&format!(".{browser}")))
}

/// Extrai URL do título da janela quando o app ativo é um browser.
pub fn extract_site_address(sample: &ActiveWindowSample) -> Option<String> {
    if !is_browser_app(sample) {
        return None;
    }

    let title = strip_browser_suffix(sample.window_title.trim());
    if title.is_empty() {
        return None;
    }

    if let Some(url) = find_http_url(title) {
        return Some(normalize_site_url(url));
    }

    find_domain_in_title(title).map(|domain| format!("https://{domain}"))
}

pub fn open_tracking_site(
    conn: &Connection,
    tracking_id: &str,
    address: &str,
    started_at: &str,
) -> AgentResult<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO tracking_sites (
            id, address, tracking_id, started_at, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, address, tracking_id, started_at, started_at],
    )?;
    Ok(id)
}

pub fn close_active_site(conn: &Connection, site_id: &str, ended_at: &str) -> AgentResult<()> {
    conn.execute(
        "UPDATE tracking_sites SET ended_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![site_id, ended_at],
    )?;
    Ok(())
}

fn strip_browser_suffix(title: &str) -> &str {
    const SUFFIXES: &[&str] = &[
        " - Google Chrome",
        " — Google Chrome",
        " - Chromium",
        " — Chromium",
        " - Mozilla Firefox",
        " — Mozilla Firefox",
        " - Microsoft Edge",
        " — Microsoft Edge",
        " - Brave",
        " — Brave",
        " - Opera",
        " — Opera",
        " - Vivaldi",
        " — Vivaldi",
    ];

    for suffix in SUFFIXES {
        if let Some(stripped) = title.strip_suffix(suffix) {
            return stripped.trim();
        }
    }

    title
}

fn find_http_url(text: &str) -> Option<&str> {
    text.split_whitespace().find_map(|token| {
        let trimmed = token.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>')
        });
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            Some(trimmed)
        } else {
            None
        }
    })
}

fn normalize_site_url(url: &str) -> String {
    let trimmed = url.trim_end_matches(|c: char| {
        matches!(c, '.' | ',' | ';' | ')' | ']' | '}')
    });
    if trimmed.starts_with("http://") {
        trimmed.replacen("http://", "https://", 1)
    } else {
        trimmed.to_string()
    }
}

fn find_domain_in_title(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let cleaned = token
            .trim_matches(|c: char| {
                matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',')
            })
            .trim_end_matches('/');
        if cleaned.contains("://") {
            continue;
        }
        if let Some(domain) = parse_domain_token(cleaned) {
            return Some(domain);
        }
    }
    None
}

fn parse_domain_token(token: &str) -> Option<String> {
    let host = token.split('/').next()?.split('?').next()?;
    if host.len() < 4 || !host.contains('.') {
        return None;
    }

    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }

    let tld = labels.last()?;
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    if labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) {
        Some(host.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_app_name(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

fn is_self_app(sample: &ActiveWindowSample) -> bool {
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

fn is_excluded_system_ui(sample: &ActiveWindowSample) -> bool {
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

pub fn is_communication_app(sample: &ActiveWindowSample) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(app_name: &str, window_title: &str) -> ActiveWindowSample {
        ActiveWindowSample {
            app_name: app_name.into(),
            window_title: window_title.into(),
            process_path: None,
            process_id: None,
        }
    }

    #[test]
    fn extract_site_from_https_in_title() {
        let address = extract_site_address(&sample(
            "google-chrome",
            "Docs - https://github.com/voowork - Google Chrome",
        ));
        assert_eq!(address.as_deref(), Some("https://github.com/voowork"));
    }

    #[test]
    fn extract_site_from_domain_token() {
        let address = extract_site_address(&sample(
            "firefox",
            "Issues · voowork/backend - github.com — Mozilla Firefox",
        ));
        assert_eq!(address.as_deref(), Some("https://github.com"));
    }

    #[test]
    fn ignores_non_browser_apps() {
        assert!(extract_site_address(&sample("code", "README.md - Visual Studio Code")).is_none());
    }

    #[test]
    fn formats_tracking_app_name_with_window_title() {
        let name = format_tracking_app_name(&sample("code", "main.rs — voowork-desktop"));
        assert_eq!(name, "code — main.rs — voowork-desktop");
    }

    #[test]
    fn truncates_long_tracking_app_names() {
        let long_title = "x".repeat(300);
        let name = format_tracking_app_name(&sample("code", &long_title));
        assert!(name.chars().count() <= TRACKING_APP_NAME_MAX_LEN);
    }

    #[test]
    fn sanitizes_long_digit_sequences_in_window_title() {
        let name = format_tracking_app_name(&sample(
            "chrome",
            "Checkout 1234567890123456",
        ));
        assert!(name.contains("****"));
        assert!(!name.contains("1234567890123456"));
    }
}

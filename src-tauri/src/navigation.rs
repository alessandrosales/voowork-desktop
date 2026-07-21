use tauri_plugin_opener::OpenerExt;

use crate::error::{AgentError, AgentResult};

pub const ENV_WEB_PANEL_URL: &str = "FRONTEND_URL";
const DEFAULT_WEB_PANEL_URL_DEV: &str = "http://localhost:5173";
const DEFAULT_WEB_PANEL_URL_PROD: &str = "https://app.voowork.com";

pub fn configured_web_panel_url() -> String {
    std::env::var(ENV_WEB_PANEL_URL)
        .or_else(|_| std::env::var("VITE_WEB_URL"))
        .map(|v| v.trim().to_string())
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                DEFAULT_WEB_PANEL_URL_DEV.to_string()
            } else {
                DEFAULT_WEB_PANEL_URL_PROD.to_string()
            }
        })
}

pub fn open_allowed_url<R: tauri::Runtime>(
    opener: &impl tauri::Manager<R>,
    url: &str,
) -> AgentResult<()> {
    let normalized = normalize_url(url);
    if !is_allowed_external_url(&normalized) {
        return Err(AgentError::Other(format!(
            "external URL not allowed: {normalized}"
        )));
    }

    opener
        .opener()
        .open_url(&normalized, None::<&str>)
        .map_err(|err| AgentError::Other(format!("failed to open URL: {err}")))
}

pub fn external_navigation_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("external-navigation")
        .on_navigation(|webview, url| {
            let is_internal_host = matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost") | Some("::1")
            );

            let is_internal = url.scheme() == "tauri" || is_internal_host;

            if is_internal {
                return true;
            }

            let is_external_link = matches!(url.scheme(), "http" | "https" | "mailto" | "tel");

            if is_external_link {
                let target = url.as_str();
                if !is_allowed_external_url(target) {
                    log::warn!("blocked external navigation: {target}");
                    return false;
                }

                log::info!("opening external link in system browser: {target}");
                let _ = webview.opener().open_url(target, None::<&str>);
                return false;
            }

            true
        })
        .build()
}

fn is_allowed_external_url(url: &str) -> bool {
    let normalized = normalize_url(url);

    if normalized.starts_with("mailto:") || normalized.starts_with("tel:") {
        return true;
    }

    let Some((scheme, rest)) = normalized.split_once("://") else {
        return false;
    };

    if scheme != "http" && scheme != "https" {
        return false;
    }

    let allowed_origins = allowed_web_origins();
    let request_origin = origin_key(scheme, rest);
    allowed_origins.contains(&request_origin)
}

fn allowed_web_origins() -> Vec<String> {
    let panel = configured_web_panel_url();
    let mut origins = vec![
        origin_from_url(&panel),
        origin_from_url(DEFAULT_WEB_PANEL_URL_PROD),
        origin_from_url(DEFAULT_WEB_PANEL_URL_DEV),
        origin_from_url("http://127.0.0.1:5173"),
    ];
    origins.retain(|origin| !origin.is_empty());
    origins.sort_unstable();
    origins.dedup();
    origins
}

fn origin_from_url(url: &str) -> String {
    let normalized = normalize_url(url);
    let Some((scheme, rest)) = normalized.split_once("://") else {
        return String::new();
    };
    origin_key(scheme, rest)
}

fn origin_key(scheme: &str, authority_and_path: &str) -> String {
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(authority_and_path);
    format!("{scheme}://{authority}")
}

fn normalize_url(url: &str) -> String {
    url.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_configured_web_panel_origin() {
        assert!(is_allowed_external_url(DEFAULT_WEB_PANEL_URL_PROD));
        assert!(is_allowed_external_url("https://app.voowork.com/dashboard"));
    }

    #[test]
    fn allows_mailto_and_tel() {
        assert!(is_allowed_external_url("mailto:support@voowork.com"));
        assert!(is_allowed_external_url("tel:+5511999999999"));
    }

    #[test]
    fn blocks_unknown_http_origin() {
        assert!(!is_allowed_external_url("https://evil.example/phish"));
    }
}

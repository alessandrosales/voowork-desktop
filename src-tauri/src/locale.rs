use crate::db::Database;

pub const LOCALE_SETTING_KEY: &str = "locale";

pub fn detect_system_locale() -> &'static str {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if let Some(locale) = resolve_locale(&value) {
                return locale;
            }
        }
    }
    "pt-BR"
}

pub fn resolve_locale(input: &str) -> Option<&'static str> {
    let token = input.split('.').next().unwrap_or(input).trim();
    if token.is_empty() {
        return None;
    }

    let normalized = token.to_ascii_lowercase();

    if normalized == "pt-br" || normalized.starts_with("pt") {
        return Some("pt-BR");
    }
    if normalized.starts_with("es") {
        return Some("es");
    }
    if normalized.starts_with("en") {
        return Some("en");
    }

    match token {
        "pt-BR" => Some("pt-BR"),
        "en" => Some("en"),
        "es" => Some("es"),
        _ => None,
    }
}

pub fn effective_locale(db: &Database) -> &'static str {
    db.get_setting(LOCALE_SETTING_KEY)
        .ok()
        .flatten()
        .as_deref()
        .and_then(resolve_locale)
        .unwrap_or_else(detect_system_locale)
}

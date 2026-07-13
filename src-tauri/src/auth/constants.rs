/// Chaves SQLite em `settings` para persistência da sessão.
pub const KEY_AUTHENTICATED: &str = "auth_authenticated";
pub const KEY_ACCESS_TOKEN: &str = "auth_access_token";
pub const KEY_REFRESH_TOKEN: &str = "auth_refresh_token";
pub const KEY_USER: &str = "auth_user_json";
pub const KEY_ORGANIZATION: &str = "auth_org_json";

pub const ENV_API_URL: &str = "VOOWORK_API_URL";
pub const DEFAULT_API_URL_DEV: &str = "http://localhost:3000";
pub const DEFAULT_API_URL_PROD: &str = "https://api.voowork.com";

/// Timeout HTTP para login e validação de sessão.
pub const HTTP_TIMEOUT_SECS: u64 = 30;

pub fn configured_api_base_url() -> String {
    std::env::var(ENV_API_URL).unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            log::warn!(
                "VOOWORK_API_URL não definida; usando {DEFAULT_API_URL_DEV}. \
                 Copie .env.example para .env na raiz do voowork-desktop."
            );
            DEFAULT_API_URL_DEV.to_string()
        } else {
            log::warn!(
                "VOOWORK_API_URL não definida; usando {DEFAULT_API_URL_PROD}. \
                 Defina .env.production ou variável de ambiente do sistema."
            );
            DEFAULT_API_URL_PROD.to_string()
        }
    })
}

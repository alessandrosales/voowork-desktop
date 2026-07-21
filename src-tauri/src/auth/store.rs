use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::{AgentError, AgentResult};

use super::token_store;

/// Chaves SQLite em `settings` para persistência da sessão.
pub const KEY_AUTHENTICATED: &str = "auth_authenticated";
pub const KEY_ACCESS_TOKEN: &str = "auth_access_token";
pub const KEY_USER: &str = "auth_user_json";
pub const KEY_ORGANIZATION: &str = "auth_org_json";

pub const DEFAULT_API_URL_DEV: &str = "http://localhost:3000";
pub const DEFAULT_API_URL_PROD: &str = "https://api.voowork.com";
pub const HTTP_TIMEOUT_SECS: u64 = 30;

// ── Token storage design (M18) ──────────────────────────────────────────
// The JWT access token is stored in two places:
//   1. OS credential store (keyring) — preferred, encrypted at rest.
//   2. SQLite `settings` table — permanent fallback in plaintext.
//
// This is intentional: the SQLite copy ensures sync and auth survive
// keyring unavailability (dbus failure, headless environments, WSL).
// The threat model assumes single-user workstation — file-system access
// implies full compromise regardless of token encryption.
// See docs/features/01-authentication.md for details.
// ────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthOrganization {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub is_authenticated: bool,
    pub user: Option<AuthUser>,
    pub organization: Option<AuthOrganization>,
}

impl AuthState {
    pub fn signed_out() -> Self {
        Self {
            is_authenticated: false,
            user: None,
            organization: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct SessionIdentity {
    pub user: AuthUser,
    pub organization: AuthOrganization,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub access_token: String,
    pub user: AuthUser,
    pub organization: AuthOrganization,
}

impl AuthSession {
    pub fn to_auth_state(&self) -> AuthState {
        AuthState {
            is_authenticated: true,
            user: Some(self.user.clone()),
            organization: Some(self.organization.clone()),
        }
    }
}

/// Retorna a URL base da API Rails.
///
/// O valor vem do `.env` (raiz do projeto), injetado em tempo de compilação
/// pelo `build.rs` e restaurado em runtime pelo `env.rs::load()`.
/// Fallback para o default de dev ou produção se nada estiver definido.
pub fn configured_api_base_url() -> String {
    let result = std::env::var("API_URL")
        .or_else(|_| std::env::var("VITE_API_URL"))
        .unwrap_or_else(|_| {
            let default = if cfg!(debug_assertions) {
                DEFAULT_API_URL_DEV
            } else {
                DEFAULT_API_URL_PROD
            };
            log::warn!("API_URL não definida; usando {default}. Defina API_URL no .env");
            default.to_string()
        });
    log::info!("configured_api_base_url() = {}", result);
    result
}

pub fn resolve_api_url(configured_url: &str) -> AgentResult<String> {
    let url = configured_url.trim();
    if url.is_empty() {
        return Err(AgentError::Auth(
            "API não configurada (defina VITE_API_URL)".into(),
        ));
    }
    Ok(url.to_string())
}

pub fn persist_session(db: &Database, session: &AuthSession) -> AgentResult<()> {
    // Always store the token in OS keyring when available (more secure).
    // The SQLite copy is kept as a permanent fallback so that sync and auth
    // continue working even if the keyring becomes temporarily unavailable
    // (e.g. dbus timeout, headless environment, keyring daemon restart).
    if let Err(err) = token_store::store_access_token(&session.access_token) {
        log::warn!(
            "failed to store access token in credential store: {err}; \
             keeping SQLite token fallback"
        );
    }
    db.set_setting(KEY_ACCESS_TOKEN, &session.access_token)?;

    db.set_setting(KEY_USER, &serde_json::to_string(&session.user)?)?;
    db.set_setting(KEY_ORGANIZATION, &serde_json::to_string(&session.organization)?)?;
    db.set_setting(KEY_AUTHENTICATED, "true")?;
    Ok(())
}

pub fn read_session_identity(db: &Database) -> AgentResult<Option<SessionIdentity>> {
    let authenticated = db
        .get_setting(KEY_AUTHENTICATED)?
        .is_some_and(|value| value == "true");
    if !authenticated {
        return Ok(None);
    }

    let user = read_setting_json::<AuthUser>(db, KEY_USER)?;
    let organization = read_setting_json::<AuthOrganization>(db, KEY_ORGANIZATION)?;

    let (Some(user), Some(organization)) = (user, organization) else {
        return Ok(None);
    };

    Ok(Some(SessionIdentity { user, organization }))
}

pub fn read_auth_state(db: &Database) -> AgentResult<AuthState> {
    let Some(identity) = read_session_identity(db)? else {
        return Ok(AuthState::signed_out());
    };

    let Some(_access_token) = load_access_token(db)? else {
        return Ok(AuthState::signed_out());
    };

    Ok(AuthState {
        is_authenticated: true,
        user: Some(identity.user),
        organization: Some(identity.organization),
    })
}

pub fn read_session(db: &Database) -> AgentResult<Option<AuthSession>> {
    let Some(identity) = read_session_identity(db)? else {
        return Ok(None);
    };

    let Some(access_token) = load_access_token(db)? else {
        return Ok(None);
    };

    Ok(Some(AuthSession {
        access_token,
        user: identity.user,
        organization: identity.organization,
    }))
}

pub fn clear_session(db: &Database) -> AgentResult<()> {
    token_store::clear_access_token()?;
    db.set_setting(KEY_AUTHENTICATED, "false")?;
    db.set_setting(KEY_ACCESS_TOKEN, "")?;
    db.set_setting(KEY_USER, "")?;
    db.set_setting(KEY_ORGANIZATION, "")?;
    Ok(())
}

/// Alias for clear_session — usada pelo sync worker quando a API revoga o token.
pub fn invalidate_session(db: &Database) -> AgentResult<()> {
    clear_session(db)
}

pub fn read_access_token(db: &Database) -> AgentResult<Option<String>> {
    load_access_token(db)
}

pub fn read_organization_id(db: &Database) -> AgentResult<Option<String>> {
    let authenticated = db
        .get_setting(KEY_AUTHENTICATED)?
        .is_some_and(|value| value == "true");
    if !authenticated {
        return Ok(None);
    }

    Ok(read_setting_json::<AuthOrganization>(db, KEY_ORGANIZATION)?
        .map(|org| org.id)
        .filter(|id| !id.is_empty()))
}

/// Reads JWT from OS credential store, falling back to the SQLite backup.
/// The SQLite copy is intentionally kept as a fallback (not cleared after
/// keyring read) so that sync/auth survive keyring unavailability.
fn load_access_token(db: &Database) -> AgentResult<Option<String>> {
    if let Some(token) = token_store::read_access_token()? {
        return Ok(Some(token));
    }

    let Some(legacy) = db.get_setting(KEY_ACCESS_TOKEN)?.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    // Best-effort migration to keyring for next time; never clear SQLite fallback.
    if let Err(err) = token_store::store_access_token(&legacy) {
        log::warn!("failed to seed access token in OS credential store: {err}");
    }

    Ok(Some(legacy))
}

fn read_setting_json<T: serde::de::DeserializeOwned>(
    db: &Database,
    key: &str,
) -> AgentResult<Option<T>> {
    Ok(db
        .get_setting(key)?
        .and_then(|json| serde_json::from_str(&json).ok()))
}

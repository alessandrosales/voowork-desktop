use parking_lot::Mutex;
use std::sync::Arc;

use crate::db::Database;
use crate::error::{AgentError, AgentResult};
use crate::projects;

use super::api::{self, LoginClient};
use super::models::{AuthState, LoginRequest};
use super::shared::resolve_base_url;
use super::store::{clear_session, persist_session, read_session, AuthSession};

pub async fn login(
    api_base_url: &str,
    db: Arc<Mutex<Database>>,
    request: LoginRequest,
) -> AgentResult<AuthState> {
    let session = fetch_session_from_api(api_base_url, &request).await?;

    {
        let db_guard = db.lock();
        persist_session(&db_guard, &session)?;
        projects::invalidate_project_cache_if_org_changed(&db_guard, &session.organization.id)?;
    }

    if let Err(err) = projects::sync_project_cache(
        api_base_url,
        &session.access_token,
        Arc::clone(&db),
    )
    .await
    {
        log::warn!("project cache sync failed after login: {err}");
    }

    Ok(session.to_auth_state())
}

/// Encerra a sessão local após ação explícita do usuário.
pub fn logout(db: &Database) -> AgentResult<()> {
    clear_session(db)
}

/// Encerra a sessão local quando a API revoga ou rejeita o token (ex.: sync worker).
pub fn invalidate_session(db: &Database) -> AgentResult<()> {
    logout(db)
}

pub fn local_auth_state(db: &Database) -> AgentResult<AuthState> {
    Ok(read_session(db)?
        .map(|session| session.to_auth_state())
        .unwrap_or_else(AuthState::signed_out))
}

pub async fn validate_auth_session(
    api_base_url: &str,
    db: Arc<Mutex<Database>>,
) -> AgentResult<AuthState> {
    let session = {
        let db_guard = db.lock();
        read_session(&db_guard)?
    };

    let Some(session) = session else {
        return Ok(AuthState::signed_out());
    };

    let base_url = match resolve_base_url(api_base_url) {
        Ok(url) => url.to_string(),
        Err(_) => return Ok(session.to_auth_state()),
    };

    match api::fetch_me_profile(&base_url, &session.access_token, &session.organization).await {
        Ok((user, organization)) => {
            let updated = AuthSession {
                access_token: session.access_token,
                refresh_token: session.refresh_token,
                user,
                organization,
            };
            {
                let db_guard = db.lock();
                persist_session(&db_guard, &updated)?;
            }
            if let Err(err) = projects::refresh_project_cache_if_stale(
                api_base_url,
                Arc::clone(&db),
            )
            .await
            {
                log::warn!("project cache refresh after auth validate failed: {err}");
            }
            Ok(updated.to_auth_state())
        }
        Err(AgentError::Auth(message)) => {
            log::info!("auth session invalidated: {message}");
            let db_guard = db.lock();
            clear_session(&db_guard)?;
            Ok(AuthState::signed_out())
        }
        Err(AgentError::Http(err)) if err.is_connect() || err.is_timeout() => {
            Ok(session.to_auth_state())
        }
        Err(err) => {
            log::warn!("auth refresh failed, keeping local session: {err}");
            Ok(session.to_auth_state())
        }
    }
}

async fn fetch_session_from_api(
    api_base_url: &str,
    request: &LoginRequest,
) -> AgentResult<AuthSession> {
    let base_url = resolve_base_url(api_base_url)?;
    let client = LoginClient::with_base_url(base_url)?;
    client
        .fetch_session(&request.email, &request.password)
        .await
}

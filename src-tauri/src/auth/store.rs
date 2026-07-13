use crate::db::Database;
use crate::error::{AgentError, AgentResult};

use super::models::{AuthOrganization, AuthState, AuthUser};
use super::constants::{
    KEY_ACCESS_TOKEN, KEY_AUTHENTICATED, KEY_ORGANIZATION, KEY_REFRESH_TOKEN, KEY_USER,
};

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub user: AuthUser,
    pub organization: AuthOrganization,
}

impl AuthSession {
    pub fn to_auth_state(self) -> AuthState {
        AuthState {
            is_authenticated: true,
            user: Some(self.user),
            organization: Some(self.organization),
        }
    }
}

pub fn persist_session(db: &Database, session: &AuthSession) -> AgentResult<()> {
    db.set_setting(KEY_ACCESS_TOKEN, &session.access_token)?;
    match &session.refresh_token {
        Some(token) => db.set_setting(KEY_REFRESH_TOKEN, token)?,
        None => db.set_setting(KEY_REFRESH_TOKEN, "")?,
    }
    db.set_setting(KEY_USER, &serde_json::to_string(&session.user)?)?;
    db.set_setting(KEY_ORGANIZATION, &serde_json::to_string(&session.organization)?)?;
    db.set_setting(KEY_AUTHENTICATED, "true")?;
    Ok(())
}

pub fn read_session(db: &Database) -> AgentResult<Option<AuthSession>> {
    let authenticated = db
        .get_setting(KEY_AUTHENTICATED)?
        .is_some_and(|value| value == "true");

    if !authenticated {
        return Ok(None);
    }

    let access_token = db
        .get_setting(KEY_ACCESS_TOKEN)?
        .filter(|value| !value.is_empty());
    let user = read_setting_json::<AuthUser>(db, KEY_USER)?;
    let organization = read_setting_json::<AuthOrganization>(db, KEY_ORGANIZATION)?;

    let (Some(access_token), Some(user), Some(organization)) = (access_token, user, organization)
    else {
        return Ok(None);
    };

    let refresh_token = db
        .get_setting(KEY_REFRESH_TOKEN)?
        .filter(|value| !value.is_empty());

    Ok(Some(AuthSession {
        access_token,
        refresh_token,
        user,
        organization,
    }))
}

pub fn clear_session(db: &Database) -> AgentResult<()> {
    db.set_setting(KEY_AUTHENTICATED, "false")?;
    db.set_setting(KEY_ACCESS_TOKEN, "")?;
    db.set_setting(KEY_REFRESH_TOKEN, "")?;
    db.set_setting(KEY_USER, "")?;
    db.set_setting(KEY_ORGANIZATION, "")?;
    Ok(())
}

pub fn read_authenticated_user_id(db: &Database) -> AgentResult<String> {
    read_session(db)?
        .map(|session| session.user.id)
        .ok_or_else(|| AgentError::Auth("user not authenticated".into()))
}

/// Usado pelo sync worker para `Authorization: Bearer`.
pub fn read_access_token(db: &Database) -> AgentResult<Option<String>> {
    Ok(db
        .get_setting(KEY_ACCESS_TOKEN)?
        .filter(|value| !value.is_empty()))
}

fn read_setting_json<T: serde::de::DeserializeOwned>(
    db: &Database,
    key: &str,
) -> AgentResult<Option<T>> {
    Ok(db
        .get_setting(key)?
        .and_then(|json| serde_json::from_str(&json).ok()))
}

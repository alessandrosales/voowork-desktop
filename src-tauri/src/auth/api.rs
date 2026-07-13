use crate::error::{AgentError, AgentResult};
use std::time::Duration;

use super::api_models::MeResponse;
use super::constants::HTTP_TIMEOUT_SECS;
use super::http_errors::{auth_error_from_response, is_auth_failure_status};
use super::models::{AuthOrganization, AuthUser};
use super::store::AuthSession;

#[derive(Debug, serde::Serialize)]
struct LoginBody {
    auth: LoginCredentials,
}

#[derive(Debug, serde::Serialize)]
struct LoginCredentials {
    email: String,
    password: String,
}

#[derive(Debug, serde::Deserialize)]
struct LoginPayload {
    token: String,
    user: LoginUserPayload,
    #[serde(default)]
    account: Option<LoginAccountPayload>,
}

#[derive(Debug, serde::Deserialize)]
struct LoginUserPayload {
    id: String,
    #[serde(default)]
    account_id: Option<String>,
    name: String,
    email: String,
}

#[derive(Debug, serde::Deserialize)]
struct LoginAccountPayload {
    id: String,
    name: String,
}

pub struct LoginClient {
    client: reqwest::Client,
    base_url: String,
}

impl LoginClient {
    pub fn with_base_url(base_url: impl Into<String>) -> AgentResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    pub async fn fetch_session(&self, email: &str, password: &str) -> AgentResult<AuthSession> {
        let url = format!("{}/api/v1/auth/login", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&LoginBody {
                auth: LoginCredentials {
                    email: email.to_string(),
                    password: password.to_string(),
                },
            })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(auth_error_from_response(status, &body));
        }

        let payload: LoginPayload = response.json().await?;
        Ok(session_from_login_payload(payload))
    }
}

fn session_from_login_payload(payload: LoginPayload) -> AuthSession {
    let organization = organization_from_login(&payload);

    AuthSession {
        access_token: payload.token,
        refresh_token: None,
        user: AuthUser {
            id: payload.user.id,
            name: payload.user.name,
            email: payload.user.email,
        },
        organization,
    }
}

fn organization_from_login(payload: &LoginPayload) -> AuthOrganization {
    if let Some(account) = &payload.account {
        return AuthOrganization {
            id: account.id.clone(),
            name: account.name.clone(),
        };
    }

    let account_id = payload
        .user
        .account_id
        .clone()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| payload.user.id.clone());

    AuthOrganization {
        id: account_id,
        name: "Conta".into(),
    }
}

pub async fn fetch_me_profile(
    base_url: &str,
    access_token: &str,
    organization_fallback: &AuthOrganization,
) -> AgentResult<(AuthUser, AuthOrganization)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;
    let url = format!("{}/api/v1/auth/me", base_url.trim_end_matches('/'));
    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?;

    if is_auth_failure_status(response.status()) {
        return Err(AgentError::Auth("sessão expirada".into()));
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(auth_error_from_response(status, &body));
    }

    let payload: MeResponse = response.json().await?;

    if payload.id.is_empty() || payload.name.is_empty() || payload.email.is_empty() {
        return Err(AgentError::Auth("dados do usuário indisponíveis".into()));
    }

    let organization = if payload.account_id == organization_fallback.id {
        organization_fallback.clone()
    } else {
        AuthOrganization {
            id: payload.account_id,
            name: organization_fallback.name.clone(),
        }
    };

    Ok((
        AuthUser {
            id: payload.id,
            name: payload.name,
            email: payload.email,
        },
        organization,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_ID: &str = "550e8400-e29b-41d4-a716-446655440001";
    const ACCOUNT_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
    const OTHER_ACCOUNT_ID: &str = "550e8400-e29b-41d4-a716-446655440002";

    #[test]
    fn session_from_login_payload_maps_account() {
        let session = session_from_login_payload(LoginPayload {
            token: "jwt-token".into(),
            user: LoginUserPayload {
                id: USER_ID.into(),
                account_id: Some(ACCOUNT_ID.into()),
                name: "Admin".into(),
                email: "admin@admin.com".into(),
            },
            account: Some(LoginAccountPayload {
                id: ACCOUNT_ID.into(),
                name: "Admin Corp".into(),
            }),
        });

        assert_eq!(session.access_token, "jwt-token");
        assert_eq!(session.user.id, USER_ID);
        assert_eq!(session.user.email, "admin@admin.com");
        assert_eq!(session.organization.id, ACCOUNT_ID);
        assert_eq!(session.organization.name, "Admin Corp");
    }

    #[test]
    fn session_from_login_payload_falls_back_to_account_id() {
        let session = session_from_login_payload(LoginPayload {
            token: "jwt-token".into(),
            user: LoginUserPayload {
                id: USER_ID.into(),
                account_id: Some(OTHER_ACCOUNT_ID.into()),
                name: "Admin".into(),
                email: "admin@admin.com".into(),
            },
            account: None,
        });

        assert_eq!(session.organization.id, OTHER_ACCOUNT_ID);
        assert_eq!(session.organization.name, "Conta");
    }
}

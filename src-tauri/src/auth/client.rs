use crate::error::{AgentError, AgentResult};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

use super::http_errors::{auth_error_from_response, is_auth_failure_status};
use super::store::AuthSession;

const HTTP_TIMEOUT_SECS: u64 = 30;

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

#[derive(Debug, Deserialize)]
struct LoginUserPayload {
    id: String,
    #[serde(default)]
    account_id: Option<String>,
    name: String,
    email: String,
    #[serde(default)]
    profile: String,
}

#[derive(Debug, Deserialize)]
struct LoginAccountPayload {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct MeResponse {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub projects: Vec<ProjectSummary>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectSummary {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
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
    let org = match &payload.account {
        Some(a) => super::store::AuthOrganization {
            id: a.id.clone(),
            name: a.name.clone(),
        },
        None => {
            let id = payload.user.account_id.clone()
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| payload.user.id.clone());
            super::store::AuthOrganization {
                id,
                name: "Conta".into(),
            }
        }
    };

    AuthSession {
        access_token: payload.token,
        refresh_token: None,
        user: super::store::AuthUser {
            id: payload.user.id,
            name: payload.user.name,
            email: payload.user.email,
            profile: payload.user.profile,
        },
        organization: org,
    }
}

pub async fn fetch_me_profile(
    base_url: &str,
    access_token: &str,
) -> AgentResult<(super::store::AuthUser, super::store::AuthOrganization, Option<Vec<ProjectSummary>>)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;
    let url = format!("{}/api/v1/auth/me", base_url.trim_end_matches('/'));
    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?;

    if is_auth_failure_status(response.status()) || response.status() == StatusCode::NOT_FOUND {
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

    Ok((
        super::store::AuthUser {
            id: payload.id,
            name: payload.name,
            email: payload.email,
            profile: payload.profile,
        },
        super::store::AuthOrganization {
            id: payload.account_id,
            name: String::new(),
        },
        Some(payload.projects),
    ))
}

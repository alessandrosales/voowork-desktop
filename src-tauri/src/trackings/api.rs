use crate::auth::http_errors::auth_error_from_response;
use crate::auth::store::HTTP_TIMEOUT_SECS;
use crate::error::{AgentError, AgentResult};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ApiActiveTracking {
    pub id: String,
    pub status: String,
    pub device: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActiveTrackingsResponse {
    data: Vec<ApiActiveTracking>,
}

pub struct TrackingsClient {
    client: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl TrackingsClient {
    pub fn with_token(base_url: &str, access_token: &str) -> AgentResult<Self> {
        let token = access_token.trim();
        if token.is_empty() {
            return Err(AgentError::Auth("token ausente".into()));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token: token.to_string(),
        })
    }

    pub async fn fetch_active_for_user(&self, user_id: &str) -> AgentResult<Vec<ApiActiveTracking>> {
        let url = format!(
            "{}/api/v1/trackings?status=active&user_id={}&unpaged=true",
            self.base_url, user_id
        );
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(auth_error_from_response(status, &body));
        }

        let payload: ActiveTrackingsResponse = response.json().await?;
        Ok(payload
            .data
            .into_iter()
            .filter(|tracking| tracking.status == "active" && !tracking.id.is_empty())
            .collect())
    }

    pub async fn patch_stop(&self, tracking_id: &str, ended_at: &str) -> AgentResult<()> {
        let url = format!("{}/api/v1/trackings/{tracking_id}", self.base_url);
        let body = json!({
            "tracking": {
                "status": "inactive",
                "ended_at": ended_at,
            }
        });

        let response = self
            .client
            .patch(&url)
            .bearer_auth(&self.access_token)
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(auth_error_from_response(status, &body))
    }
}

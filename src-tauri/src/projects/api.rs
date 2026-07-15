use crate::auth::http_errors::auth_error_from_response;
use crate::auth::store::HTTP_TIMEOUT_SECS;
use crate::error::{AgentError, AgentResult};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ApiProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ApiTask {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct ProjectResponse {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct MeProjectsResponse {
    #[serde(default)]
    projects: Vec<ProjectResponse>,
}

#[derive(Debug, Deserialize)]
struct TaskResponse {
    id: String,
    name: String,
}

pub struct ProjectsClient {
    client: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl ProjectsClient {
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

    pub async fn fetch_assigned_projects(&self) -> AgentResult<Vec<ApiProject>> {
        let url = format!("{}/api/v1/auth/me", self.base_url);
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

        let payload: MeProjectsResponse = response.json().await?;

        Ok(payload
            .projects
            .into_iter()
            .filter(|project| !project.id.is_empty())
            .map(|project| ApiProject {
                id: project.id,
                name: project.name,
            })
            .collect())
    }

    pub async fn fetch_tasks(&self, project_id: &str) -> AgentResult<Vec<ApiTask>> {
        let url = format!(
            "{}/api/v1/projects/{}/tasks",
            self.base_url, project_id
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

        let payload: Vec<TaskResponse> = response.json().await?;
        Ok(payload
            .into_iter()
            .filter(|task| !task.id.is_empty())
            .map(|task| ApiTask {
                id: task.id,
                name: task.name,
            })
            .collect())
    }
}

//! Tipos de resposta da API Voowork (snake_case do backend).

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MeResponse {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub projects: Vec<ProjectSummary>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub name: String,
}

use crate::app_state::AppState;
use crate::error::AgentResult;
use crate::models::ProjectOption;
use std::sync::Arc;

#[tauri::command]
pub fn list_projects(state: tauri::State<'_, AppState>) -> AgentResult<Vec<ProjectOption>> {
    let db = state.db.lock();
    db.list_projects()
}

#[tauri::command]
pub async fn sync_projects(state: tauri::State<'_, AppState>) -> AgentResult<usize> {
    let (api_base_url, access_token) = {
        let db = state.db.lock();
        let token = crate::auth::read_access_token(&db)?
            .ok_or_else(|| crate::error::AgentError::Auth("usuário não autenticado".into()))?;
        (state.api_base_url.clone(), token)
    };

    crate::projects::sync_project_cache(&api_base_url, &access_token, Arc::clone(&state.db)).await
}

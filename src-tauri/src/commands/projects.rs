use crate::app_state::AppState;
use crate::error::AgentResult;
use crate::models::ProjectOption;
use std::sync::Arc;

#[tauri::command]
pub async fn list_projects(state: tauri::State<'_, AppState>) -> AgentResult<Vec<ProjectOption>> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = app_state.db.lock();
        db.list_projects()
    })
    .await
    .map_err(|err| crate::error::AgentError::Other(format!("list projects worker failed: {err}")))?
}

#[tauri::command]
pub async fn sync_projects(state: tauri::State<'_, AppState>) -> AgentResult<usize> {
    let (api_base_url, access_token, profile) = {
        let db = state.db.lock();
        let token = crate::auth::read_access_token(&db)?
            .ok_or_else(|| crate::error::AgentError::Auth("usuário não autenticado".into()))?;
        let user_profile = crate::auth::read_session_identity(&db)?
            .map(|identity| identity.user.profile)
            .unwrap_or_default();
        (state.api_base_url.clone(), token, user_profile)
    };

    crate::projects::sync_project_cache(&api_base_url, &access_token, &profile, Arc::clone(&state.db)).await
}

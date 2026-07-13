use std::sync::Arc;

use crate::app_state::AppState;
use crate::error::AgentResult;

use super::models::{AuthState, LoginRequest};
use super::service::{
    local_auth_state, login as sign_in, logout as sign_out,
    validate_auth_session as validate_auth_with_api,
};

#[tauri::command]
pub async fn login(
    state: tauri::State<'_, AppState>,
    request: LoginRequest,
) -> AgentResult<AuthState> {
    sign_in(&state.api_base_url, Arc::clone(&state.db), request).await
}

#[tauri::command]
pub fn logout(state: tauri::State<'_, AppState>) -> AgentResult<()> {
    let db = state.db.lock();
    sign_out(&db)
}

#[tauri::command]
pub fn get_auth_state(state: tauri::State<'_, AppState>) -> AgentResult<AuthState> {
    let db = state.db.lock();
    local_auth_state(&db)
}

#[tauri::command]
pub async fn validate_auth_session(
    state: tauri::State<'_, AppState>,
) -> AgentResult<AuthState> {
    validate_auth_with_api(&state.api_base_url, Arc::clone(&state.db)).await
}

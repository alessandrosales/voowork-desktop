use std::sync::Arc;

use parking_lot::Mutex;
use tauri::Emitter;

use crate::app_state::AppState;
use crate::db::Database;
use crate::error::{AgentError, AgentResult};
use crate::projects;
use crate::sync::EVENT_AUTH_SESSION_EXPIRED;
use crate::tracking::TrackingManager;
use crate::tray::{refresh_tray_ui, EVENT_AUTH_LOGGED_OUT};
use crate::windows::hide_mini_timer;

use super::client::{self, LoginClient};
use super::store::{
    clear_session as clear_session_store, persist_session, read_auth_state, read_session,
    resolve_api_url, AuthSession, AuthState, LoginRequest,
};

#[tauri::command]
pub async fn login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: LoginRequest,
) -> AgentResult<AuthState> {
    let api_base_url = resolve_api_url(&state.api_base_url)?;
    let client = LoginClient::with_base_url(&api_base_url)?;
    let session = client.fetch_session(&request.email, &request.password).await?;

    let db = Arc::clone(&state.db);
    {
        let db_guard = db.lock();
        persist_session(&db_guard, &session)?;
        projects::invalidate_project_cache_if_org_changed(&db_guard, &session.organization.id)?;

        db_guard.set_setting("selected_project_id", "")?;
        db_guard.set_setting("selected_task_id", "")?;
    }

    if let Err(err) = projects::sync_project_cache(&api_base_url, &session.access_token, &session.user.profile, Arc::clone(&db)).await {
        log::warn!("project cache sync failed after login: {err}");
    }

    state.tracking_manager.set_session_authenticated(true);
    let auth_state = session.to_auth_state();
    let _ = refresh_tray_ui(&app);
    Ok(auth_state)
}

#[tauri::command]
pub async fn logout(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> AgentResult<()> {
    let app_state = state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        if app_state.tracking_manager.status().active {
            if let Err(err) = app_state.tracking_manager.stop_tracking() {
                log::warn!("stop tracking before logout failed: {err}");
            }
        }
        perform_logout(&app_state)
    })
    .await
    .map_err(|err| AgentError::Other(format!("logout worker failed: {err}")))??;

    hide_mini_timer(&app);
    let _ = refresh_tray_ui(&app);
    let _ = app.emit(EVENT_AUTH_LOGGED_OUT, ());
    Ok(())
}

pub fn perform_logout(state: &AppState) -> AgentResult<()> {
    if let Err(err) = super::token_store::clear_access_token() {
        log::warn!("failed to clear access token from credential store: {err}");
    }

    {
        let db = state.db.lock();
        db.set_setting(super::store::KEY_AUTHENTICATED, "false")?;
        db.set_setting(super::store::KEY_ACCESS_TOKEN, "")?;
        db.set_setting(super::store::KEY_USER, "")?;
        db.set_setting(super::store::KEY_ORGANIZATION, "")?;
    }

    state.tracking_manager.set_session_authenticated(false);
    Ok(())
}

pub(crate) fn clear_invalidated_session(
    db: &Arc<Mutex<Database>>,
    tracking_manager: &TrackingManager,
) {
    {
        let db_guard = db.lock();
        if let Err(err) = clear_session_store(&db_guard) {
            log::warn!("failed to clear session after auth invalidation: {err}");
        }
    }
    tracking_manager.set_session_authenticated(false);
}

#[tauri::command]
pub async fn get_auth_state(state: tauri::State<'_, AppState>) -> AgentResult<AuthState> {
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = app_state.db.lock();
        read_auth_state(&db)
    })
    .await
    .map_err(|err| AgentError::Other(format!("get auth state worker failed: {err}")))?
}

#[tauri::command]
pub async fn validate_auth_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AgentResult<AuthState> {
    let db = Arc::clone(&state.db);
    let session = {
        let db_guard = db.lock();
        read_session(&db_guard)?
    };

    let Some(session) = session else {

        log::info!("validate_auth_session: no access token found — session is not authenticated");
        state.tracking_manager.set_session_authenticated(false);
        return Ok(AuthState::signed_out());
    };

    let api_base_url = match resolve_api_url(&state.api_base_url) {
        Ok(url) => url,
        Err(_) => return Ok(session.to_auth_state()),
    };

    match client::fetch_me_profile(&api_base_url, &session.access_token).await {
        Ok((user, mut org, _projects_list)) => {

            if org.name.is_empty() && !session.organization.name.is_empty() {
                org.name = session.organization.name.clone();
            }
            let updated = AuthSession {
                access_token: session.access_token,
                user,
                organization: org,
            };
            {
                let db_guard = db.lock();
                persist_session(&db_guard, &updated)?;
            }
            if let Err(err) = projects::refresh_project_cache_if_stale(&api_base_url, &updated.user.profile, Arc::clone(&db)).await {
                log::warn!("project cache refresh after auth validate failed: {err}");
            }
            state.tracking_manager.set_session_authenticated(true);
            Ok(updated.to_auth_state())
        }
        Err(AgentError::Auth(msg)) => {
            log::info!("auth session invalidated: {msg}");
            let app_state = state.inner().clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                clear_invalidated_session(&app_state.db, &app_state.tracking_manager);
            })
            .await;
            let _ = app.emit(EVENT_AUTH_SESSION_EXPIRED, ());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screenshot::ScreenshotCapture;
    use std::time::Duration;

    fn test_state() -> (Arc<Mutex<Database>>, TrackingManager) {
        let dir = std::env::temp_dir().join(format!("voowork-auth-test-{}", uuid::Uuid::new_v4()));
        let db = Arc::new(Mutex::new(Database::open(dir.clone()).unwrap()));
        let screenshot = ScreenshotCapture::new(dir.join("screenshots")).unwrap();
        let manager = TrackingManager::new(Arc::clone(&db), screenshot);
        (db, manager)
    }

    #[test]
    fn clear_invalidated_session_does_not_deadlock_on_db_mutex() {
        let (db, manager) = test_state();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            clear_invalidated_session(&db, &manager);
            let _ = tx.send(());
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("clear_invalidated_session deadlocked: db guard alive across set_session_authenticated");
    }
}

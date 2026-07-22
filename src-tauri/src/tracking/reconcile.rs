use std::sync::Arc;

use tauri::AppHandle;

use crate::app_state::AppState;
use crate::auth::{read_access_token, read_session_identity};
use crate::crypto::DeviceKeys;
use crate::error::{AgentError, AgentResult};
use crate::sync::SYNC_FLUSH_TIMEOUT_SECS;
use crate::trackings::{ApiActiveTracking, TrackingsClient};

use super::TrackingManager;

#[derive(Debug, Clone, Default)]
pub struct RemoteActiveSnapshot {
    pub tracking_id: Option<String>,
    pub device: Option<String>,
}

pub fn snapshot_from_actives(actives: &[ApiActiveTracking]) -> RemoteActiveSnapshot {
    let first = actives.first();
    RemoteActiveSnapshot {
        tracking_id: first.map(|t| t.id.clone()),
        device: first.and_then(|t| t.device.clone()),
    }
}

impl TrackingManager {
    pub fn set_remote_active_cache(&self, actives: &[ApiActiveTracking]) {
        *self.remote_active.lock() = snapshot_from_actives(actives);
    }

    pub fn clear_remote_active_cache(&self) {
        *self.remote_active.lock() = RemoteActiveSnapshot::default();
    }
}

pub async fn reconcile_after_auth(
    tracking_manager: &TrackingManager,
    api_base_url: &str,
    access_token: &str,
    user_id: &str,
) -> AgentResult<()> {
    let actives = match fetch_active_resilient(api_base_url, access_token, user_id).await {
        Ok(actives) => actives,
        Err(AgentError::Http(err)) if err.is_connect() || err.is_timeout() => {
            log::warn!("reconcile skipped (offline): {err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };

    tracking_manager.set_remote_active_cache(&actives);

    let local = tracking_manager.active.lock().clone();
    let Some(local_tracking) = local else {
        return Ok(());
    };

    let Some(remote) = actives.first() else {
        return Ok(());
    };

    if remote.id != local_tracking.tracking_id {
        log::warn!(
            "reconcile: local tracking {} differs from remote {} — stopping local session",
            local_tracking.tracking_id,
            remote.id
        );
        tracking_manager.stop_tracking()?;
    }

    Ok(())
}

pub async fn prepare_before_start(app: &AppHandle, state: &AppState) -> AgentResult<()> {
    let (user_id, device_name, access_token) = {
        let db = state.db.lock();
        let identity = read_session_identity(&db)?
            .ok_or_else(|| AgentError::Auth("user not authenticated".into()))?;
        let device_name = DeviceKeys::device_name(db.conn())?;
        let access_token = read_access_token(&db)?;
        (identity.user.id, device_name, access_token)
    };

    let Some(access_token) = access_token else {
        return Ok(());
    };

    if state.sync_worker.is_enabled() {
        state
            .sync_worker
            .flush(
                Arc::clone(&state.db),
                app.clone(),
                SYNC_FLUSH_TIMEOUT_SECS,
            )
            .await;
    }

    let actives = match fetch_active_resilient(&state.api_base_url, &access_token, &user_id).await {
        Ok(actives) => actives,
        Err(AgentError::Http(err)) if err.is_connect() || err.is_timeout() => {
            log::warn!("remote active check skipped (offline): {err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };

    state.tracking_manager.set_remote_active_cache(&actives);

    let local_tracking_id = state
        .tracking_manager
        .active
        .lock()
        .as_ref()
        .map(|tracking| tracking.tracking_id.clone());

    for remote in &actives {
        if local_tracking_id.as_deref() == Some(remote.id.as_str()) {
            continue;
        }

        let remote_device = remote.device.as_deref().unwrap_or("");
        if remote_device != device_name.as_str() {
            let label = remote.device.clone().unwrap_or_else(|| "outro dispositivo".into());
            return Err(AgentError::Session(format!(
                "Já existe um rastreamento ativo em outro dispositivo ({label}). Encerre-o antes de iniciar aqui."
            )));
        }

        let ended_at = chrono::Utc::now().to_rfc3339();
        let client = TrackingsClient::with_token(&state.api_base_url, &access_token)?;
        client.patch_stop(&remote.id, &ended_at).await?;
        log::info!(
            "handoff: closed remote orphan tracking {} on device {}",
            remote.id,
            device_name
        );
    }

    state.tracking_manager.clear_remote_active_cache();
    Ok(())
}

async fn fetch_active_resilient(
    api_base_url: &str,
    access_token: &str,
    user_id: &str,
) -> AgentResult<Vec<ApiActiveTracking>> {
    let client = TrackingsClient::with_token(api_base_url, access_token)?;
    client.fetch_active_for_user(user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_from_actives_uses_first_entry() {
        let actives = vec![
            ApiActiveTracking {
                id: "t-1".into(),
                status: "active".into(),
                device: Some("desktop-a".into()),
            },
            ApiActiveTracking {
                id: "t-2".into(),
                status: "active".into(),
                device: Some("desktop-b".into()),
            },
        ];

        let snapshot = snapshot_from_actives(&actives);
        assert_eq!(snapshot.tracking_id.as_deref(), Some("t-1"));
        assert_eq!(snapshot.device.as_deref(), Some("desktop-a"));
    }

    #[test]
    fn snapshot_from_empty_actives_is_default() {
        let snapshot = snapshot_from_actives(&[]);
        assert!(snapshot.tracking_id.is_none());
        assert!(snapshot.device.is_none());
    }
}

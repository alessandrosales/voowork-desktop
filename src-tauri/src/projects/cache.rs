use chrono::{DateTime, Utc};

use crate::auth::{read_organization_id, KEY_AUTHENTICATED};
use crate::db::Database;
use crate::error::{AgentError, AgentResult};

use super::constants::{
    PROJECT_CACHE_TTL_SECS, SETTING_PROJECT_CACHE_ORG_ID, SETTING_PROJECT_CACHE_SYNCED_AT,
};

/// Keys persisted in `settings` for the user's project/task selection.
/// Cleared on org change so the UI doesn't show stale UUIDs as selected.
const SETTING_SELECTED_PROJECT_ID: &str = "selected_project_id";
const SETTING_SELECTED_TASK_ID: &str = "selected_task_id";

pub fn requires_populated_project_cache(db: &Database) -> AgentResult<bool> {
    Ok(db
        .get_setting(KEY_AUTHENTICATED)?
        .is_some_and(|value| value == "true"))
}

pub fn cache_needs_refresh(db: &Database) -> AgentResult<bool> {
    if !requires_populated_project_cache(db)? {
        return Ok(false);
    }

    let Some(synced_at) = db.get_setting(SETTING_PROJECT_CACHE_SYNCED_AT)? else {
        return Ok(true);
    };

    if synced_at.is_empty() {
        return Ok(true);
    }

    let parsed = DateTime::parse_from_rfc3339(&synced_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now() - chrono::Duration::seconds(PROJECT_CACHE_TTL_SECS as i64 + 1));

    Ok(Utc::now().signed_duration_since(parsed).num_seconds() >= PROJECT_CACHE_TTL_SECS as i64)
}

pub fn invalidate_if_org_changed(db: &Database, organization_id: &str) -> AgentResult<bool> {
    let cached_org = db.get_setting(SETTING_PROJECT_CACHE_ORG_ID)?;
    if cached_org.as_deref() == Some(organization_id) {
        return Ok(false);
    }

    db.clear_projects()?;
    db.set_setting(SETTING_PROJECT_CACHE_SYNCED_AT, "")?;
    db.set_setting(SETTING_PROJECT_CACHE_ORG_ID, "")?;
    // Clear stale project/task selection so the UI doesn't display old
    // UUIDs in the Select dropdown when the org's project set changes.
    db.set_setting(SETTING_SELECTED_PROJECT_ID, "")?;
    db.set_setting(SETTING_SELECTED_TASK_ID, "")?;
    Ok(true)
}

pub fn mark_cache_synced(db: &Database, organization_id: &str) -> AgentResult<()> {
    db.set_setting(
        SETTING_PROJECT_CACHE_SYNCED_AT,
        &Utc::now().to_rfc3339(),
    )?;
    db.set_setting(SETTING_PROJECT_CACHE_ORG_ID, organization_id)?;
    Ok(())
}

pub fn ensure_can_start_tracking(db: &Database, project_id: &str) -> AgentResult<()> {
    if !requires_populated_project_cache(db)? {
        return Ok(());
    }

    if db.project_count()? == 0 {
        return Err(AgentError::Session(
            "nenhum projeto atribuído — sincronize com a API ou solicite acesso ao gestor".into(),
        ));
    }

    let projects = db.list_projects()?;
    if !projects.iter().any(|project| project.id == project_id) {
        return Err(AgentError::Session(
            "projeto não encontrado no cache local".into(),
        ));
    }

    Ok(())
}

pub fn organization_id_from_session(db: &Database) -> AgentResult<String> {
    read_organization_id(db)?
        .ok_or_else(|| AgentError::Auth("usuário não autenticado".into()))
}

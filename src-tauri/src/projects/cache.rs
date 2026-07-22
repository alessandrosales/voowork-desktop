use chrono::{DateTime, Utc};

use crate::auth::{read_organization_id, KEY_AUTHENTICATED};
use crate::db::Database;
use crate::error::{AgentError, AgentResult};

use super::constants::{
    PROJECT_CACHE_TTL_SECS, SETTING_PROJECT_CACHE_ORG_ID, SETTING_PROJECT_CACHE_SYNCED_AT,
};

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

pub fn ensure_task_belongs_to_project(
    db: &Database,
    project_id: &str,
    task_id: &str,
) -> AgentResult<()> {
    if !requires_populated_project_cache(db)? {
        return Ok(());
    }

    let task_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1",
            rusqlite::params![project_id],
            |row| row.get(0),
        )
        .map_err(AgentError::from)?;

    if task_count == 0 {
        return Ok(());
    }

    match db.task_name(project_id, task_id)? {
        Some(_) => Ok(()),
        None => Err(AgentError::Session(format!(
            "tarefa '{task_id}' não pertence ao projeto '{project_id}'"
        ))),
    }
}

pub fn organization_id_from_session(db: &Database) -> AgentResult<String> {
    read_organization_id(db)?
        .ok_or_else(|| AgentError::Auth("usuário não autenticado".into()))
}

#[cfg(test)]
mod task_validation_tests {
    use super::*;
    use crate::auth::KEY_AUTHENTICATED;
    use crate::db::Database;
    use std::path::PathBuf;

    fn test_db() -> Database {
        let dir = PathBuf::from(std::env::temp_dir())
            .join(format!("voowork-m7-test-{}", uuid::Uuid::new_v4()));
        let db = Database::open(dir).unwrap();

        db.set_setting(KEY_AUTHENTICATED, "true").unwrap();
        db
    }

    fn insert_project(db: &Database, id: &str, name: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO projects (id, account_id, name, featured, created_at, updated_at)
                 VALUES (?1, 'acc-1', ?2, 0, ?3, ?3)",
                rusqlite::params![id, name, now],
            )
            .unwrap();
    }

    fn insert_task(db: &Database, project_id: &str, id: &str, name: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO tasks (id, account_id, project_id, name, position, created_at, updated_at)
                 VALUES (?1, 'acc-1', ?2, ?3, 0, ?4, ?4)",
                rusqlite::params![id, project_id, name, now],
            )
            .unwrap();
    }

    #[test]
    fn ensure_task_belongs_to_project_accepts_valid() {
        let db = test_db();
        insert_project(&db, "proj-1", "Project 1");
        insert_task(&db, "proj-1", "task-1", "Task 1");

        let result = ensure_task_belongs_to_project(&db, "proj-1", "task-1");
        assert!(result.is_ok(), "valid task should be accepted");
    }

    #[test]
    fn ensure_task_belongs_to_project_rejects_foreign() {
        let db = test_db();
        insert_project(&db, "proj-1", "Project 1");
        insert_task(&db, "proj-1", "task-1", "Task 1");
        insert_project(&db, "proj-2", "Project 2");
        insert_task(&db, "proj-2", "task-2", "Task 2");

        let result = ensure_task_belongs_to_project(&db, "proj-1", "task-2");
        assert!(result.is_err(), "foreign task should be rejected");
    }

    #[test]
    fn ensure_task_belongs_to_project_rejects_unknown() {
        let db = test_db();
        insert_project(&db, "proj-1", "Project 1");
        insert_task(&db, "proj-1", "task-1", "Task 1");

        let result = ensure_task_belongs_to_project(&db, "proj-1", "task-999");
        assert!(result.is_err(), "unknown task should be rejected");
    }

    #[test]
    fn ensure_task_belongs_to_project_allows_empty_cache() {
        let db = test_db();
        insert_project(&db, "proj-1", "Project 1");

        let result = ensure_task_belongs_to_project(&db, "proj-1", "any-task");
        assert!(result.is_ok(), "empty task cache should allow any task");
    }
}

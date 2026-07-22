mod api;
mod cache;
mod constants;

use crate::db::Database;
use crate::error::AgentResult;
use crate::models::TaskOption;
use parking_lot::Mutex;
use std::sync::Arc;

pub use api::ProjectsClient;
pub use cache::{ensure_can_start_tracking, ensure_task_belongs_to_project};

pub fn invalidate_project_cache_if_org_changed(
    db: &Database,
    organization_id: &str,
) -> AgentResult<bool> {
    cache::invalidate_if_org_changed(db, organization_id)
}

pub async fn sync_project_cache(
    api_base_url: &str,
    access_token: &str,
    db: Arc<Mutex<Database>>,
) -> AgentResult<usize> {
    let organization_id = {
        let db_guard = db.lock();
        cache::organization_id_from_session(&db_guard)?
    };

    let client = ProjectsClient::with_token(api_base_url, access_token)?;
    // GET /api/v1/projects is account-scoped and filtered by membership on the backend.
    let projects = client.fetch_visible_projects().await?;
    let mut entries = Vec::with_capacity(projects.len());

    for (index, project) in projects.iter().enumerate() {
        let task_options = match client.fetch_tasks(&project.id).await {
            Ok(tasks) => tasks
                .into_iter()
                .map(|task| TaskOption {
                    id: task.id,
                    name: task.name,
                })
                .collect(),
            Err(err) => {
                log::warn!(
                    "failed to fetch tasks for project {} ({}): {err}",
                    project.name, project.id
                );
                Vec::new()
            }
        };
        entries.push((project.id.clone(), project.name.clone(), task_options, index as i64));
    }

    let fetched_ids: Vec<String> = entries.iter().map(|(id, _, _, _)| id.clone()).collect();
    let count = entries.len();

    {
        let db_guard = db.lock();
        cache::invalidate_if_org_changed(&db_guard, &organization_id)?;
        for (id, name, tasks, _sort_order) in entries {
            db_guard.upsert_project(&organization_id, &id, &name, &tasks, false)?;
        }
        db_guard.remove_projects_not_in(&fetched_ids)?;
        cache::mark_cache_synced(&db_guard, &organization_id)?;
    }

    Ok(count)
}

pub async fn refresh_project_cache_if_stale(
    api_base_url: &str,
    db: Arc<Mutex<Database>>,
) -> AgentResult<()> {
    let (needs_refresh, access_token) = {
        let db_guard = db.lock();
        let needs_refresh = cache::cache_needs_refresh(&db_guard)?;
        let access_token = crate::auth::read_access_token(&db_guard)?;
        (needs_refresh, access_token)
    };

    if !needs_refresh {
        return Ok(());
    }

    let Some(access_token) = access_token else {
        return Ok(());
    };

    if let Err(err) = sync_project_cache(api_base_url, &access_token, Arc::clone(&db)).await {
        log::warn!("background project cache refresh failed: {err}");
    }

    Ok(())
}

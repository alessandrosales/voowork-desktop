use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::{ProjectOption, TaskOption};
use rusqlite::{params, OptionalExtension};

impl Database {
    pub fn list_projects(&self) -> AgentResult<Vec<ProjectOption>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, tasks_json FROM project_cache ORDER BY sort_order ASC, name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let tasks_json: String = row.get(2)?;
            let tasks: Vec<TaskOption> =
                serde_json::from_str(&tasks_json).unwrap_or_default();
            Ok(ProjectOption { id, name, tasks })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }

    pub fn upsert_project(
        &self,
        id: &str,
        name: &str,
        tasks: &[TaskOption],
        sort_order: i64,
    ) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let tasks_json = serde_json::to_string(tasks)?;
        self.conn.execute(
            "INSERT INTO project_cache (id, name, tasks_json, sort_order, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               tasks_json = excluded.tasks_json,
               sort_order = excluded.sort_order,
               updated_at = excluded.updated_at",
            params![id, name, tasks_json, sort_order, now],
        )?;
        Ok(())
    }

    pub fn project_name(&self, project_id: &str) -> AgentResult<String> {
        let name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM project_cache WHERE id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(name.unwrap_or_else(|| project_id.to_string()))
    }

    pub fn task_name(&self, project_id: &str, task_id: &str) -> AgentResult<Option<String>> {
        let projects = self.list_projects()?;
        Ok(projects
            .into_iter()
            .find(|p| p.id == project_id)
            .and_then(|p| p.tasks.into_iter().find(|t| t.id == task_id))
            .map(|t| t.name))
    }

    pub fn project_cache_count(&self) -> AgentResult<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM project_cache", [], |row| row.get(0))
            .map_err(AgentError::from)
    }

    pub fn clear_project_cache(&self) -> AgentResult<()> {
        self.conn.execute("DELETE FROM project_cache", [])?;
        Ok(())
    }

    pub fn remove_projects_not_in(&self, ids: &[String]) -> AgentResult<()> {
        if ids.is_empty() {
            self.clear_project_cache()?;
            return Ok(());
        }

        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("DELETE FROM project_cache WHERE id NOT IN ({placeholders})");
        self.conn.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
        Ok(())
    }
}

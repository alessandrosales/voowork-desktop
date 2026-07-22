use super::Database;
use crate::error::{AgentError, AgentResult};
use crate::models::{ProjectOption, TaskOption};
use rusqlite::{params, OptionalExtension};

impl Database {
    pub fn list_projects(&self) -> AgentResult<Vec<ProjectOption>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name FROM projects ORDER BY featured DESC, name ASC",
        )?;
        let project_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut projects = Vec::new();
        for row in project_rows {
            let (id, name) = row?;
            let tasks = self.list_tasks_for_project(&id)?;
            projects.push(ProjectOption { id, name, tasks });
        }

        Ok(projects)
    }

    fn list_tasks_for_project(&self, project_id: &str) -> AgentResult<Vec<TaskOption>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name FROM tasks WHERE project_id = ?1 ORDER BY position ASC, name ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(TaskOption {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentError::from)
    }

    pub fn upsert_project(
        &self,
        account_id: &str,
        id: &str,
        name: &str,
        tasks: &[TaskOption],
        featured: bool,
    ) -> AgentResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO projects (id, account_id, name, featured, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               featured = excluded.featured,
               updated_at = excluded.updated_at",
            params![id, account_id, name, featured as i64, now],
        )?;

        let mut keep_task_ids = Vec::with_capacity(tasks.len());
        for (position, task) in tasks.iter().enumerate() {
            keep_task_ids.push(task.id.clone());
            self.conn.execute(
                "INSERT INTO tasks (id, account_id, project_id, name, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   position = excluded.position,
                   updated_at = excluded.updated_at",
                params![
                    task.id,
                    account_id,
                    id,
                    task.name,
                    position as i64,
                    now
                ],
            )?;
        }

        if keep_task_ids.is_empty() {
            self.conn.execute(
                "DELETE FROM tasks WHERE project_id = ?1",
                params![id],
            )?;
        } else {
            let placeholders = std::iter::repeat_n("?", keep_task_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM tasks WHERE project_id = ?1 AND id NOT IN ({placeholders})"
            );
            let mut sql_params: Vec<&dyn rusqlite::ToSql> =
                vec![&id as &dyn rusqlite::ToSql];
            for task_id in &keep_task_ids {
                sql_params.push(task_id);
            }
            self.conn.execute(&sql, sql_params.as_slice())?;
        }

        Ok(())
    }

    pub fn project_name(&self, project_id: &str) -> AgentResult<String> {
        let name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(name.unwrap_or_else(|| project_id.to_string()))
    }

    pub fn task_name(&self, project_id: &str, task_id: &str) -> AgentResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT name FROM tasks WHERE project_id = ?1 AND id = ?2",
                params![project_id, task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AgentError::from)
    }

    pub fn project_count(&self) -> AgentResult<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .map_err(AgentError::from)
    }

    pub fn clear_projects(&self) -> AgentResult<()> {
        self.conn.execute("DELETE FROM tasks", [])?;
        self.conn.execute("DELETE FROM projects", [])?;
        Ok(())
    }

    pub fn remove_projects_not_in(&self, ids: &[String]) -> AgentResult<()> {
        if ids.is_empty() {
            return self.clear_projects();
        }

        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!("DELETE FROM tasks WHERE project_id NOT IN ({placeholders})");
        self.conn.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;

        let sql = format!("DELETE FROM projects WHERE id NOT IN ({placeholders})");
        self.conn.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
        Ok(())
    }
}

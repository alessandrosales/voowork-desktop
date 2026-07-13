use super::Database;
use crate::error::AgentResult;
use crate::models::SessionRow;
use rusqlite::{params, OptionalExtension};

impl Database {
    pub fn list_recent_sessions(&self, limit: i64) -> AgentResult<Vec<SessionRow>> {
        self.list_sessions_internal(limit, None)
    }

    pub fn list_sessions(&self, limit: i64, offset: i64) -> AgentResult<Vec<SessionRow>> {
        self.list_sessions_internal(limit, Some(offset))
    }

    fn list_sessions_internal(
        &self,
        limit: i64,
        offset: Option<i64>,
    ) -> AgentResult<Vec<SessionRow>> {
        let mut sessions = Vec::new();

        if let Some(offset) = offset {
            let mut stmt = self.conn.prepare(
                "SELECT id, project_id, task_id, started_at, ended_at, monotonic_started_ns,
                        monotonic_ended_ns, status
                 FROM sessions
                 ORDER BY started_at DESC
                 LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit, offset], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                sessions.push(self.map_session_row(row?)?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, project_id, task_id, started_at, ended_at, monotonic_started_ns,
                        monotonic_ended_ns, status
                 FROM sessions
                 ORDER BY started_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                sessions.push(self.map_session_row(row?)?);
            }
        }

        Ok(sessions)
    }

    fn map_session_row(
        &self,
        (
            id,
            project_id,
            task_id,
            started_at,
            ended_at,
            monotonic_started_ns,
            monotonic_ended_ns,
            status,
        ): (
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            i64,
            Option<i64>,
            String,
        ),
    ) -> AgentResult<SessionRow> {
        let project_name = self.project_name(&project_id)?;
        let task_name = match &task_id {
            Some(tid) => self.task_name(&project_id, tid)?,
            None => None,
        };
        let duration_seconds = monotonic_ended_ns.map(|ended| {
            ((ended - monotonic_started_ns).max(0) / 1_000_000_000) as u64
        });
        let sync_status = self.session_sync_status(&id)?;

        Ok(SessionRow {
            id,
            project_id,
            project_name,
            task_id,
            task_name,
            started_at,
            ended_at,
            duration_seconds,
            status,
            sync_status,
        })
    }

    fn session_sync_status(&self, session_id: &str) -> AgentResult<String> {
        let status: Option<String> = self
            .conn
            .query_row(
                "SELECT status FROM sync_queue
                 WHERE entity_type = 'session' AND entity_id = ?1
                 ORDER BY created_at DESC LIMIT 1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(status.unwrap_or_else(|| "local".into()))
    }

    pub fn screenshot_stats_for_session(
        &self,
        session_id: &str,
    ) -> AgentResult<(u64, Option<String>)> {
        let (count, last_captured_at): (i64, Option<String>) = self.conn.query_row(
            "SELECT COUNT(*), MAX(captured_at) FROM screenshots WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((count.max(0) as u64, last_captured_at))
    }
}

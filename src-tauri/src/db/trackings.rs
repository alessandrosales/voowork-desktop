use super::Database;
use crate::error::AgentResult;
use crate::models::TrackingRow;
use rusqlite::{params, OptionalExtension};

impl Database {
    #[allow(clippy::too_many_arguments)]
    pub fn insert_tracking(
        &self,
        id: &str,
        account_id: &str,
        project_id: &str,
        task_id: &str,
        user_id: &str,
        device: Option<&str>,
        started_at: &str,
    ) -> AgentResult<()> {
        let now = started_at.to_string();
        self.conn.execute(
            "INSERT INTO trackings (
                id, account_id, project_id, task_id, user_id, status, device,
                started_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?7, ?8, ?8)",
            params![id, account_id, project_id, task_id, user_id, device, started_at, now],
        )?;
        Ok(())
    }

    pub fn finalize_tracking(&self, id: &str, ended_at: &str) -> AgentResult<()> {
        self.conn.execute(
            "UPDATE trackings
             SET status = 'inactive', ended_at = ?2, updated_at = ?2
             WHERE id = ?1",
            params![id, ended_at],
        )?;
        Ok(())
    }

    pub fn list_active_tracking_ids(&self) -> AgentResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM trackings
             WHERE status = 'active' AND ended_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }

    pub fn list_open_tracking_apps(
        &self,
        tracking_id: &str,
    ) -> AgentResult<Vec<crate::models::TrackingAppRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tracking_id, name, started_at, ended_at
             FROM tracking_apps
             WHERE tracking_id = ?1 AND ended_at IS NULL
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(params![tracking_id], map_tracking_app_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }

    pub fn list_open_tracking_sites(
        &self,
        tracking_id: &str,
    ) -> AgentResult<Vec<crate::models::TrackingSiteRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tracking_id, address, started_at, ended_at
             FROM tracking_sites
             WHERE tracking_id = ?1 AND ended_at IS NULL
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(params![tracking_id], |row| {
            Ok(crate::models::TrackingSiteRow {
                id: row.get(0)?,
                tracking_id: row.get(1)?,
                address: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }

    pub fn list_trackings(&self, limit: i64, offset: i64) -> AgentResult<Vec<TrackingRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, task_id, started_at, ended_at, status
             FROM trackings
             ORDER BY started_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut trackings = Vec::new();
        for row in rows {
            let (id, project_id, task_id, started_at, ended_at, status) = row?;
            let project_name = self.project_name(&project_id)?;
            let task_name = self
                .task_name(&project_id, &task_id)?
                .unwrap_or_else(|| task_id.clone());
            let sync_status = self.tracking_sync_status(&id)?;
            let duration_seconds = self.tracking_duration_seconds(&started_at, ended_at.as_deref())?;

            trackings.push(TrackingRow {
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
            });
        }

        Ok(trackings)
    }

    fn tracking_sync_status(&self, tracking_id: &str) -> AgentResult<String> {
        let status: Option<String> = self
            .conn
            .query_row(
                "SELECT status FROM sync_queue
                 WHERE entity_type = 'tracking' AND entity_id = ?1
                 ORDER BY created_at DESC LIMIT 1",
                params![tracking_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(status.unwrap_or_else(|| "local".into()))
    }

    fn tracking_duration_seconds(
        &self,
        started_at: &str,
        ended_at: Option<&str>,
    ) -> AgentResult<Option<u64>> {
        let Some(ended_at) = ended_at else {
            return Ok(None);
        };
        let started = chrono::DateTime::parse_from_rfc3339(started_at)
            .map_err(|e| crate::error::AgentError::Other(e.to_string()))?;
        let ended = chrono::DateTime::parse_from_rfc3339(ended_at)
            .map_err(|e| crate::error::AgentError::Other(e.to_string()))?;
        Ok(Some((ended - started).num_seconds().max(0) as u64))
    }

    pub fn estimate_tracking_ended_at(&self, tracking_id: &str) -> AgentResult<Option<String>> {
        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT MAX(ts) FROM (
                    SELECT MAX(captured_at) as ts FROM tracking_screenshots WHERE tracking_id = ?1
                    UNION ALL
                    SELECT MAX(ended_at) as ts FROM tracking_peripheral_events WHERE tracking_id = ?1
                )",
                params![tracking_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result)
    }

    pub fn get_tracking_started_at(&self, tracking_id: &str) -> AgentResult<String> {
        self.conn
            .query_row(
                "SELECT started_at FROM trackings WHERE id = ?1",
                params![tracking_id],
                |row| row.get(0),
            )
            .map_err(crate::error::AgentError::from)
    }

    pub fn get_tracking_site(&self, site_id: &str) -> AgentResult<crate::models::TrackingSiteRow> {
        self.conn.query_row(
            "SELECT id, tracking_id, address, started_at, ended_at
             FROM tracking_sites WHERE id = ?1",
            params![site_id],
            |row| {
                Ok(crate::models::TrackingSiteRow {
                    id: row.get(0)?,
                    tracking_id: row.get(1)?,
                    address: row.get(2)?,
                    started_at: row.get(3)?,
                    ended_at: row.get(4)?,
                })
            },
        )
        .map_err(crate::error::AgentError::from)
    }

    pub fn get_tracking_app(&self, app_id: &str) -> AgentResult<crate::models::TrackingAppRow> {
        self.conn.query_row(
            "SELECT id, tracking_id, name, started_at, ended_at
             FROM tracking_apps WHERE id = ?1",
            params![app_id],
            |row| {
                Ok(crate::models::TrackingAppRow {
                    id: row.get(0)?,
                    tracking_id: row.get(1)?,
                    name: row.get(2)?,
                    started_at: row.get(3)?,
                    ended_at: row.get(4)?,
                })
            },
        )
        .map_err(crate::error::AgentError::from)
    }

    pub fn list_tracking_apps(
        &self,
        limit: i64,
        offset: i64,
        tracking_id: Option<&str>,
    ) -> AgentResult<Vec<crate::models::TrackingAppRow>> {
        let (sql, filter): (&str, Option<&str>) = if tracking_id.is_some() {
            (
                "SELECT id, tracking_id, name, started_at, ended_at
                 FROM tracking_apps
                 WHERE tracking_id = ?3
                 ORDER BY started_at DESC
                 LIMIT ?1 OFFSET ?2",
                tracking_id,
            )
        } else {
            (
                "SELECT id, tracking_id, name, started_at, ended_at
                 FROM tracking_apps
                 ORDER BY started_at DESC
                 LIMIT ?1 OFFSET ?2",
                None,
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(tracking_id) = filter {
            stmt.query_map(params![limit, offset, tracking_id], map_tracking_app_row)?
        } else {
            stmt.query_map(params![limit, offset], map_tracking_app_row)?
        };

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }

    pub fn list_tracking_sites(
        &self,
        limit: i64,
        offset: i64,
        tracking_id: Option<&str>,
    ) -> AgentResult<Vec<crate::models::TrackingSiteRow>> {
        let (sql, filter): (&str, Option<&str>) = if tracking_id.is_some() {
            (
                "SELECT id, tracking_id, address, started_at, ended_at
                 FROM tracking_sites
                 WHERE tracking_id = ?3
                 ORDER BY started_at DESC
                 LIMIT ?1 OFFSET ?2",
                tracking_id,
            )
        } else {
            (
                "SELECT id, tracking_id, address, started_at, ended_at
                 FROM tracking_sites
                 ORDER BY started_at DESC
                 LIMIT ?1 OFFSET ?2",
                None,
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = if let Some(tracking_id) = filter {
            stmt.query_map(params![limit, offset, tracking_id], map_tracking_site_row)?
        } else {
            stmt.query_map(params![limit, offset], map_tracking_site_row)?
        };

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }
}

fn map_tracking_site_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::models::TrackingSiteRow> {
    Ok(crate::models::TrackingSiteRow {
        id: row.get(0)?,
        tracking_id: row.get(1)?,
        address: row.get(2)?,
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
    })
}

fn map_tracking_app_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::models::TrackingAppRow> {
    Ok(crate::models::TrackingAppRow {
        id: row.get(0)?,
        tracking_id: row.get(1)?,
        name: row.get(2)?,
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
    })
}

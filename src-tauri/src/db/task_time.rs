use super::Database;
use crate::error::{AgentError, AgentResult};
use chrono::{DateTime, Utc};
use rusqlite::params;

pub const TIME_CATEGORY_ACTIVE: &str = "active";
pub const TIME_CATEGORY_INACTIVITY: &str = "inactivity";

impl Database {
    pub fn get_task_active_seconds(&self, task_id: &str) -> AgentResult<u64> {
        let seconds: Option<i64> = self.conn.query_row(
            "SELECT active_seconds FROM task_time_totals WHERE task_id = ?1",
            params![task_id],
            |row| row.get(0),
        ).ok();
        Ok(seconds.unwrap_or(0).max(0) as u64)
    }

    pub fn add_task_active_seconds(&self, task_id: &str, seconds: u64) -> AgentResult<()> {
        if seconds == 0 {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO task_time_totals (task_id, active_seconds, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET
                active_seconds = active_seconds + excluded.active_seconds,
                updated_at = excluded.updated_at",
            params![task_id, seconds as i64, now],
        )?;
        Ok(())
    }

    pub fn set_task_active_seconds(&self, task_id: &str, seconds: u64) -> AgentResult<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO task_time_totals (task_id, active_seconds, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET
                active_seconds = excluded.active_seconds,
                updated_at = excluded.updated_at",
            params![task_id, seconds as i64, now],
        )?;
        Ok(())
    }

    pub fn sum_screenshot_seconds(
        &self,
        tracking_id: &str,
        time_category: Option<&str>,
    ) -> AgentResult<u64> {
        let mut total = 0u64;

        if let Some(category) = time_category {
            let mut stmt = self.conn.prepare(
                "SELECT period_started_at, captured_at
                 FROM tracking_screenshots
                 WHERE tracking_id = ?1 AND time_category = ?2
                 ORDER BY captured_at ASC",
            )?;
            let rows = stmt.query_map(params![tracking_id, category], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                ))
            })?;
            for row in rows {
                let (period_start, captured_at) = row?;
                let Some(period_start) = period_start else {
                    continue;
                };
                total += period_duration_seconds(&period_start, &captured_at)?;
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT period_started_at, captured_at
                 FROM tracking_screenshots
                 WHERE tracking_id = ?1
                 ORDER BY captured_at ASC",
            )?;
            let rows = stmt.query_map(params![tracking_id], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                ))
            })?;
            for row in rows {
                let (period_start, captured_at) = row?;
                let Some(period_start) = period_start else {
                    continue;
                };
                total += period_duration_seconds(&period_start, &captured_at)?;
            }
        }

        Ok(total)
    }

    pub fn clear_task_time_totals(&self) -> AgentResult<()> {
        self.conn.execute("DELETE FROM task_time_totals", [])?;
        Ok(())
    }
}

pub fn period_duration_seconds(start: &str, end: &str) -> AgentResult<u64> {
    let started = parse_rfc3339(start)?;
    let ended = parse_rfc3339(end)?;
    Ok((ended - started).num_seconds().max(0) as u64)
}

pub fn parse_rfc3339(value: &str) -> AgentResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AgentError::Other(e.to_string()))
}

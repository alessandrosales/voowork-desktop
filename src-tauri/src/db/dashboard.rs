use super::Database;
use crate::error::AgentResult;
use crate::models::{ActivityChartPoint, DashboardSummary};
use rusqlite::params;

impl Database {
    pub fn dashboard_summary(&self) -> AgentResult<DashboardSummary> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let hours_today_seconds: u64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(
                    CASE
                        WHEN monotonic_ended_ns IS NOT NULL
                        THEN (monotonic_ended_ns - monotonic_started_ns) / 1000000000
                        ELSE 0
                    END
                ), 0)
                 FROM sessions
                 WHERE substr(started_at, 1, 10) = ?1 AND status != 'active'",
                params![today],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v.max(0) as u64)?;

        let sessions_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE substr(started_at, 1, 10) = ?1",
            params![today],
            |row| row.get(0),
        )?;

        let avg_activity_confidence: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(activity_score_confidence), 1.0)
             FROM activity_ticks
             WHERE substr(bucket_start, 1, 10) = ?1",
            params![today],
            |row| row.get(0),
        )?;

        let (sync_pending, _, sync_confirmed) = self.sync_queue_stats()?;

        Ok(DashboardSummary {
            hours_today_seconds,
            sessions_today,
            avg_activity_confidence,
            sync_pending,
            sync_confirmed,
        })
    }

    pub fn activity_chart(&self, range: &str) -> AgentResult<Vec<ActivityChartPoint>> {
        let filter = if range == "7d" {
            "datetime(bucket_start) >= datetime('now', '-7 days')"
        } else {
            "substr(bucket_start, 1, 10) = date('now')"
        };

        let sql = format!(
            "SELECT substr(bucket_start, 12, 2) AS hour_label,
                    SUM(mouse_events) AS mouse_total,
                    SUM(keyboard_events) AS keyboard_total
             FROM activity_ticks
             WHERE {filter}
             GROUP BY hour_label
             ORDER BY hour_label ASC"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(ActivityChartPoint {
                label: format!("{}h", row.get::<_, String>(0)?),
                mouse: row.get::<_, i64>(1)?.max(0) as u64,
                keyboard: row.get::<_, i64>(2)?.max(0) as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }
}

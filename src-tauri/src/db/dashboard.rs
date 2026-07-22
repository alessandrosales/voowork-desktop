use super::Database;
use crate::error::AgentResult;
use crate::models::{ActivityChartPoint, DashboardSummary};
use rusqlite::params;

impl Database {
    pub fn dashboard_summary(&self) -> AgentResult<DashboardSummary> {

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let hours_today_seconds: u64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(
                    CASE
                        WHEN ended_at IS NOT NULL
                        THEN CAST((julianday(ended_at) - julianday(started_at)) * 86400 AS INTEGER)
                        ELSE 0
                    END
                ), 0) - COALESCE((
                    SELECT COALESCE(SUM(COALESCE(duration_seconds, 0)), 0)
                    FROM tracking_inactivity_periods ip
                    INNER JOIN trackings t2 ON t2.id = ip.tracking_id
                    WHERE substr(t2.started_at, 1, 10) = ?1
                      AND ip.status IN ('resumed', 'classified', 'discarded')
                ), 0)
                 FROM trackings
                 WHERE substr(started_at, 1, 10) = ?1 AND status = 'inactive'",
                params![today],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v.max(0) as u64)?;

        let trackings_today: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM trackings WHERE substr(started_at, 1, 10) = ?1",
            params![today],
            |row| row.get(0),
        )?;

        let avg_activity_confidence: f64 = 1.0;

        let (sync_pending, _, sync_confirmed) = self.sync_queue_stats()?;

        Ok(DashboardSummary {
            hours_today_seconds,
            trackings_today,
            avg_activity_confidence,
            sync_pending,
            sync_confirmed,
        })
    }

    pub fn activity_chart(&self, range: &str) -> AgentResult<Vec<ActivityChartPoint>> {
        let filter = if range == "7d" {
            "datetime(started_at) >= datetime('now', '-7 days')"
        } else {
            "substr(started_at, 1, 10) = date('now')"
        };

        let sql = format!(
            "SELECT substr(started_at, 12, 2) AS hour_label,
                    SUM(CASE WHEN event = 'mouse_activity' THEN count ELSE 0 END) AS mouse_total,
                    SUM(CASE WHEN event = 'keyboard_activity' THEN count ELSE 0 END) AS keyboard_total
             FROM tracking_peripheral_events
             WHERE {filter}
             GROUP BY hour_label
             ORDER BY hour_label ASC"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(ActivityChartPoint {
                label: format!("{}h", row.get::<_, String>(0)?),
                mouse: row.get::<_, f64>(1)?.max(0.0) as u64,
                keyboard: row.get::<_, f64>(2)?.max(0.0) as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::error::AgentError::from)
    }
}

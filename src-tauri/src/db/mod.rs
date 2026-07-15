use crate::error::AgentResult;
use rusqlite::Connection;
use std::path::PathBuf;

pub mod tracking_inactivity_periods;
pub mod schema;
pub mod tracking_peripheral_events;

mod dashboard;
pub mod frontend_settings;
mod projects;
mod settings;
mod sync_queue;
mod task_time;
mod tracking_screenshots;
mod trackings;

pub use task_time::{period_duration_seconds, TIME_CATEGORY_ACTIVE, TIME_CATEGORY_INACTIVITY};

pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    pub fn open(app_data_dir: PathBuf) -> AgentResult<Self> {
        std::fs::create_dir_all(&app_data_dir)?;
        let path = app_data_dir.join("voowork-desktop.db");
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn, path };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn migrate(&self) -> AgentResult<()> {
        for sql in schema::MIGRATIONS {
            self.conn.execute_batch(sql)?;
        }
        self.ensure_tracking_screenshots_runtime_columns()?;
        self.migrate_sync_queue_entity_types()?;
        self.migrate_legacy_idle_periods()?;
        Ok(())
    }

    fn table_exists(&self, name: &str) -> bool {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    fn migrate_legacy_idle_periods(&self) -> AgentResult<()> {
        let has_legacy_idle_periods = self.table_exists("idle_periods");
        let has_new_table = self.table_exists("tracking_inactivity_periods");

        if has_legacy_idle_periods {
            if has_new_table {
                let legacy_rows: i64 = self
                    .conn
                    .query_row("SELECT COUNT(*) FROM idle_periods", [], |row| row.get(0))
                    .unwrap_or(0);

                if legacy_rows > 0 {
                    self.conn.execute_batch(
                        "INSERT OR IGNORE INTO tracking_inactivity_periods (
                            id, tracking_id, inactivity_started_at, paused_at, resumed_at,
                            duration_seconds, discarded_seconds, reclassified_seconds,
                            category, status, created_at, updated_at
                        )
                        SELECT
                            id, tracking_id, idle_started_at, paused_at, resumed_at,
                            duration_seconds, discarded_seconds, reclassified_seconds,
                            category, status, created_at, updated_at
                        FROM idle_periods;",
                    )?;
                }

                self.conn
                    .execute_batch("DROP TABLE IF EXISTS idle_periods;")?;
            } else {
                self.conn.execute_batch(
                    "ALTER TABLE idle_periods RENAME TO tracking_inactivity_periods;",
                )?;
                let has_old_column: bool = self.conn.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('tracking_inactivity_periods') WHERE name = 'idle_started_at'",
                    [],
                    |row| row.get::<_, i64>(0),
                ).map(|count| count > 0).unwrap_or(false);

                if has_old_column {
                    self.conn.execute_batch(
                        "ALTER TABLE tracking_inactivity_periods RENAME COLUMN idle_started_at TO inactivity_started_at;",
                    )?;
                }
            }
        }

        self.conn.execute_batch(
            "UPDATE sync_queue SET entity_type = 'tracking_inactivity_period'
             WHERE entity_type = 'idle_period';
             UPDATE settings SET key = 'tracking_inactivity_threshold_minutes'
             WHERE key = 'idle_threshold_minutes';
             UPDATE settings SET key = 'tracking_inactivity_profile'
             WHERE key = 'idle_profile';
             UPDATE tracking_screenshots SET time_category = 'inactivity'
             WHERE time_category = 'idle';",
        )?;
        Ok(())
    }

    fn migrate_sync_queue_entity_types(&self) -> AgentResult<()> {
        self.conn.execute_batch(
            "UPDATE sync_queue SET entity_type = 'tracking_screenshot'
             WHERE entity_type = 'screenshot';
             UPDATE sync_queue SET entity_type = 'tracking_peripheral_event'
             WHERE entity_type = 'peripheral_event';",
        )?;
        Ok(())
    }

    fn ensure_tracking_screenshots_runtime_columns(&self) -> AgentResult<()> {
        let has_period_start: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracking_screenshots') WHERE name = 'period_started_at'",
            [],
            |row| row.get::<_, i64>(0),
        ).map(|count| count > 0).unwrap_or(false);

        if !has_period_start {
            self.conn.execute_batch(
                "ALTER TABLE tracking_screenshots ADD COLUMN period_started_at TEXT;
                 ALTER TABLE tracking_screenshots ADD COLUMN time_category TEXT NOT NULL DEFAULT 'active';",
            )?;
        }

        let has_remote_path: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracking_screenshots') WHERE name = 'remote_path'",
            [],
            |row| row.get::<_, i64>(0),
        ).map(|count| count > 0).unwrap_or(false);

        if !has_remote_path {
            self.conn.execute_batch(
                "ALTER TABLE tracking_screenshots ADD COLUMN remote_path TEXT;
                 ALTER TABLE tracking_screenshots ADD COLUMN synced_at TEXT;",
            )?;
        }
        Ok(())
    }

}

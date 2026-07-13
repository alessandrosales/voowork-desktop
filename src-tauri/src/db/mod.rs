pub mod schema;

mod activity;
mod dashboard;
mod projects;
mod screenshots;
mod sessions;
mod settings;
mod sync_queue;

use crate::error::AgentResult;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    pub fn open(app_data_dir: PathBuf) -> AgentResult<Self> {
        std::fs::create_dir_all(&app_data_dir)?;
        let path = app_data_dir.join("voowork-agent.db");
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
        self.migrate_screenshot_context_columns()?;
        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> AgentResult<bool> {
        let sql = format!("PRAGMA table_info({table})");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for name in rows {
            if name? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn migrate_screenshot_context_columns(&self) -> AgentResult<()> {
        for column in ["user_id", "project_id", "task_id"] {
            if !self.column_exists("screenshots", column)? {
                self.conn.execute(
                    &format!("ALTER TABLE screenshots ADD COLUMN {column} TEXT"),
                    [],
                )?;
            }
        }

        self.conn.execute_batch(
            "UPDATE screenshots
             SET project_id = (
                   SELECT project_id FROM sessions WHERE sessions.id = screenshots.session_id
                 ),
                 task_id = (
                   SELECT task_id FROM sessions WHERE sessions.id = screenshots.session_id
                 )
             WHERE project_id IS NULL AND session_id IS NOT NULL",
        )?;

        Ok(())
    }
}

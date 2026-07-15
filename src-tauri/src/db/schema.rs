pub const MIGRATIONS: &[&str] = &[
    // Infraestrutura local do agente (fora do DER, necessária para operação offline).
    r#"
    CREATE TABLE IF NOT EXISTS device_metadata (
        id TEXT PRIMARY KEY NOT NULL,
        device_name TEXT NOT NULL,
        public_key TEXT NOT NULL,
        private_key_b64 TEXT NOT NULL,
        registered_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sync_queue (
        id TEXT PRIMARY KEY NOT NULL,
        entity_type TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        signature TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        attempts INTEGER NOT NULL DEFAULT 0,
        last_attempt_at TEXT,
        next_retry_at TEXT,
        error_message TEXT,
        created_at TEXT NOT NULL,
        confirmed_at TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status, next_retry_at);
    "#,
    // Domínio alinhado a voowork-backend/docs/db.mermaid
    r#"
    CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY NOT NULL,
        account_id TEXT NOT NULL,
        name TEXT NOT NULL,
        featured INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_projects_account ON projects(account_id);

    CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY NOT NULL,
        account_id TEXT NOT NULL,
        project_id TEXT NOT NULL REFERENCES projects(id),
        name TEXT NOT NULL,
        description TEXT,
        position INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project_id, position);

    CREATE TABLE IF NOT EXISTS trackings (
        id TEXT PRIMARY KEY NOT NULL,
        account_id TEXT NOT NULL,
        project_id TEXT NOT NULL,
        task_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'active',
        device TEXT,
        edition_reason TEXT,
        started_at TEXT NOT NULL,
        ended_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_trackings_status ON trackings(status, started_at DESC);
    CREATE INDEX IF NOT EXISTS idx_trackings_project ON trackings(project_id, started_at DESC);

    CREATE TABLE IF NOT EXISTS tracking_screenshots (
        id TEXT PRIMARY KEY NOT NULL,
        path TEXT NOT NULL,
        tracking_id TEXT NOT NULL REFERENCES trackings(id),
        original_id TEXT NOT NULL,
        captured_at TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracking_screenshots_tracking ON tracking_screenshots(tracking_id, captured_at);

    CREATE TABLE IF NOT EXISTS tracking_peripheral_events (
        id TEXT PRIMARY KEY NOT NULL,
        event TEXT NOT NULL,
        count REAL NOT NULL DEFAULT 0.0,
        tracking_id TEXT NOT NULL REFERENCES trackings(id),
        screenshot_original_id TEXT,
        started_at TEXT NOT NULL,
        ended_at TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracking_peripheral_tracking ON tracking_peripheral_events(tracking_id, started_at);

    CREATE TABLE IF NOT EXISTS tracking_apps (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        tracking_id TEXT NOT NULL REFERENCES trackings(id),
        started_at TEXT NOT NULL,
        ended_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracking_apps_tracking ON tracking_apps(tracking_id, started_at);

    CREATE TABLE IF NOT EXISTS tracking_sites (
        id TEXT PRIMARY KEY NOT NULL,
        address TEXT NOT NULL,
        tracking_id TEXT NOT NULL REFERENCES trackings(id),
        started_at TEXT NOT NULL,
        ended_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracking_sites_tracking ON tracking_sites(tracking_id, started_at);
    "#,
    r#"
    DROP TABLE IF EXISTS app_focus_events;
    DROP TABLE IF EXISTS activity_ticks;
    DROP TABLE IF EXISTS screenshots;
    DROP TABLE IF EXISTS sessions;
    DROP TABLE IF EXISTS project_cache;
    "#,
    // Tabelas locais de inatividade e agregação de tempo (criadas anteriormente em db/mod.rs).
    r#"
    CREATE TABLE IF NOT EXISTS tracking_inactivity_periods (
        id TEXT PRIMARY KEY NOT NULL,
        tracking_id TEXT NOT NULL REFERENCES trackings(id),
        inactivity_started_at TEXT NOT NULL,
        paused_at TEXT,
        resumed_at TEXT,
        duration_seconds INTEGER NOT NULL DEFAULT 0,
        discarded_seconds INTEGER NOT NULL DEFAULT 0,
        reclassified_seconds INTEGER NOT NULL DEFAULT 0,
        category TEXT,
        status TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_tracking_inactivity_periods_tracking
        ON tracking_inactivity_periods(tracking_id, inactivity_started_at DESC);

    CREATE TABLE IF NOT EXISTS task_time_totals (
        task_id TEXT PRIMARY KEY NOT NULL,
        active_seconds INTEGER NOT NULL DEFAULT 0,
        updated_at TEXT NOT NULL
    );
    "#,
];

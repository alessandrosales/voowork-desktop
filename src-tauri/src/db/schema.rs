pub const MIGRATIONS: &[&str] = &[
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

    CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY NOT NULL,
        project_id TEXT NOT NULL,
        task_id TEXT,
        started_at TEXT NOT NULL,
        ended_at TEXT,
        monotonic_started_ns INTEGER NOT NULL,
        monotonic_ended_ns INTEGER,
        status TEXT NOT NULL DEFAULT 'active',
        prev_hash TEXT NOT NULL DEFAULT 'genesis',
        record_hash TEXT NOT NULL,
        clock_skew_flags INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS activity_ticks (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL REFERENCES sessions(id),
        bucket_start TEXT NOT NULL,
        bucket_end TEXT NOT NULL,
        mouse_events INTEGER NOT NULL DEFAULT 0,
        keyboard_events INTEGER NOT NULL DEFAULT 0,
        mouse_positions_json TEXT,
        activity_score_confidence REAL NOT NULL DEFAULT 1.0,
        automation_flags INTEGER NOT NULL DEFAULT 0,
        monotonic_elapsed_ns INTEGER NOT NULL,
        wall_clock_at_tick TEXT NOT NULL,
        clock_skew_detected INTEGER NOT NULL DEFAULT 0,
        prev_hash TEXT NOT NULL,
        record_hash TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_activity_ticks_session ON activity_ticks(session_id);

    CREATE TABLE IF NOT EXISTS screenshots (
        id TEXT PRIMARY KEY NOT NULL,
        user_id TEXT,
        project_id TEXT,
        task_id TEXT,
        session_id TEXT NOT NULL REFERENCES sessions(id),
        file_path TEXT NOT NULL,
        sha256_hash TEXT NOT NULL,
        width INTEGER,
        height INTEGER,
        captured_at TEXT NOT NULL,
        activity_tick_id TEXT,
        blur_applied INTEGER NOT NULL DEFAULT 0,
        synced_at TEXT,
        created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_screenshots_session ON screenshots(session_id);

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

    CREATE TABLE IF NOT EXISTS project_cache (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        tasks_json TEXT NOT NULL,
        sort_order INTEGER NOT NULL DEFAULT 0,
        updated_at TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS app_focus_events (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL REFERENCES sessions(id),
        app_name TEXT NOT NULL,
        window_title TEXT,
        process_path TEXT,
        process_id INTEGER,
        captured_at TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_app_focus_session ON app_focus_events(session_id);
    CREATE INDEX IF NOT EXISTS idx_app_focus_captured ON app_focus_events(captured_at);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS idle_periods (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL REFERENCES sessions(id),
        idle_started_at TEXT NOT NULL,
        paused_at TEXT,
        resumed_at TEXT,
        duration_seconds INTEGER,
        discarded_seconds INTEGER NOT NULL DEFAULT 0,
        reclassified_seconds INTEGER NOT NULL DEFAULT 0,
        category TEXT,
        status TEXT NOT NULL DEFAULT 'paused',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_idle_periods_session ON idle_periods(session_id);
    "#,
];

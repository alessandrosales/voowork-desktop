use crate::db::Database;
use crate::error::AgentResult;
use crate::integrity::{finalize_session, insert_activity_tick, insert_session, ActivityTickRecord};
use crate::models::TaskOption;
use chrono::{Duration, Utc};
use rusqlite::params;
use uuid::Uuid;

pub fn ensure_demo_data(db: &Database) -> AgentResult<()> {
    if db.get_setting("demo_seeded")?.is_some() {
        return Ok(());
    }

    if db.get_setting(crate::auth::KEY_AUTHENTICATED)?.is_some_and(|v| v == "true") {
        return Ok(());
    }

    if std::env::var("VOOWORK_API_URL").is_ok() {
        return Ok(());
    }

    seed_projects(db)?;
    seed_demo_sessions(db)?;

    db.set_setting("demo_seeded", "true")?;
    Ok(())
}

fn seed_projects(db: &Database) -> AgentResult<()> {
    db.upsert_project(
        "proj-voowork",
        "Voowork Platform",
        &[
            TaskOption {
                id: "task-auth".into(),
                name: "Autenticação".into(),
            },
            TaskOption {
                id: "task-agent".into(),
                name: "App Desktop".into(),
            },
            TaskOption {
                id: "task-sync".into(),
                name: "Sync e Offline".into(),
            },
        ],
        0,
    )?;

    db.upsert_project(
        "proj-client",
        "Portal do Cliente",
        &[
            TaskOption {
                id: "task-dashboard".into(),
                name: "Dashboard".into(),
            },
            TaskOption {
                id: "task-reports".into(),
                name: "Relatórios".into(),
            },
        ],
        1,
    )?;

    db.upsert_project(
        "proj-internal",
        "Operações Internas",
        &[TaskOption {
            id: "task-support".into(),
            name: "Suporte".into(),
        }],
        2,
    )?;

    Ok(())
}

fn seed_demo_sessions(db: &Database) -> AgentResult<()> {
    let conn = db.conn();
    let now = Utc::now();

    let sessions = [
        (
            "proj-voowork",
            Some("task-agent"),
            now - Duration::hours(26),
            2 * 3600,
            0.92,
        ),
        (
            "proj-client",
            Some("task-dashboard"),
            now - Duration::hours(20),
            3 * 3600,
            0.88,
        ),
        (
            "proj-voowork",
            Some("task-sync"),
            now - Duration::hours(4),
            90 * 60,
            0.95,
        ),
        (
            "proj-internal",
            Some("task-support"),
            now - Duration::hours(2),
            45 * 60,
            0.81,
        ),
    ];

    for (project_id, task_id, started, duration_secs, confidence) in sessions {
        let session_id = Uuid::new_v4().to_string();
        let started_at = started.to_rfc3339();
        let ended_at = (started + Duration::seconds(duration_secs)).to_rfc3339();
        let monotonic_started = 0i64;
        let monotonic_ended = duration_secs * 1_000_000_000;

        insert_session(
            conn,
            &session_id,
            project_id,
            task_id,
            &started_at,
            monotonic_started,
        )?;
        finalize_session(conn, &session_id, &ended_at, monotonic_ended, 0)?;

        let ticks = (duration_secs / 60).max(1);
        for i in 0..ticks {
            let tick_start = started + Duration::minutes(i);
            let tick_end = tick_start + Duration::minutes(1);
            let tick_id = Uuid::new_v4().to_string();
            insert_activity_tick(
                conn,
                &ActivityTickRecord {
                    id: tick_id,
                    session_id: session_id.clone(),
                    bucket_start: tick_start.to_rfc3339(),
                    bucket_end: tick_end.to_rfc3339(),
                    mouse_events: 40 + (i * 7) as i64,
                    keyboard_events: 25 + (i * 5) as i64,
                    mouse_positions_json: None,
                    activity_score_confidence: confidence,
                    automation_flags: 0,
                    monotonic_elapsed_ns: (i + 1) * 60 * 1_000_000_000,
                    wall_clock_at_tick: tick_end.to_rfc3339(),
                    clock_skew_detected: 0,
                },
            )?;
        }

        let payload = serde_json::json!({
            "sessionId": session_id,
            "projectId": project_id,
            "taskId": task_id,
            "startedAt": started_at,
            "endedAt": ended_at,
        });

        let status = if started < now - Duration::hours(6) {
            "confirmed"
        } else {
            "pending"
        };

        let queue_id = Uuid::new_v4().to_string();
        let now_str = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sync_queue (id, entity_type, entity_id, payload_json, signature, status, created_at, next_retry_at, confirmed_at)
             VALUES (?1, 'session', ?2, ?3, NULL, ?4, ?5, ?5, ?6)",
            params![
                queue_id,
                session_id,
                payload.to_string(),
                status,
                now_str,
                if status == "confirmed" {
                    Some(now_str.clone())
                } else {
                    None::<String>
                }
            ],
        )?;
    }

    Ok(())
}

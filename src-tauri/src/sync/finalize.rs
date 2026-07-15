use crate::tracking_focus::{close_active_app, close_active_site};
use crate::db::Database;
use crate::error::AgentResult;
use crate::sync::{
    SyncOutbox, ENTITY_TRACKING, ENTITY_TRACKING_APP, ENTITY_TRACKING_SITE,
};
use rusqlite::Connection;
use serde_json::json;

/// Finaliza trackings órfãos (crash/reboot anterior): fecha filhos abertos, enfileira PATCH e marca inactive localmente.
pub fn finalize_orphaned_trackings(db: &Database) -> AgentResult<u32> {
    let now = chrono::Utc::now().to_rfc3339();
    let orphans = db.list_active_tracking_ids()?;

    for tracking_id in &orphans {
        close_open_children_in_db(db, tracking_id, &now)?;
        enqueue_tracking_stop(db.conn(), tracking_id, &now)?;
        db.finalize_tracking(tracking_id, &now)?;
    }

    Ok(orphans.len() as u32)
}

/// Finaliza um tracking específico com sync remoto (ex.: shutdown sem stop explícito).
pub fn finalize_tracking_remotely(db: &Database, tracking_id: &str) -> AgentResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    close_open_children_in_db(db, tracking_id, &now)?;
    enqueue_tracking_stop(db.conn(), tracking_id, &now)?;
    db.finalize_tracking(tracking_id, &now)
}

/// Fecha apps/sites ainda abertos no SQLite e enfileira POST para cada intervalo.
pub fn close_open_children_in_db(
    db: &Database,
    tracking_id: &str,
    ended_at: &str,
) -> AgentResult<()> {
    let conn = db.conn();

    for app in db.list_open_tracking_apps(tracking_id)? {
        close_active_app(conn, &app.id, ended_at)?;
        enqueue_closed_app(conn, &app, ended_at)?;
    }

    for site in db.list_open_tracking_sites(tracking_id)? {
        close_active_site(conn, &site.id, ended_at)?;
        enqueue_closed_site(conn, &site, ended_at)?;
    }

    Ok(())
}

pub fn enqueue_tracking_stop(
    conn: &Connection,
    tracking_id: &str,
    ended_at: &str,
) -> AgentResult<()> {
    SyncOutbox::enqueue(
        conn,
        ENTITY_TRACKING,
        tracking_id,
        json!({
            "trackingId": tracking_id,
            "endedAt": ended_at,
            "status": "inactive",
        }),
    )?;
    Ok(())
}

fn enqueue_closed_app(
    conn: &Connection,
    app: &crate::models::TrackingAppRow,
    ended_at: &str,
) -> AgentResult<()> {
    SyncOutbox::enqueue(
        conn,
        ENTITY_TRACKING_APP,
        &app.id,
        json!({
            "appId": app.id,
            "trackingId": app.tracking_id,
            "name": app.name,
            "startedAt": app.started_at,
            "endedAt": ended_at,
        }),
    )?;
    Ok(())
}

fn enqueue_closed_site(
    conn: &Connection,
    site: &crate::models::TrackingSiteRow,
    ended_at: &str,
) -> AgentResult<()> {
    SyncOutbox::enqueue(
        conn,
        ENTITY_TRACKING_SITE,
        &site.id,
        json!({
            "siteId": site.id,
            "trackingId": site.tracking_id,
            "address": site.address,
            "startedAt": site.started_at,
            "endedAt": ended_at,
        }),
    )?;
    Ok(())
}

use crate::error::AgentResult;
use crate::integrity::{mark_session_suspicious, validate_activity_chain};
use rusqlite::{params, Connection};

use super::constants::{ENTITY_ACTIVITY_TICK, ENTITY_SESSION};

pub fn validate_entity_before_sync(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
) -> AgentResult<()> {
    if entity_type != ENTITY_ACTIVITY_TICK && entity_type != ENTITY_SESSION {
        return Ok(());
    }

    let session_id = if entity_type == ENTITY_SESSION {
        entity_id.to_string()
    } else {
        conn.query_row(
            "SELECT session_id FROM activity_ticks WHERE id = ?1",
            params![entity_id],
            |row| row.get(0),
        )?
    };

    let validation = validate_activity_chain(conn, &session_id)?;
    if validation.valid {
        return Ok(());
    }

    mark_session_suspicious(conn, &session_id)?;
    log::warn!(
        "hash chain broken for session {session_id} at {:?}",
        validation.broken_at_id
    );
    Ok(())
}

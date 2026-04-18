//! CRUD for `directive_log` — enrichment queue.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::Result;

pub const STATE_PENDING: &str = "pending";
pub const STATE_COMPLETED: &str = "completed";
pub const STATE_ABANDONED: &str = "abandoned";

pub const SOURCE_DIRECTIVE: &str = "directive";
pub const SOURCE_AUTOPILOT: &str = "autopilot";

pub const KIND_ENRICH: &str = "ENRICH";
pub const KIND_FIX_ICM: &str = "FIX_ICM";

pub mod priority {
    pub const IMMEDIATE: i64 = 10;
    pub const NORMAL: i64 = 50;
    pub const LOW: i64 = 90;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveLogRow {
    pub id: Option<i64>,
    pub kind: String,
    pub target_path: String,
    pub session_id: String,
    pub emitted_at: i64,
    pub completed_at: Option<i64>,
    pub retry_count: i64,
    pub state: String,
    pub source: String,
    #[serde(default = "priority_default")]
    pub priority: i64,
}

fn priority_default() -> i64 {
    priority::NORMAL
}

pub fn insert(conn: &Connection, row: &DirectiveLogRow) -> Result<i64> {
    conn.execute(
        "INSERT INTO directive_log
            (kind, target_path, session_id, emitted_at, completed_at,
             retry_count, state, source, priority)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.kind, row.target_path, row.session_id, row.emitted_at, row.completed_at,
            row.retry_count, row.state, row.source, row.priority,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn latest_for_target(
    conn: &Connection,
    target_path: &str,
    kind: &str,
) -> Result<Option<DirectiveLogRow>> {
    conn.query_row(
        "SELECT id, kind, target_path, session_id, emitted_at, completed_at, retry_count, state, source, priority
         FROM directive_log
         WHERE target_path = ?1 AND kind = ?2
         ORDER BY id DESC
         LIMIT 1",
        params![target_path, kind],
        row_from_sqlite,
    )
    .optional()
    .map_err(crate::DbError::from)
}

pub fn list_pending_by_priority(
    conn: &Connection,
    kind: &str,
    limit: i64,
) -> Result<Vec<DirectiveLogRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, target_path, session_id, emitted_at, completed_at, retry_count, state, source, priority
         FROM directive_log
         WHERE kind = ?1 AND state = ?2
         ORDER BY priority ASC, emitted_at ASC
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![kind, STATE_PENDING, limit], row_from_sqlite)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn mark_completed(conn: &Connection, id: i64, completed_at: i64) -> Result<()> {
    conn.execute(
        "UPDATE directive_log SET state = ?1, completed_at = ?2 WHERE id = ?3",
        params![STATE_COMPLETED, completed_at, id],
    )?;
    Ok(())
}

pub fn mark_abandoned(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE directive_log SET state = ?1 WHERE id = ?2",
        params![STATE_ABANDONED, id],
    )?;
    Ok(())
}

pub fn increment_retry_count(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE directive_log SET retry_count = retry_count + 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn count_by_state(conn: &Connection, kind: &str, state: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM directive_log WHERE kind = ?1 AND state = ?2",
        params![kind, state],
        |r| r.get(0),
    )
    .map_err(crate::DbError::from)
}

pub fn list_targets_in_state(conn: &Connection, kind: &str, state: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT target_path FROM directive_log d1
         WHERE kind = ?1 AND state = ?2
           AND id = (SELECT MAX(id) FROM directive_log d2
                     WHERE d2.target_path = d1.target_path AND d2.kind = d1.kind)",
    )?;
    let rows = stmt
        .query_map(params![kind, state], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn row_from_sqlite(r: &rusqlite::Row<'_>) -> rusqlite::Result<DirectiveLogRow> {
    Ok(DirectiveLogRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        target_path: r.get(2)?,
        session_id: r.get(3)?,
        emitted_at: r.get(4)?,
        completed_at: r.get(5)?,
        retry_count: r.get(6)?,
        state: r.get(7)?,
        source: r.get(8)?,
        priority: r.get(9).unwrap_or(priority::NORMAL),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrismDb;

    fn db() -> PrismDb {
        let db = PrismDb::open_in_memory().unwrap();
        db.initialize().unwrap();
        db
    }

    #[test]
    fn insert_and_latest_for_target() {
        let db = db();
        let id = insert(
            db.conn(),
            &DirectiveLogRow {
                id: None,
                kind: KIND_ENRICH.into(),
                target_path: "src/foo".into(),
                session_id: "s1".into(),
                emitted_at: 100,
                completed_at: None,
                retry_count: 0,
                state: STATE_PENDING.into(),
                source: SOURCE_DIRECTIVE.into(),
                priority: priority::NORMAL,
            },
        )
        .unwrap();
        assert!(id > 0);
        let row = latest_for_target(db.conn(), "src/foo", KIND_ENRICH)
            .unwrap()
            .unwrap();
        assert_eq!(row.state, STATE_PENDING);
    }

    #[test]
    fn mark_completed_updates_state() {
        let db = db();
        let id = insert(
            db.conn(),
            &DirectiveLogRow {
                id: None,
                kind: KIND_ENRICH.into(),
                target_path: "src/foo".into(),
                session_id: "s1".into(),
                emitted_at: 100,
                completed_at: None,
                retry_count: 0,
                state: STATE_PENDING.into(),
                source: SOURCE_DIRECTIVE.into(),
                priority: priority::NORMAL,
            },
        )
        .unwrap();
        mark_completed(db.conn(), id, 500).unwrap();
        let row = latest_for_target(db.conn(), "src/foo", KIND_ENRICH)
            .unwrap()
            .unwrap();
        assert_eq!(row.state, STATE_COMPLETED);
        assert_eq!(row.completed_at, Some(500));
    }
}

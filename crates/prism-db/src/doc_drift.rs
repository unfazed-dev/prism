//! CRUD for `doc_drift`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::{DbError, Result};

pub const DRIFT_TYPE_ICM: &str = "IcmViolation";
pub const DRIFT_TYPE_OUTDATED: &str = "OutdatedContextFile";

/// Count unresolved drift rows whose `drift_type` matches.
pub fn count_unresolved_by_type(conn: &Connection, drift_type: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM doc_drift WHERE drift_type = ?1 AND resolved = 0",
        params![drift_type],
        |r| r.get(0),
    )
    .map_err(DbError::from)
}

/// True when an identical unresolved drift row already exists. Used by hooks
/// to dedupe repeated violations on the same file instead of accumulating rows.
pub fn exists_unresolved(
    conn: &Connection,
    affected_doc: &str,
    drift_type: &str,
    description: &str,
) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM doc_drift
         WHERE affected_doc = ?1 AND drift_type = ?2 AND description = ?3 AND resolved = 0",
        params![affected_doc, drift_type, description],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocDriftRow {
    pub drift_id: Option<i64>,
    pub session_id: String,
    pub detected_turn: i64,
    pub affected_doc: String,
    pub drift_type: String,
    pub severity: String,
    pub description: String,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<String>,
}

pub fn insert(conn: &Connection, row: &DocDriftRow) -> Result<i64> {
    conn.execute(
        "INSERT INTO doc_drift (session_id, detected_turn, affected_doc, drift_type, severity, description, resolved, resolved_by, resolved_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.session_id, row.detected_turn, row.affected_doc, row.drift_type,
            row.severity, row.description, row.resolved, row.resolved_by, row.resolved_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod exists_unresolved_tests {
    use super::*;
    use crate::PrismDb;

    fn db() -> PrismDb {
        let db = PrismDb::open_in_memory().unwrap();
        db.initialize().unwrap();
        db
    }

    fn row(affected_doc: &str, drift_type: &str, description: &str) -> DocDriftRow {
        DocDriftRow {
            drift_id: None,
            session_id: "s".into(),
            detected_turn: 0,
            affected_doc: affected_doc.into(),
            drift_type: drift_type.into(),
            severity: "warning".into(),
            description: description.into(),
            resolved: false,
            resolved_by: None,
            resolved_at: None,
        }
    }

    #[test]
    fn missing_row_returns_false() {
        let d = db();
        assert!(!exists_unresolved(d.conn(), "a.md", DRIFT_TYPE_ICM, "x").unwrap());
    }

    #[test]
    fn inserted_unresolved_row_returns_true() {
        let d = db();
        insert(d.conn(), &row("a.md", DRIFT_TYPE_ICM, "x")).unwrap();
        assert!(exists_unresolved(d.conn(), "a.md", DRIFT_TYPE_ICM, "x").unwrap());
    }

    #[test]
    fn resolved_row_does_not_match() {
        let d = db();
        let mut r = row("a.md", DRIFT_TYPE_ICM, "x");
        r.resolved = true;
        insert(d.conn(), &r).unwrap();
        assert!(!exists_unresolved(d.conn(), "a.md", DRIFT_TYPE_ICM, "x").unwrap());
    }

    #[test]
    fn different_description_does_not_match() {
        let d = db();
        insert(d.conn(), &row("a.md", DRIFT_TYPE_ICM, "x")).unwrap();
        assert!(!exists_unresolved(d.conn(), "a.md", DRIFT_TYPE_ICM, "y").unwrap());
    }
}


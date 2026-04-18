//! CRUD for `file_hashes` with pending-hash crash guard.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashRow {
    pub file_path: String,
    pub hash: String,
    pub last_checked: String,
    pub file_size: i64,
    pub language: Option<String>,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub previous_hash: Option<String>,
}

pub fn upsert(conn: &Connection, row: &FileHashRow) -> Result<()> {
    conn.execute(
        "INSERT INTO file_hashes (file_path, hash, last_checked, file_size, language, pending, previous_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(file_path) DO UPDATE SET
           hash = excluded.hash,
           last_checked = excluded.last_checked,
           file_size = excluded.file_size,
           language = excluded.language,
           pending = excluded.pending,
           previous_hash = excluded.previous_hash",
        params![
            row.file_path, row.hash, row.last_checked, row.file_size,
            row.language, row.pending, row.previous_hash,
        ],
    )?;
    Ok(())
}

pub fn get_by_path(conn: &Connection, file_path: &str) -> Result<Option<FileHashRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, hash, last_checked, file_size, language, pending, previous_hash
         FROM file_hashes WHERE file_path = ?1",
    )?;
    let mut rows = stmt.query_map(params![file_path], row_from_sqlite)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn list_all(conn: &Connection) -> Result<Vec<FileHashRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, hash, last_checked, file_size, language, pending, previous_hash FROM file_hashes",
    )?;
    let rows = stmt
        .query_map([], row_from_sqlite)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_pending(conn: &Connection) -> Result<Vec<FileHashRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, hash, last_checked, file_size, language, pending, previous_hash
         FROM file_hashes WHERE pending = 1",
    )?;
    let rows = stmt
        .query_map([], row_from_sqlite)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn mark_pending(conn: &Connection, file_path: &str, new_hash: &str) -> Result<()> {
    let previous: Option<String> = conn
        .query_row(
            "SELECT hash FROM file_hashes WHERE file_path = ?1",
            params![file_path],
            |r| r.get(0),
        )
        .ok();
    conn.execute(
        "INSERT INTO file_hashes (file_path, hash, last_checked, file_size, language, pending, previous_hash)
         VALUES (?1, ?2, ?3, 0, NULL, 1, ?4)
         ON CONFLICT(file_path) DO UPDATE SET
           hash = excluded.hash,
           last_checked = excluded.last_checked,
           pending = 1,
           previous_hash = ?4",
        params![file_path, new_hash, chrono::Utc::now().to_rfc3339(), previous],
    )?;
    Ok(())
}

pub fn clear_pending(conn: &Connection, file_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE file_hashes SET pending = 0, previous_hash = NULL WHERE file_path = ?1",
        params![file_path],
    )?;
    Ok(())
}

pub fn rollback_hash(conn: &Connection, file_path: &str) -> Result<()> {
    let prev: Option<String> = conn
        .query_row(
            "SELECT previous_hash FROM file_hashes WHERE file_path = ?1",
            params![file_path],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    match prev {
        Some(hash) => {
            conn.execute(
                "UPDATE file_hashes SET hash = ?1, pending = 0, previous_hash = NULL WHERE file_path = ?2",
                params![hash, file_path],
            )?;
        }
        None => {
            conn.execute(
                "DELETE FROM file_hashes WHERE file_path = ?1",
                params![file_path],
            )?;
        }
    }
    Ok(())
}

pub fn delete(conn: &Connection, file_path: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM file_hashes WHERE file_path = ?1",
        params![file_path],
    )?;
    Ok(())
}

fn row_from_sqlite(r: &rusqlite::Row<'_>) -> rusqlite::Result<FileHashRow> {
    Ok(FileHashRow {
        file_path: r.get(0)?,
        hash: r.get(1)?,
        last_checked: r.get(2)?,
        file_size: r.get(3)?,
        language: r.get(4)?,
        pending: r.get(5)?,
        previous_hash: r.get(6)?,
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
    fn mark_pending_stashes_previous_hash() {
        let db = db();
        upsert(
            db.conn(),
            &FileHashRow {
                file_path: "src/lib.rs".into(),
                hash: "old".into(),
                last_checked: "2026-04-16T00:00:00Z".into(),
                file_size: 10,
                language: None,
                pending: false,
                previous_hash: None,
            },
        )
        .unwrap();
        mark_pending(db.conn(), "src/lib.rs", "new").unwrap();
        let row = get_by_path(db.conn(), "src/lib.rs").unwrap().unwrap();
        assert_eq!(row.hash, "new");
        assert!(row.pending);
        assert_eq!(row.previous_hash.as_deref(), Some("old"));
    }

    #[test]
    fn rollback_restores_previous_hash() {
        let db = db();
        upsert(
            db.conn(),
            &FileHashRow {
                file_path: "a.md".into(),
                hash: "old".into(),
                last_checked: "2026-04-16T00:00:00Z".into(),
                file_size: 0,
                language: None,
                pending: false,
                previous_hash: None,
            },
        )
        .unwrap();
        mark_pending(db.conn(), "a.md", "new").unwrap();
        rollback_hash(db.conn(), "a.md").unwrap();
        let row = get_by_path(db.conn(), "a.md").unwrap().unwrap();
        assert_eq!(row.hash, "old");
        assert!(!row.pending);
    }

    #[test]
    fn rollback_deletes_fresh_pending_row() {
        let db = db();
        mark_pending(db.conn(), "fresh.md", "h").unwrap();
        rollback_hash(db.conn(), "fresh.md").unwrap();
        assert!(get_by_path(db.conn(), "fresh.md").unwrap().is_none());
    }
}

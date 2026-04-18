//! CRUD for `file_hashes` — content hashes for drift detection.

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
}

pub fn upsert(conn: &Connection, row: &FileHashRow) -> Result<()> {
    conn.execute(
        "INSERT INTO file_hashes (file_path, hash, last_checked, file_size, language)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(file_path) DO UPDATE SET
           hash = excluded.hash,
           last_checked = excluded.last_checked,
           file_size = excluded.file_size,
           language = excluded.language",
        params![
            row.file_path,
            row.hash,
            row.last_checked,
            row.file_size,
            row.language,
        ],
    )?;
    Ok(())
}

pub fn get_by_path(conn: &Connection, file_path: &str) -> Result<Option<FileHashRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, hash, last_checked, file_size, language
         FROM file_hashes WHERE file_path = ?1",
    )?;
    let mut rows = stmt.query_map(params![file_path], |r| {
        Ok(FileHashRow {
            file_path: r.get(0)?,
            hash: r.get(1)?,
            last_checked: r.get(2)?,
            file_size: r.get(3)?,
            language: r.get(4)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
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
    fn upsert_then_get_roundtrip() {
        let d = db();
        upsert(
            d.conn(),
            &FileHashRow {
                file_path: "a.rs".into(),
                hash: "h".into(),
                last_checked: "2026-04-18T00:00:00Z".into(),
                file_size: 10,
                language: Some("rust".into()),
            },
        )
        .unwrap();
        let row = get_by_path(d.conn(), "a.rs").unwrap().unwrap();
        assert_eq!(row.hash, "h");
        assert_eq!(row.language.as_deref(), Some("rust"));
    }

    #[test]
    fn upsert_updates_existing_row() {
        let d = db();
        let mk = |h: &str| FileHashRow {
            file_path: "a.rs".into(),
            hash: h.into(),
            last_checked: "t".into(),
            file_size: 0,
            language: None,
        };
        upsert(d.conn(), &mk("old")).unwrap();
        upsert(d.conn(), &mk("new")).unwrap();
        let row = get_by_path(d.conn(), "a.rs").unwrap().unwrap();
        assert_eq!(row.hash, "new");
    }

    #[test]
    fn get_missing_returns_none() {
        let d = db();
        assert!(get_by_path(d.conn(), "nope").unwrap().is_none());
    }
}

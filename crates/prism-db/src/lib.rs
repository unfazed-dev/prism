//! prism-db — SQLite persistence for doc registry, file hashes, drift events,
//! enrichment queue, and enrichment run accounting.

pub mod directive_log;
pub mod doc_drift;
pub mod document_registry;
pub mod enrichment_runs;
pub mod file_hashes;
pub mod schema;

use std::io::{self, Write};
use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("not found: {entity} with id {id}")]
    NotFound {
        entity: &'static str,
        id: String,
    },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Atomically replace `path`'s contents with `content`.
pub fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = match path.file_name() {
        Some(name) => {
            let mut tmp = std::ffi::OsString::from(".");
            tmp.push(name);
            tmp.push(format!(".prism-tmp-{}", std::process::id()));
            path.with_file_name(tmp)
        }
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "atomic_write target has no file name",
            ))
        }
    };

    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        f.write_all(content)?;
        f.flush()?;
        f.sync_all()?;
    }

    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

/// Central database handle.
pub struct PrismDb {
    conn: Connection,
}

impl PrismDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<()> {
        schema::create_all_tables(&self.conn)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn transaction(&self) -> Result<rusqlite::Transaction<'_>> {
        self.conn.unchecked_transaction().map_err(DbError::from)
    }

    // File hashes convenience
    pub fn get_file_hash(&self, path: &str) -> Result<Option<file_hashes::FileHashRow>> {
        file_hashes::get_by_path(&self.conn, path)
    }

    pub fn upsert_file_hash(&self, path: &str, hash: &str) -> Result<()> {
        file_hashes::upsert(
            &self.conn,
            &file_hashes::FileHashRow {
                file_path: path.to_string(),
                hash: hash.to_string(),
                last_checked: chrono::Utc::now().to_rfc3339(),
                file_size: 0,
                language: None,
                pending: false,
                previous_hash: None,
            },
        )
    }

    pub fn write_managed_with_hash(
        &self,
        path_key: &str,
        abs_path: &std::path::Path,
        content: &[u8],
        new_hash: &str,
    ) -> Result<()> {
        file_hashes::mark_pending(&self.conn, path_key, new_hash)?;
        atomic_write(abs_path, content)
            .map_err(|e| DbError::Other(format!("atomic write failed for {path_key}: {e}")))?;
        file_hashes::clear_pending(&self.conn, path_key)
    }

    pub fn list_unresolved_drift(&self) -> Result<Vec<doc_drift::DocDriftRow>> {
        doc_drift::list_all_unresolved(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_and_initialize() {
        let db = PrismDb::open_in_memory().expect("open in-memory");
        db.initialize().expect("initialize schema");
    }
}

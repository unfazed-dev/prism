//! prism-db — SQLite persistence for doc registry, file hashes, drift events,
//! and enrichment directive queue.

pub mod directive_log;
pub mod doc_drift;
pub mod document_registry;
pub mod file_hashes;
pub mod schema;

use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

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
            },
        )
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

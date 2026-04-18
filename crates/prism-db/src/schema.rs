//! Schema DDL for prism-db. Single-version; no migration chain in v2.

use rusqlite::Connection;

use crate::Result;

pub fn create_all_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(ALL_TABLES_DDL)?;
    Ok(())
}

const ALL_TABLES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS document_registry (
    doc_id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    doc_type TEXT NOT NULL,
    layer TEXT,
    classification TEXT NOT NULL,
    status TEXT DEFAULT 'active',
    version TEXT DEFAULT '1.0.0',
    created_at TEXT NOT NULL,
    last_synced TEXT NOT NULL,
    last_synced_by TEXT NOT NULL,
    review_date TEXT,
    token_budget INTEGER,
    token_estimate INTEGER,
    source_hash TEXT,
    parent_dir TEXT,
    origin TEXT DEFAULT 'prism'
);

CREATE TABLE IF NOT EXISTS file_hashes (
    file_path TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    last_checked TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    language TEXT
);

CREATE TABLE IF NOT EXISTS doc_drift (
    drift_id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    detected_turn INTEGER NOT NULL,
    affected_doc TEXT NOT NULL,
    drift_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    description TEXT NOT NULL,
    resolved BOOLEAN DEFAULT 0,
    resolved_by TEXT,
    resolved_at TEXT
);

CREATE TABLE IF NOT EXISTS directive_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    target_path TEXT NOT NULL,
    session_id TEXT NOT NULL,
    emitted_at INTEGER NOT NULL,
    completed_at INTEGER,
    retry_count INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'pending',
    source TEXT NOT NULL DEFAULT 'directive',
    priority INTEGER NOT NULL DEFAULT 50
);
CREATE INDEX IF NOT EXISTS idx_directive_log_target ON directive_log(target_path, kind);
CREATE INDEX IF NOT EXISTS idx_directive_log_state ON directive_log(state, kind);
CREATE INDEX IF NOT EXISTS idx_directive_log_priority ON directive_log(priority, emitted_at);

"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn create_all_tables_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_all_tables(&conn).unwrap();
        create_all_tables(&conn).unwrap();
    }
}

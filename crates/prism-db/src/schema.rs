//! Schema DDL for prism-db. Single-version; no migration chain in v2.

use rusqlite::Connection;

use crate::Result;

pub fn create_all_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(ALL_TABLES_DDL)?;
    Ok(())
}

const ALL_TABLES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);

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

CREATE TABLE IF NOT EXISTS document_dependencies (
    from_doc TEXT NOT NULL,
    to_doc TEXT NOT NULL,
    relation TEXT NOT NULL,
    PRIMARY KEY (from_doc, to_doc),
    FOREIGN KEY (from_doc) REFERENCES document_registry(doc_id),
    FOREIGN KEY (to_doc) REFERENCES document_registry(doc_id)
);

CREATE TABLE IF NOT EXISTS file_hashes (
    file_path TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    last_checked TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    language TEXT,
    pending BOOLEAN NOT NULL DEFAULT 0,
    previous_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_file_hashes_pending ON file_hashes(pending) WHERE pending = 1;

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

CREATE TABLE IF NOT EXISTS enrichment_runs (
    run_id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    directory TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_estimate_usd REAL NOT NULL DEFAULT 0.0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    outcome TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_enrichment_runs_session ON enrichment_runs(session_id, started_at);
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

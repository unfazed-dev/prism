//! CRUD for `document_registry`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRegistryRow {
    pub doc_id: String,
    pub title: String,
    pub description: Option<String>,
    pub doc_type: String,
    pub layer: Option<String>,
    pub classification: String,
    pub status: String,
    pub version: String,
    pub created_at: String,
    pub last_synced: String,
    pub last_synced_by: String,
    pub review_date: Option<String>,
    pub token_budget: Option<i64>,
    pub token_estimate: Option<i64>,
    pub source_hash: Option<String>,
    pub parent_dir: Option<String>,
    pub origin: String,
}

pub fn upsert(conn: &Connection, row: &DocumentRegistryRow) -> Result<()> {
    conn.execute(
        "INSERT INTO document_registry (doc_id, title, description, doc_type, layer, classification, status, version, created_at, last_synced, last_synced_by, review_date, token_budget, token_estimate, source_hash, parent_dir, origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(doc_id) DO UPDATE SET
             title = excluded.title,
             description = excluded.description,
             doc_type = excluded.doc_type,
             layer = excluded.layer,
             classification = excluded.classification,
             version = excluded.version,
             last_synced = excluded.last_synced,
             last_synced_by = excluded.last_synced_by,
             source_hash = excluded.source_hash,
             parent_dir = excluded.parent_dir,
             origin = excluded.origin",
        params![
            row.doc_id, row.title, row.description, row.doc_type, row.layer,
            row.classification, row.status, row.version, row.created_at,
            row.last_synced, row.last_synced_by, row.review_date, row.token_budget,
            row.token_estimate, row.source_hash, row.parent_dir, row.origin,
        ],
    )?;
    Ok(())
}

pub fn list_all(conn: &Connection) -> Result<Vec<DocumentRegistryRow>> {
    let mut stmt = conn.prepare(
        "SELECT doc_id, title, description, doc_type, layer, classification, status, version, created_at, last_synced, last_synced_by, review_date, token_budget, token_estimate, source_hash, parent_dir, origin
         FROM document_registry",
    )?;
    let rows = stmt
        .query_map([], map_doc_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn map_doc_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRegistryRow> {
    Ok(DocumentRegistryRow {
        doc_id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        doc_type: row.get(3)?,
        layer: row.get(4)?,
        classification: row.get(5)?,
        status: row.get(6)?,
        version: row.get(7)?,
        created_at: row.get(8)?,
        last_synced: row.get(9)?,
        last_synced_by: row.get(10)?,
        review_date: row.get(11)?,
        token_budget: row.get(12)?,
        token_estimate: row.get(13)?,
        source_hash: row.get(14)?,
        parent_dir: row.get(15)?,
        origin: row.get(16)?,
    })
}

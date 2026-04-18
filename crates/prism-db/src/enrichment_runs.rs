//! CRUD for `enrichment_runs` — per-directory `prism enrich` accounting.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentRunRow {
    pub run_id: Option<i64>,
    pub session_id: String,
    pub directory: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_estimate_usd: f64,
    pub duration_ms: i64,
    pub outcome: String,
    pub started_at: String,
    pub ended_at: String,
}

pub fn insert(conn: &Connection, row: &EnrichmentRunRow) -> Result<i64> {
    conn.execute(
        "INSERT INTO enrichment_runs (
            session_id, directory, model, input_tokens, output_tokens,
            cost_estimate_usd, duration_ms, outcome, started_at, ended_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.session_id, row.directory, row.model, row.input_tokens, row.output_tokens,
            row.cost_estimate_usd, row.duration_ms, row.outcome, row.started_at, row.ended_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn session_totals(conn: &Connection, session_id: &str) -> Result<SessionTotals> {
    let (directories, input_tokens, output_tokens, cost): (i64, i64, i64, f64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cost_estimate_usd), 0.0)
         FROM enrichment_runs WHERE session_id = ?1",
        params![session_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok(SessionTotals {
        directories: directories as usize,
        input_tokens,
        output_tokens,
        cost_usd: cost,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct SessionTotals {
    pub directories: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

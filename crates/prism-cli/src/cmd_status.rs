use std::env;

use anyhow::Context;
use prism_db::{directive_log, doc_drift, document_registry, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        println!("PRISM not initialized here. Run `prism start`.");
        return Ok(());
    }

    let db = PrismDb::open(&db_path).context("open database")?;

    let docs = document_registry::list_all(db.conn())?;
    let source_drift =
        doc_drift::count_unresolved_by_type(db.conn(), doc_drift::DRIFT_TYPE_OUTDATED)?;
    let icm_violations =
        doc_drift::count_unresolved_by_type(db.conn(), doc_drift::DRIFT_TYPE_ICM)?;
    let pending_enrich = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_ENRICH,
        directive_log::STATE_PENDING,
    )?;
    let pending_fix = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_FIX_ICM,
        directive_log::STATE_PENDING,
    )?;

    println!("PRISM status — {}", project_root.display());
    println!("  managed docs:      {}", docs.len());
    println!("  source drift:      {}", source_drift);
    println!("  icm violations:    {}", icm_violations);
    println!("  pending enrich:    {}", pending_enrich);
    println!("  pending fix:       {}", pending_fix);
    Ok(())
}

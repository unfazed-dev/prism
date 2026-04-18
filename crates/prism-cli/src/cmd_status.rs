use std::env;

use anyhow::Context;
use prism_db::{directive_log, document_registry, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        println!("PRISM not initialized here. Run `prism start`.");
        return Ok(());
    }

    let db = PrismDb::open(&db_path).context("open database")?;

    let docs = document_registry::list_all(db.conn())?;
    let drift = db.list_unresolved_drift()?;
    let pending_enrich = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_ENRICH,
        directive_log::STATE_PENDING,
    )?;

    println!("PRISM status — {}", project_root.display());
    println!("  managed docs:      {}", docs.len());
    println!("  unresolved drift:  {}", drift.len());
    println!("  pending enrich:    {}", pending_enrich);
    Ok(())
}

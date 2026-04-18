use std::env;

use anyhow::Context;
use prism_db::{directive_log, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let db = PrismDb::open(&db_path).context("open database")?;
    let pending = directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_ENRICH, 50)?;

    if pending.is_empty() {
        println!("No pending enrichment directives.");
        return Ok(());
    }

    println!(
        "Pending enrichment: {} directive(s). (Haiku subprocess wiring TBD.)",
        pending.len()
    );
    for row in &pending {
        println!("  {} (retry {})", row.target_path, row.retry_count);
    }
    Ok(())
}

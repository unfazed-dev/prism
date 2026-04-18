use std::env;

use anyhow::Context;
use prism_core::config::AutopilotConfig;
use prism_core::enrich::{enrich_directory, EnrichOutcome};
use prism_db::{directive_log, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let db = PrismDb::open(&db_path).context("open database")?;
    let pending =
        directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_ENRICH, 50)?;

    if pending.is_empty() {
        println!("No pending enrichment directives.");
        return Ok(());
    }

    let cfg = AutopilotConfig::default();
    let mut completed = 0usize;
    let mut failed = 0usize;

    for row in &pending {
        let dir = project_root.join(&row.target_path);
        if !dir.exists() {
            directive_log::mark_abandoned(db.conn(), row.id.unwrap_or_default())?;
            continue;
        }

        match enrich_directory(&dir, &project_root, &cfg, false) {
            Ok(EnrichOutcome::Completed { .. }) => {
                directive_log::mark_completed(
                    db.conn(),
                    row.id.unwrap_or_default(),
                    chrono::Utc::now().timestamp(),
                )?;
                completed += 1;
                println!("  completed: {}", row.target_path);
            }
            Ok(outcome) => {
                directive_log::increment_retry_count(db.conn(), row.id.unwrap_or_default())?;
                failed += 1;
                println!("  deferred:  {} ({:?})", row.target_path, outcome);
            }
            Err(err) => {
                directive_log::increment_retry_count(db.conn(), row.id.unwrap_or_default())?;
                failed += 1;
                println!("  error:     {} — {}", row.target_path, err);
            }
        }
    }

    println!("Drained {} directive(s): {} completed, {} deferred.", pending.len(), completed, failed);
    Ok(())
}

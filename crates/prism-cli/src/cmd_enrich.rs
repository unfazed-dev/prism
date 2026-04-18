use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use prism_core::config::{AutopilotConfig, PrismConfig};
use prism_core::enrich::{enrich_directory, EnrichOutcome};
use prism_db::{directive_log, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let (autopilot_cfg, max_retries) = load_cfg(&project_root);

    if !autopilot_cfg.enabled {
        anyhow::bail!(
            "Autopilot disabled in .prism/config.json (enrichment.autopilot.enabled=false)."
        );
    }

    if !claude_cli_on_path() {
        anyhow::bail!(
            "`claude` CLI not found on PATH. Install Claude Code or set enrichment.autopilot.enabled=false."
        );
    }

    let db = PrismDb::open(&db_path).context("open database")?;
    let pending =
        directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_ENRICH, 50)?;

    if pending.is_empty() {
        println!("No pending enrichment directives.");
        return Ok(());
    }

    let mut completed = 0usize;
    let mut deferred = 0usize;
    let mut abandoned = 0usize;

    for row in &pending {
        let id = row.id.unwrap_or_default();
        let dir = project_root.join(&row.target_path);
        if !dir.exists() {
            directive_log::mark_abandoned(db.conn(), id)?;
            abandoned += 1;
            println!("  abandoned: {} (missing dir)", row.target_path);
            continue;
        }

        match enrich_directory(&dir, &project_root, &autopilot_cfg, false) {
            Ok(EnrichOutcome::Completed { .. }) => {
                directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
                completed += 1;
                println!("  completed: {}", row.target_path);
            }
            Ok(EnrichOutcome::Failed { stderr, .. }) => {
                directive_log::increment_retry_count(db.conn(), id)?;
                if (row.retry_count as u32) + 1 >= max_retries {
                    directive_log::mark_abandoned(db.conn(), id)?;
                    abandoned += 1;
                    println!(
                        "  abandoned: {} after {} retries — {}",
                        row.target_path,
                        row.retry_count + 1,
                        stderr
                    );
                } else {
                    deferred += 1;
                    println!("  failed:    {} — {}", row.target_path, stderr);
                }
            }
            Ok(outcome) => {
                directive_log::increment_retry_count(db.conn(), id)?;
                if (row.retry_count as u32) + 1 >= max_retries {
                    directive_log::mark_abandoned(db.conn(), id)?;
                    abandoned += 1;
                    println!(
                        "  abandoned: {} after {} retries ({:?})",
                        row.target_path,
                        row.retry_count + 1,
                        outcome
                    );
                } else {
                    deferred += 1;
                    println!("  deferred:  {} ({:?})", row.target_path, outcome);
                }
            }
            Err(err) => {
                directive_log::increment_retry_count(db.conn(), id)?;
                deferred += 1;
                println!("  error:     {} — {}", row.target_path, err);
            }
        }
    }

    println!(
        "Drained {} directive(s): {} completed, {} deferred, {} abandoned.",
        pending.len(),
        completed,
        deferred,
        abandoned
    );
    Ok(())
}

fn load_cfg(project_root: &Path) -> (AutopilotConfig, u32) {
    let cfg_path = project_root.join(".prism/config.json");
    if cfg_path.exists() {
        if let Ok(cfg) = PrismConfig::load(&cfg_path) {
            return (cfg.enrichment.autopilot, cfg.enrichment.max_retries);
        }
    }
    (AutopilotConfig::default(), 3)
}

fn claude_cli_on_path() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

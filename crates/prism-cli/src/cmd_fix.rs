use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use prism_core::config::{AutopilotConfig, PrismConfig};
use prism_core::enrich::{fix_icm_file, IcmFixOutcome};
use prism_core::icm::{validate_icm, IcmSettings, Scope};
use prism_db::{directive_log, PrismDb};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let (cfg, max_retries) = load_cfg(&project_root);

    if !cfg.enabled {
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
        directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_FIX_ICM, 50)?;

    if pending.is_empty() {
        println!("No pending ICM fix directives.");
        return Ok(());
    }

    let mut resolved = 0usize;
    let mut deferred = 0usize;
    let mut abandoned = 0usize;

    for row in &pending {
        let id = row.id.unwrap_or_default();
        let rel = PathBuf::from(&row.target_path);
        let abs = if rel.is_absolute() {
            rel.clone()
        } else {
            project_root.join(&rel)
        };
        if !abs.exists() {
            directive_log::mark_abandoned(db.conn(), id)?;
            abandoned += 1;
            println!("  abandoned: {} (missing file)", row.target_path);
            continue;
        }

        let violations =
            validate_icm(&project_root, &Scope::File(rel.clone()), IcmSettings::default());
        if violations.is_empty() {
            directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
            resolved += 1;
            println!("  resolved:  {} (clean on re-check)", row.target_path);
            continue;
        }

        match fix_icm_file(&project_root, &rel, &violations, &cfg) {
            Ok(IcmFixOutcome::Resolved { .. }) => {
                directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
                resolved += 1;
                println!("  resolved:  {}", row.target_path);
            }
            Ok(IcmFixOutcome::StillViolated { remaining, .. }) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut deferred,
                    &mut abandoned,
                    &format!("{} violation(s) remain", remaining.len()),
                )?;
            }
            Ok(IcmFixOutcome::Failed { stderr, .. }) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut deferred,
                    &mut abandoned,
                    &stderr,
                )?;
            }
            Ok(IcmFixOutcome::TimedOut { .. }) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut deferred,
                    &mut abandoned,
                    "timed out",
                )?;
            }
            Err(err) => {
                directive_log::increment_retry_count(db.conn(), id)?;
                deferred += 1;
                println!("  error:     {} — {}", row.target_path, err);
            }
        }
    }

    println!(
        "Drained {} FIX_ICM directive(s): {} resolved, {} deferred, {} abandoned.",
        pending.len(),
        resolved,
        deferred,
        abandoned
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn advance_or_abandon(
    db: &PrismDb,
    id: i64,
    retry_count: i64,
    max_retries: u32,
    target_path: &str,
    deferred: &mut usize,
    abandoned: &mut usize,
    reason: &str,
) -> anyhow::Result<()> {
    directive_log::increment_retry_count(db.conn(), id)?;
    if (retry_count as u32) + 1 >= max_retries {
        directive_log::mark_abandoned(db.conn(), id)?;
        *abandoned += 1;
        println!(
            "  abandoned: {} after {} retries — {}",
            target_path,
            retry_count + 1,
            reason
        );
    } else {
        *deferred += 1;
        println!("  deferred:  {} — {}", target_path, reason);
    }
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

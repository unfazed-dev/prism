use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use prism_core::command_runner::{CommandRunner, SystemRunner};
use prism_core::config::{AutopilotConfig, PrismConfig};
use prism_core::enrich::{enrich_directory_with, EnrichOutcome};
use prism_db::{directive_log, PrismDb};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EnrichStats {
    pub total: usize,
    pub completed: usize,
    pub deferred: usize,
    pub abandoned: usize,
}

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let (autopilot_cfg, _) = load_cfg(&project_root);
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

    let runner = SystemRunner;
    let stats = run_with(&project_root, &runner)?;
    println!(
        "Drained {} directive(s): {} completed, {} deferred, {} abandoned.",
        stats.total, stats.completed, stats.deferred, stats.abandoned
    );
    Ok(())
}

pub fn run_with(project_root: &Path, runner: &dyn CommandRunner) -> anyhow::Result<EnrichStats> {
    let db_path = project_root.join(".prism/prism.db");
    let (autopilot_cfg, max_retries) = load_cfg(project_root);
    let db = PrismDb::open(&db_path).context("open database")?;
    let pending =
        directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_ENRICH, 50)?;

    let mut stats = EnrichStats {
        total: pending.len(),
        ..Default::default()
    };

    if pending.is_empty() {
        println!("No pending enrichment directives.");
        return Ok(stats);
    }

    for row in &pending {
        let id = row.id.unwrap_or_default();
        let dir = project_root.join(&row.target_path);
        if !dir.exists() {
            directive_log::mark_abandoned(db.conn(), id)?;
            stats.abandoned += 1;
            println!("  abandoned: {} (missing dir)", row.target_path);
            continue;
        }

        match enrich_directory_with(&dir, project_root, &autopilot_cfg, false, runner) {
            Ok(EnrichOutcome::Completed { .. }) => {
                directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
                stats.completed += 1;
                println!("  completed: {}", row.target_path);
            }
            Ok(EnrichOutcome::Failed { stderr, .. }) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut stats,
                    &stderr,
                )?;
            }
            Ok(outcome) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut stats,
                    &format!("{outcome:?}"),
                )?;
            }
            Err(err) => {
                directive_log::increment_retry_count(db.conn(), id)?;
                stats.deferred += 1;
                println!("  error:     {} — {}", row.target_path, err);
            }
        }
    }

    Ok(stats)
}

fn advance_or_abandon(
    db: &PrismDb,
    id: i64,
    retry_count: i64,
    max_retries: u32,
    target_path: &str,
    stats: &mut EnrichStats,
    reason: &str,
) -> anyhow::Result<()> {
    directive_log::increment_retry_count(db.conn(), id)?;
    if (retry_count as u32) + 1 >= max_retries {
        directive_log::mark_abandoned(db.conn(), id)?;
        stats.abandoned += 1;
        println!(
            "  abandoned: {} after {} retries — {}",
            target_path,
            retry_count + 1,
            reason
        );
    } else {
        stats.deferred += 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::command_runner::MockRunner;
    use prism_db::directive_log::{
        priority, DirectiveLogRow, KIND_ENRICH, SOURCE_DIRECTIVE, STATE_PENDING,
    };
    use tempfile::TempDir;

    fn seed_project(root: &Path) {
        std::fs::create_dir_all(root.join(".prism")).unwrap();
        std::fs::write(root.join("CLAUDE.md"), "# root\n").unwrap();
        std::fs::write(root.join("CONTEXT.md"), "# routing\n").unwrap();
        std::fs::write(
            root.join(".prism/config.json"),
            r#"{"version":"0.1.0","enrichment":{"autopilot":{"enabled":true}}}"#,
        )
        .unwrap();
        let db = PrismDb::open(&root.join(".prism/prism.db")).unwrap();
        db.initialize().unwrap();
    }

    fn enqueue_enrich(root: &Path, target: &str, retry_count: i64) -> i64 {
        let db = PrismDb::open(&root.join(".prism/prism.db")).unwrap();
        directive_log::insert(
            db.conn(),
            &DirectiveLogRow {
                id: None,
                kind: KIND_ENRICH.into(),
                target_path: target.into(),
                session_id: "s".into(),
                emitted_at: 0,
                completed_at: None,
                retry_count,
                state: STATE_PENDING.into(),
                source: SOURCE_DIRECTIVE.into(),
                priority: priority::NORMAL,
            },
        )
        .unwrap()
    }

    #[test]
    fn empty_queue_yields_zero_stats() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        let mock = MockRunner::new();
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats, EnrichStats::default());
    }

    #[test]
    fn missing_target_dir_is_abandoned() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        enqueue_enrich(dir.path(), "no-such-dir", 0);
        let mock = MockRunner::new();
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.abandoned, 1);
        assert_eq!(stats.completed, 0);
    }

    #[test]
    fn claude_failure_defers_when_retries_remain() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("CLAUDE.md"), "# sub\n").unwrap();
        std::fs::write(sub.join("CONTEXT.md"), "# sub ctx\n").unwrap();
        enqueue_enrich(dir.path(), "sub", 0);
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(1, "boom"));
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.deferred, 1);
        assert_eq!(stats.abandoned, 0);
    }

    #[test]
    fn claude_failure_abandons_at_max_retries() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("CLAUDE.md"), "# sub\n").unwrap();
        std::fs::write(sub.join("CONTEXT.md"), "# sub ctx\n").unwrap();
        enqueue_enrich(dir.path(), "sub", 2); // max_retries default 3
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(1, "still bad"));
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.abandoned, 1);
        assert_eq!(stats.deferred, 0);
    }

    #[test]
    fn missing_enriched_marker_defers() {
        // Subprocess succeeds (exit 0) but does not touch CLAUDE.md → MarkerMissing
        // falls into the catch-all Ok(outcome) branch and defers.
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("CLAUDE.md"), "# sub\n").unwrap();
        std::fs::write(sub.join("CONTEXT.md"), "# sub ctx\n").unwrap();
        enqueue_enrich(dir.path(), "sub", 0);
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.deferred, 1);
        assert_eq!(stats.completed, 0);
    }
}

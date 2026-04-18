use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use prism_core::command_runner::{CommandRunner, SystemRunner};
use prism_core::config::{AutopilotConfig, PrismConfig};
use prism_core::enrich::{fix_icm_file_with, IcmFixOutcome};
use prism_core::icm::{load_settings, validate_icm, Scope};
use prism_db::{directive_log, PrismDb};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FixStats {
    pub total: usize,
    pub resolved: usize,
    pub deferred: usize,
    pub abandoned: usize,
}

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir()?;
    let db_path = project_root.join(".prism/prism.db");
    if !db_path.exists() {
        anyhow::bail!("PRISM not initialized. Run `prism start` first.");
    }

    let (cfg, _) = load_cfg(&project_root);
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

    let runner = SystemRunner;
    let stats = run_with(&project_root, &runner)?;
    println!(
        "Drained {} FIX_ICM directive(s): {} resolved, {} deferred, {} abandoned.",
        stats.total, stats.resolved, stats.deferred, stats.abandoned
    );
    Ok(())
}

pub fn run_with(project_root: &Path, runner: &dyn CommandRunner) -> anyhow::Result<FixStats> {
    let db_path = project_root.join(".prism/prism.db");
    let (cfg, max_retries) = load_cfg(project_root);
    let db = PrismDb::open(&db_path).context("open database")?;
    let pending =
        directive_log::list_pending_by_priority(db.conn(), directive_log::KIND_FIX_ICM, 50)?;

    let mut stats = FixStats {
        total: pending.len(),
        ..Default::default()
    };

    if pending.is_empty() {
        println!("No pending ICM fix directives.");
        return Ok(stats);
    }

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
            stats.abandoned += 1;
            println!("  abandoned: {} (missing file)", row.target_path);
            continue;
        }

        let settings = load_settings(project_root);
        let violations = validate_icm(project_root, &Scope::File(rel.clone()), settings);
        if violations.is_empty() {
            directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
            stats.resolved += 1;
            println!("  resolved:  {} (clean on re-check)", row.target_path);
            continue;
        }

        match fix_icm_file_with(project_root, &rel, &violations, &cfg, runner) {
            Ok(IcmFixOutcome::Resolved { .. }) => {
                directive_log::mark_completed(db.conn(), id, chrono::Utc::now().timestamp())?;
                stats.resolved += 1;
                println!("  resolved:  {}", row.target_path);
            }
            Ok(IcmFixOutcome::StillViolated { remaining, .. }) => {
                advance_or_abandon(
                    &db,
                    id,
                    row.retry_count,
                    max_retries,
                    &row.target_path,
                    &mut stats,
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
                    &mut stats,
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
                    &mut stats,
                    "timed out",
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
    stats: &mut FixStats,
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
        priority, DirectiveLogRow, KIND_FIX_ICM, SOURCE_DIRECTIVE, STATE_PENDING,
    };
    use tempfile::TempDir;

    /// Seed a project with .prism/prism.db and an L0/L1 scaffold so ICM lint
    /// focuses only on the stage file under test.
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

    fn enqueue_fix(root: &Path, target: &str) -> i64 {
        let db = PrismDb::open(&root.join(".prism/prism.db")).unwrap();
        directive_log::insert(
            db.conn(),
            &DirectiveLogRow {
                id: None,
                kind: KIND_FIX_ICM.into(),
                target_path: target.into(),
                session_id: "s".into(),
                emitted_at: 0,
                completed_at: None,
                retry_count: 0,
                state: STATE_PENDING.into(),
                source: SOURCE_DIRECTIVE.into(),
                priority: priority::NORMAL,
            },
        )
        .unwrap()
    }

    fn write_broken_stage(root: &Path) -> PathBuf {
        let stage = root.join("01-discovery");
        std::fs::create_dir_all(&stage).unwrap();
        let ctx = stage.join("CONTEXT.md");
        // Missing `## Outputs`
        std::fs::write(&ctx, "# s\n\n## Inputs\n\n## Process\n").unwrap();
        ctx
    }

    fn write_clean_stage(root: &Path) -> PathBuf {
        let stage = root.join("01-discovery");
        std::fs::create_dir_all(&stage).unwrap();
        let ctx = stage.join("CONTEXT.md");
        std::fs::write(&ctx, "# s\n\n## Inputs\n\n## Process\n\n## Outputs\n").unwrap();
        ctx
    }

    #[test]
    fn empty_queue_yields_zero_stats() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        let mock = MockRunner::new();
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats, FixStats::default());
    }

    #[test]
    fn missing_target_file_is_abandoned() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        enqueue_fix(dir.path(), "01-discovery/CONTEXT.md");
        let mock = MockRunner::new();
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.abandoned, 1);
        assert_eq!(stats.resolved, 0);
    }

    #[test]
    fn clean_target_resolves_without_calling_claude() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        write_clean_stage(dir.path());
        enqueue_fix(dir.path(), "01-discovery/CONTEXT.md");
        let mock = MockRunner::new();
        // No expect() registered — any call to runner would panic with NotFound.
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.resolved, 1);
        assert_eq!(stats.total, 1);
    }

    #[test]
    fn still_violated_defers_when_retries_remain() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        write_broken_stage(dir.path());
        enqueue_fix(dir.path(), "01-discovery/CONTEXT.md");
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let stats = run_with(dir.path(), &mock).unwrap();
        // File still missing `## Outputs` after subprocess no-op → StillViolated
        assert_eq!(stats.deferred, 1);
        assert_eq!(stats.abandoned, 0);
    }

    #[test]
    fn still_violated_abandons_after_max_retries() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        write_broken_stage(dir.path());
        // max_retries defaults to 3; retry_count=2 means next failure abandons.
        let db = PrismDb::open(&dir.path().join(".prism/prism.db")).unwrap();
        directive_log::insert(
            db.conn(),
            &DirectiveLogRow {
                id: None,
                kind: KIND_FIX_ICM.into(),
                target_path: "01-discovery/CONTEXT.md".into(),
                session_id: "s".into(),
                emitted_at: 0,
                completed_at: None,
                retry_count: 2,
                state: STATE_PENDING.into(),
                source: SOURCE_DIRECTIVE.into(),
                priority: priority::NORMAL,
            },
        )
        .unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.abandoned, 1);
        assert_eq!(stats.deferred, 0);
    }

    #[test]
    fn claude_failure_defers() {
        let dir = TempDir::new().unwrap();
        seed_project(dir.path());
        write_broken_stage(dir.path());
        enqueue_fix(dir.path(), "01-discovery/CONTEXT.md");
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(1, "boom"));
        let stats = run_with(dir.path(), &mock).unwrap();
        assert_eq!(stats.deferred, 1);
        assert_eq!(stats.resolved, 0);
    }
}

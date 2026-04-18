//! Autonomous CLAUDE.md enrichment via `claude -p` headless mode.
//!
//! Used by `prism enrich` to run a restricted Claude session per pending
//! directory. The caller is responsible for recording outcomes to the
//! `directive_log` table.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::command_runner::{CommandRunner, RunResult, SystemRunner};
use crate::config::AutopilotConfig;
use crate::icm::IcmViolation;
use crate::PrismError;

const ENRICHED_MARKER: &str = "<!-- prism:enriched -->";
const ENRICHED_CONTEXT_MARKER: &str = "<!-- prism:context-enriched -->";

/// Outcome of a single directory enrichment attempt.
#[derive(Debug, Clone)]
pub enum EnrichOutcome {
    /// CLAUDE.md was written and the enriched marker is present.
    Completed {
        /// Absolute path of the enriched directory.
        dir: PathBuf,
    },
    /// `claude -p` exited zero but the enriched marker is absent from CLAUDE.md.
    MarkerMissing {
        /// Absolute path of the directory whose CLAUDE.md is still unmarked.
        dir: PathBuf,
    },
    /// `claude -p` exited zero, CLAUDE.md is marked, but CONTEXT.md marker is absent.
    ContextMarkerMissing {
        /// Absolute path of the directory whose CONTEXT.md is still unmarked.
        dir: PathBuf,
    },
    /// The subprocess exceeded `cfg.timeout_secs`.
    TimedOut {
        /// Absolute path of the directory that timed out.
        dir: PathBuf,
    },
    /// The subprocess exited non-zero.
    Failed {
        /// Absolute path of the directory that failed.
        dir: PathBuf,
        /// First line of stderr from the subprocess.
        stderr: String,
    },
    /// Dry-run: shows the prompt that would be sent without executing.
    DryRun {
        /// Absolute path of the directory that would be enriched.
        dir: PathBuf,
        /// The prompt that would be passed to `claude -p`.
        prompt: String,
    },
}

/// Run `claude -p` headless on one directory and verify the enriched marker
/// appears in its `CLAUDE.md`.
///
/// * `dir` — absolute path to the directory to enrich.
/// * `project_root` — repository root (working directory for the subprocess).
/// * `cfg` — autopilot configuration (model, timeout, allowed tools).
/// * `dry_run` — when `true`, return [`EnrichOutcome::DryRun`] immediately.
pub fn enrich_directory(
    dir: &Path,
    project_root: &Path,
    cfg: &AutopilotConfig,
    dry_run: bool,
) -> Result<EnrichOutcome, PrismError> {
    enrich_directory_with(dir, project_root, cfg, dry_run, &SystemRunner)
}

/// Runner-injecting variant of [`enrich_directory`]. Production paths use
/// [`SystemRunner`]; tests inject a `MockRunner` with scripted responses.
pub fn enrich_directory_with(
    dir: &Path,
    project_root: &Path,
    cfg: &AutopilotConfig,
    dry_run: bool,
    runner: &dyn CommandRunner,
) -> Result<EnrichOutcome, PrismError> {
    let prompt = build_enrichment_prompt(dir, project_root);

    if dry_run {
        return Ok(EnrichOutcome::DryRun {
            dir: dir.to_path_buf(),
            prompt,
        });
    }

    let tools = cfg.allowed_tools.join(",");
    let args = [
        "-p",
        &prompt,
        "--model",
        &cfg.model,
        "--allowedTools",
        &tools,
        "--output-format",
        "json",
    ];

    let result = runner.run_timeout(
        "claude",
        &args,
        Some(project_root),
        None,
        Duration::from_secs(cfg.timeout_secs),
    )?;

    let output = match result {
        RunResult::Completed(o) => o,
        RunResult::TimedOut => {
            return Ok(EnrichOutcome::TimedOut {
                dir: dir.to_path_buf(),
            });
        }
    };

    if !output.success() {
        let stderr = output
            .stderr_str()
            .lines()
            .next()
            .unwrap_or("non-zero exit")
            .to_string();
        return Ok(EnrichOutcome::Failed {
            dir: dir.to_path_buf(),
            stderr,
        });
    }

    // Verify the enriched marker appeared in CLAUDE.md.
    let claude_md = dir.join("CLAUDE.md");
    let claude_content = std::fs::read_to_string(&claude_md).unwrap_or_default();
    if !claude_content.contains(ENRICHED_MARKER) {
        return Ok(EnrichOutcome::MarkerMissing {
            dir: dir.to_path_buf(),
        });
    }

    // For non-root directories, also verify CONTEXT.md was enriched.
    // Use the stripped relative path for root detection — `dir.join(".")` is not
    // lexically equal to `dir`, so compare via strip_prefix instead.
    let rel = dir.strip_prefix(project_root).unwrap_or(dir);
    let is_root = rel.as_os_str().is_empty() || rel == std::path::Path::new(".");
    if !is_root {
        let context_md = dir.join("CONTEXT.md");
        let context_content = std::fs::read_to_string(&context_md).unwrap_or_default();
        if !context_content.contains(ENRICHED_CONTEXT_MARKER) {
            return Ok(EnrichOutcome::ContextMarkerMissing {
                dir: dir.to_path_buf(),
            });
        }
    }

    Ok(EnrichOutcome::Completed {
        dir: dir.to_path_buf(),
    })
}

/// Build the `claude -p` prompt for enriching a single directory's `CLAUDE.md`
/// and (for non-root directories) `CONTEXT.md` in one headless invocation.
///
/// Root CONTEXT.md is DB-driven (`regenerate_context_md` in `session_start.rs`)
/// and must NOT be written here — only CLAUDE.md is enriched for the root dir.
pub fn build_enrichment_prompt(dir: &Path, project_root: &Path) -> String {
    let rel = dir.strip_prefix(project_root).unwrap_or(dir);
    let is_root = rel.as_os_str().is_empty() || rel == std::path::Path::new(".");

    if is_root {
        "Enrich the CLAUDE.md at the repository root of this project.\n\
            \n\
            Steps:\n\
            1. Read 2-3 key source files at the root to understand the project's actual purpose.\n\
            2. Create or update `CLAUDE.md` with accurate, project-specific descriptions.\n\
            \n\
            Rules for CLAUDE.md:\n\
            - Ensure `<!-- prism:managed -->` is on line 1.\n\
            - Add `<!-- prism:enriched -->` on line 2.\n\
            - Preserve the template structure: heading, blockquote summary, Purpose, Key Files,\n\
              Subdirectories, Conventions, Dependencies sections.\n\
            - Describe what the code actually does — do not infer from filenames alone.\n\
            - Be concise: 20-40 lines total."
            .to_string()
    } else {
        format!(
            "Enrich the CLAUDE.md and CONTEXT.md in the `{rel}` directory of this project.\n\
            \n\
            Steps:\n\
            1. Read 2-3 key source files in `{rel}/` to understand the directory's actual purpose\n\
               and what is currently happening there.\n\
            2. Create or update `{rel}/CLAUDE.md`.\n\
            3. Create or update `{rel}/CONTEXT.md`.\n\
            \n\
            Rules for CLAUDE.md (static — what IS here):\n\
            - Ensure `<!-- prism:managed -->` is on line 1.\n\
            - Add `<!-- prism:enriched -->` on line 2.\n\
            - Preserve the template structure: heading, blockquote summary, Purpose, Key Files,\n\
              Subdirectories, Conventions, Dependencies sections.\n\
            - Describe what the code actually does — do not infer from filenames alone.\n\
            - Be concise: 20-40 lines total.\n\
            \n\
            Rules for CONTEXT.md (dynamic — what is HAPPENING here):\n\
            - Ensure `<!-- prism:managed -->` is on line 1.\n\
            - Add `<!-- prism:context-enriched -->` on line 2.\n\
            - Describe active work, recent changes, open questions, and decisions in progress.\n\
            - Do not duplicate static structure from CLAUDE.md.\n\
            - Be concise: 10-20 lines total.",
            rel = rel.display()
        )
    }
}

// ---------------------------------------------------------------------------
// ICM fix-mode
// ---------------------------------------------------------------------------

/// Outcome of a single `FIX_ICM` directive.
#[derive(Debug, Clone)]
pub enum IcmFixOutcome {
    /// Haiku rewrote the file and re-validation returned zero violations.
    Resolved { file: PathBuf },
    /// Subprocess ran zero-exit but violations remain.
    StillViolated {
        file: PathBuf,
        remaining: Vec<IcmViolation>,
    },
    /// Subprocess exceeded `cfg.timeout_secs`.
    TimedOut { file: PathBuf },
    /// Subprocess exited non-zero.
    Failed { file: PathBuf, stderr: String },
}

/// Build the prompt Haiku uses to fix a single managed markdown file.
pub fn build_icm_fix_prompt(rel_path: &Path, violations: &[IcmViolation]) -> String {
    let rules: Vec<String> = violations
        .iter()
        .map(|v| format!("- {}: {}", v.rule.id(), v.message))
        .collect();
    format!(
        "The file `{rel}` violates the ICM (Interpreted Context Methodology) spec.\n\
         \n\
         Violated rules:\n\
         {rules}\n\
         \n\
         Canonical spec: https://github.com/RinDig/Interpreted-Context-Methdology/blob/main/_core/CONVENTIONS.md\n\
         \n\
         Fix the file in place. Preserve the `<!-- prism:managed -->` marker on line 1 if present. Do not introduce em dashes (U+2014). CONTEXT.md files should stay routing-only (links + short prose), not duplicate structure from CLAUDE.md. Stage-level CONTEXT.md files must contain headings `## Inputs`, `## Process`, `## Outputs`. Keep CONTEXT.md files under 80 lines.\n\
         \n\
         Return the corrected file content only.",
        rel = rel_path.display(),
        rules = rules.join("\n"),
    )
}

/// Run a single `FIX_ICM` directive against the given managed markdown file.
///
/// Re-runs [`validate_icm`] after the subprocess completes to decide the
/// outcome.
pub fn fix_icm_file(
    project_root: &Path,
    rel_path: &Path,
    violations: &[IcmViolation],
    cfg: &AutopilotConfig,
) -> Result<IcmFixOutcome, PrismError> {
    fix_icm_file_with(project_root, rel_path, violations, cfg, &SystemRunner)
}

pub fn fix_icm_file_with(
    project_root: &Path,
    rel_path: &Path,
    violations: &[IcmViolation],
    cfg: &AutopilotConfig,
    runner: &dyn CommandRunner,
) -> Result<IcmFixOutcome, PrismError> {
    let prompt = build_icm_fix_prompt(rel_path, violations);
    let tools = cfg.allowed_tools.join(",");
    let args = [
        "-p",
        &prompt,
        "--model",
        &cfg.model,
        "--allowedTools",
        &tools,
        "--output-format",
        "json",
    ];

    let abs_path = if rel_path.is_absolute() {
        rel_path.to_path_buf()
    } else {
        project_root.join(rel_path)
    };

    let result = runner.run_timeout(
        "claude",
        &args,
        Some(project_root),
        None,
        Duration::from_secs(cfg.timeout_secs),
    )?;

    match result {
        RunResult::TimedOut => Ok(IcmFixOutcome::TimedOut {
            file: abs_path,
        }),
        RunResult::Completed(out) if !out.success() => {
            let stderr = out
                .stderr_str()
                .lines()
                .next()
                .unwrap_or("non-zero exit")
                .to_string();
            Ok(IcmFixOutcome::Failed {
                file: abs_path,
                stderr,
            })
        }
        RunResult::Completed(_) => {
            let remaining = crate::icm::validate_icm(
                project_root,
                &crate::icm::Scope::File(rel_path.to_path_buf()),
                crate::icm::IcmSettings::default(),
            );
            if remaining.is_empty() {
                Ok(IcmFixOutcome::Resolved { file: abs_path })
            } else {
                Ok(IcmFixOutcome::StillViolated {
                    file: abs_path,
                    remaining,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Initial-scaffold Haiku helpers (PR 3)
// ---------------------------------------------------------------------------

/// Which file a Haiku initial-scaffold prompt is targeting.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaffoldKind {
    /// Project root `CLAUDE.md`.
    RootClaudeMd,
    /// Project root `CONTEXT.md`.
    RootContextMd,
    /// A subdirectory `CLAUDE.md`.
    DirectoryClaudeMd,
    /// A subdirectory `CONTEXT.md`.
    DirectoryContextMd,
}

impl ScaffoldKind {
    fn file_name(&self) -> &'static str {
        match self {
            Self::RootClaudeMd | Self::DirectoryClaudeMd => "CLAUDE.md",
            Self::RootContextMd | Self::DirectoryContextMd => "CONTEXT.md",
        }
    }
}

/// Build the prompt fed to `claude -p` when generating a fresh CLAUDE.md or
/// CONTEXT.md for a newly scaffolded directory. Pure string builder.
///
/// The template-rendered placeholder is included verbatim so Haiku preserves
/// heading structure and managed markers.
pub fn build_initial_scaffold_prompt(
    kind: ScaffoldKind,
    dir_rel_path: &Path,
    project_root: &Path,
    template_rendered: &str,
) -> String {
    let rel = dir_rel_path.display();
    let root = project_root.display();
    let file = kind.file_name();
    let role = match kind {
        ScaffoldKind::RootClaudeMd | ScaffoldKind::DirectoryClaudeMd => {
            "static description of what IS in this directory (structure, purpose, conventions)"
        }
        ScaffoldKind::RootContextMd | ScaffoldKind::DirectoryContextMd => {
            "dynamic state of what is HAPPENING in this directory (active work, recent changes)"
        }
    };
    let marker = match kind {
        ScaffoldKind::RootClaudeMd | ScaffoldKind::DirectoryClaudeMd => "<!-- prism:enriched -->",
        ScaffoldKind::RootContextMd | ScaffoldKind::DirectoryContextMd => {
            "<!-- prism:context-enriched -->"
        }
    };
    let length = match kind {
        ScaffoldKind::RootClaudeMd | ScaffoldKind::DirectoryClaudeMd => "20-40 lines",
        _ => "10-20 lines",
    };
    format!(
        "You are generating initial `{file}` content for the `{rel}` directory of \
         a project using the ICM (Interpreted Context Methodology) documentation \
         standard. Project root: `{root}`.\n\
         \n\
         `{file}` captures the {role}.\n\
         \n\
         Inspect 2-3 key source files inside `{rel}/` to ground the content in \
         what the code actually does. Do not invent facts.\n\
         \n\
         Constraints:\n\
         - Ensure `<!-- prism:managed -->` is on line 1.\n\
         - Add `{marker}` on line 2.\n\
         - Preserve the template section headings shown below; fill content only.\n\
         - Use lowercase-with-hyphens in any path references.\n\
         - Keep output under {length}.\n\
         - Emit final file content only — no preamble, no explanations.\n\
         \n\
         Template skeleton to preserve and fill:\n\
         ---\n\
         {template_rendered}\n\
         ---\n"
    )
}

/// Run `claude -p` synchronously and return stdout on success.
///
/// Does not parse or write files — the caller decides what to do with the
/// output. Respects `cfg.timeout_secs` via a worker thread + mpsc channel.
pub fn invoke_claude_haiku_blocking(
    prompt: &str,
    project_root: &Path,
    cfg: &AutopilotConfig,
) -> Result<String, PrismError> {
    invoke_claude_haiku_blocking_with(prompt, project_root, cfg, &SystemRunner)
}

/// Runner-injecting variant of [`invoke_claude_haiku_blocking`].
pub fn invoke_claude_haiku_blocking_with(
    prompt: &str,
    project_root: &Path,
    cfg: &AutopilotConfig,
    runner: &dyn CommandRunner,
) -> Result<String, PrismError> {
    let tools = cfg.allowed_tools.join(",");
    let args = [
        "-p",
        prompt,
        "--model",
        &cfg.model,
        "--allowed-tools",
        &tools,
    ];

    let result = runner
        .run_timeout(
            "claude",
            &args,
            Some(project_root),
            None,
            Duration::from_secs(cfg.timeout_secs),
        )
        .map_err(|e| PrismError::Io(std::io::Error::other(format!("spawn claude: {e}"))))?;

    match result {
        RunResult::TimedOut => Err(PrismError::Io(std::io::Error::other("claude -p timed out"))),
        RunResult::Completed(out) if out.success() => String::from_utf8(out.stdout)
            .map_err(|e| PrismError::Io(std::io::Error::other(format!("stdout utf8: {e}")))),
        RunResult::Completed(out) => {
            let first = out.stderr_str().lines().next().unwrap_or("").to_string();
            Err(PrismError::Io(std::io::Error::other(format!(
                "claude -p failed: {first}"
            ))))
        }
    }
}

#[cfg(test)]
mod initial_scaffold_prompt_tests {
    use super::*;

    #[test]
    fn prompt_includes_directory_and_markers() {
        let prompt = build_initial_scaffold_prompt(
            ScaffoldKind::DirectoryClaudeMd,
            Path::new("src/services"),
            Path::new("/proj"),
            "# services\n\n> Business logic.",
        );
        assert!(prompt.contains("src/services"));
        assert!(prompt.contains("CLAUDE.md"));
        assert!(prompt.contains("<!-- prism:enriched -->"));
        assert!(prompt.contains("Business logic"));
    }

    #[test]
    fn prompt_for_context_md_uses_context_marker() {
        let prompt = build_initial_scaffold_prompt(
            ScaffoldKind::DirectoryContextMd,
            Path::new("src"),
            Path::new("/proj"),
            "tpl",
        );
        assert!(prompt.contains("<!-- prism:context-enriched -->"));
        assert!(!prompt.contains("<!-- prism:enriched -->\n"));
    }
}

#[cfg(test)]
mod icm_fix_tests {
    use super::*;
    use crate::command_runner::{MockRunner, RunResult};
    use crate::icm::{IcmRule, IcmViolation};
    use tempfile::TempDir;

    fn seed_clean_stage(root: &Path) -> PathBuf {
        let stage = root.join("01-discovery");
        std::fs::create_dir_all(&stage).unwrap();
        let ctx = stage.join("CONTEXT.md");
        std::fs::write(
            &ctx,
            "# s\n\n## Inputs\n\n## Process\n\n## Outputs\n",
        )
        .unwrap();
        // Also seed L0/L1 so whole-project scope would be clean; this test uses
        // Scope::File so those aren't strictly required, but doesn't hurt.
        std::fs::write(root.join("CLAUDE.md"), "# root\n").unwrap();
        std::fs::write(root.join("CONTEXT.md"), "# routing\n").unwrap();
        ctx
    }

    #[test]
    fn prompt_names_rule_ids_and_cites_spec() {
        let v = vec![
            IcmViolation::at_file(
                IcmRule::StageContextSections,
                PathBuf::from("01-discovery/CONTEXT.md"),
                "missing `## Outputs`",
            ),
            IcmViolation::at_line(
                IcmRule::NoEmDash,
                PathBuf::from("01-discovery/CONTEXT.md"),
                3,
                "em dash at column 10",
            ),
        ];
        let prompt = build_icm_fix_prompt(Path::new("01-discovery/CONTEXT.md"), &v);
        assert!(prompt.contains("STAGE_CONTEXT_SECTIONS"));
        assert!(prompt.contains("NO_EM_DASH"));
        assert!(prompt.contains("Interpreted-Context-Methdology"));
        assert!(prompt.contains("01-discovery/CONTEXT.md"));
    }

    #[test]
    fn resolved_when_revalidation_clean() {
        let dir = TempDir::new().unwrap();
        let ctx = seed_clean_stage(dir.path());
        let rel = ctx.strip_prefix(dir.path()).unwrap().to_path_buf();
        let mock = MockRunner::new();
        // Runner is a no-op; the file is already clean so validate returns empty.
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = fix_icm_file_with(
            dir.path(),
            &rel,
            &[IcmViolation::at_file(
                IcmRule::StageContextSections,
                rel.clone(),
                "missing",
            )],
            &AutopilotConfig::default(),
            &mock,
        )
        .unwrap();
        assert!(matches!(out, IcmFixOutcome::Resolved { .. }));
    }

    #[test]
    fn still_violated_when_file_stays_broken() {
        let dir = TempDir::new().unwrap();
        // Seed a stage CONTEXT.md that is STILL broken (missing Outputs heading).
        let stage = dir.path().join("01-discovery");
        std::fs::create_dir_all(&stage).unwrap();
        let ctx = stage.join("CONTEXT.md");
        std::fs::write(&ctx, "# s\n\n## Inputs\n\n## Process\n").unwrap();
        let rel = ctx.strip_prefix(dir.path()).unwrap().to_path_buf();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = fix_icm_file_with(
            dir.path(),
            &rel,
            &[IcmViolation::at_file(
                IcmRule::StageContextSections,
                rel.clone(),
                "missing",
            )],
            &AutopilotConfig::default(),
            &mock,
        )
        .unwrap();
        match out {
            IcmFixOutcome::StillViolated { remaining, .. } => {
                assert!(remaining.iter().any(|v| v.rule == IcmRule::StageContextSections));
            }
            other => panic!("expected StillViolated, got {other:?}"),
        }
    }

    #[test]
    fn failed_on_nonzero_exit() {
        let dir = TempDir::new().unwrap();
        let ctx = seed_clean_stage(dir.path());
        let rel = ctx.strip_prefix(dir.path()).unwrap().to_path_buf();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(1, "boom\ntail"));
        let out = fix_icm_file_with(
            dir.path(),
            &rel,
            &[],
            &AutopilotConfig::default(),
            &mock,
        )
        .unwrap();
        match out {
            IcmFixOutcome::Failed { stderr, .. } => assert_eq!(stderr, "boom"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn timed_out_on_deadline() {
        let dir = TempDir::new().unwrap();
        let ctx = seed_clean_stage(dir.path());
        let rel = ctx.strip_prefix(dir.path()).unwrap().to_path_buf();
        let mock = MockRunner::new();
        mock.expect_timeout(
            "claude",
            Some("-p"),
            MockRunner::ok(""),
            Ok(RunResult::TimedOut),
        );
        let out = fix_icm_file_with(
            dir.path(),
            &rel,
            &[],
            &AutopilotConfig::default(),
            &mock,
        )
        .unwrap();
        assert!(matches!(out, IcmFixOutcome::TimedOut { .. }));
    }
}

#[cfg(test)]
mod enrich_directory_tests {
    use super::*;
    use crate::command_runner::{MockRunner, RunResult};
    use tempfile::TempDir;

    fn cfg() -> AutopilotConfig {
        AutopilotConfig::default()
    }

    fn write_marked_claude(dir: &Path) {
        std::fs::write(
            dir.join("CLAUDE.md"),
            format!("<!-- prism:managed -->\n{ENRICHED_MARKER}\n# dir\n"),
        )
        .unwrap();
    }

    fn write_marked_context(dir: &Path) {
        std::fs::write(
            dir.join("CONTEXT.md"),
            format!("<!-- prism:managed -->\n{ENRICHED_CONTEXT_MARKER}\n# dir\n"),
        )
        .unwrap();
    }

    #[test]
    fn dry_run_short_circuits_without_runner_call() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        // No scripts registered — if runner.run_timeout is called, test fails.
        let out =
            enrich_directory_with(dir.path(), dir.path(), &cfg(), true, &mock).expect("dry-run");
        assert!(matches!(out, EnrichOutcome::DryRun { .. }));
    }

    #[test]
    fn completed_root_when_claude_md_marker_present() {
        let dir = TempDir::new().unwrap();
        write_marked_claude(dir.path());
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap();
        assert!(matches!(out, EnrichOutcome::Completed { .. }));
    }

    #[test]
    fn completed_subdir_requires_both_markers() {
        let root = TempDir::new().unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        write_marked_claude(&sub);
        write_marked_context(&sub);
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = enrich_directory_with(&sub, root.path(), &cfg(), false, &mock).unwrap();
        assert!(matches!(out, EnrichOutcome::Completed { .. }));
    }

    #[test]
    fn marker_missing_when_claude_md_absent() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap();
        assert!(matches!(out, EnrichOutcome::MarkerMissing { .. }));
    }

    #[test]
    fn context_marker_missing_in_subdir() {
        let root = TempDir::new().unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        write_marked_claude(&sub);
        // CONTEXT.md intentionally absent
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("{}"));
        let out = enrich_directory_with(&sub, root.path(), &cfg(), false, &mock).unwrap();
        assert!(matches!(out, EnrichOutcome::ContextMarkerMissing { .. }));
    }

    #[test]
    fn failed_surfaces_stderr_first_line() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect(
            "claude",
            Some("-p"),
            MockRunner::fail(1, "boom\nsecond-line"),
        );
        let out = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap();
        match out {
            EnrichOutcome::Failed { stderr, .. } => assert_eq!(stderr, "boom"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn failed_uses_placeholder_on_empty_stderr() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(2, ""));
        let out = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap();
        match out {
            EnrichOutcome::Failed { stderr, .. } => assert_eq!(stderr, "non-zero exit"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn timed_out_on_deadline() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect_timeout(
            "claude",
            Some("-p"),
            MockRunner::ok("ignored"),
            Ok(RunResult::TimedOut),
        );
        let out = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap();
        assert!(matches!(out, EnrichOutcome::TimedOut { .. }));
    }

    #[test]
    fn runner_io_error_bubbles() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::not_found());
        let err = enrich_directory_with(dir.path(), dir.path(), &cfg(), false, &mock).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.to_lowercase().contains("not found") || msg.contains("program not found"));
    }
}

#[cfg(test)]
mod invoke_claude_haiku_tests {
    use super::*;
    use crate::command_runner::{MockRunner, RunResult};
    use tempfile::TempDir;

    #[test]
    fn returns_stdout_on_success() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::ok("hello"));
        let s = invoke_claude_haiku_blocking_with(
            "prompt",
            dir.path(),
            &AutopilotConfig::default(),
            &mock,
        )
        .unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn returns_error_on_non_zero_exit() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::fail(3, "nope\ntrail"));
        let err =
            invoke_claude_haiku_blocking_with("p", dir.path(), &AutopilotConfig::default(), &mock)
                .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("claude -p failed"));
        assert!(msg.contains("nope"));
    }

    #[test]
    fn returns_error_on_timeout() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect_timeout(
            "claude",
            Some("-p"),
            MockRunner::ok(""),
            Ok(RunResult::TimedOut),
        );
        let err =
            invoke_claude_haiku_blocking_with("p", dir.path(), &AutopilotConfig::default(), &mock)
                .unwrap_err();
        assert!(format!("{err}").contains("timed out"));
    }

    #[test]
    fn returns_error_on_spawn_failure() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect("claude", Some("-p"), MockRunner::not_found());
        let err =
            invoke_claude_haiku_blocking_with("p", dir.path(), &AutopilotConfig::default(), &mock)
                .unwrap_err();
        assert!(format!("{err}").contains("spawn claude"));
    }

    #[test]
    fn returns_error_on_invalid_utf8() {
        let dir = TempDir::new().unwrap();
        let mock = MockRunner::new();
        mock.expect(
            "claude",
            Some("-p"),
            Ok(crate::command_runner::CommandOutput {
                status: Some(0),
                stdout: vec![0xFF, 0xFE, 0xFD],
                stderr: Vec::new(),
            }),
        );
        let err =
            invoke_claude_haiku_blocking_with("p", dir.path(), &AutopilotConfig::default(), &mock)
                .unwrap_err();
        assert!(format!("{err}").contains("stdout utf8"));
    }
}

# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/).

## [0.2.0] — 2026-04-18

### Added
- ICM enforcement surface:
  - `/prism:lint` — read-only audit of layer existence, stage-folder shape, line budgets, heading schema, and em-dash style.
  - `/prism:fix` — drain pending `FIX_ICM` directives via Haiku, re-validate, and resolve/defer/abandon per retry budget.
  - Non-blocking `PostToolUse` validator that surfaces new violations through the drift log.
  - `icm.allow_em_dash` config override to relax the em-dash rule per project.
- CI workflow at `.github/workflows/ci.yml` — runs `cargo build`, `cargo test`, and `cargo clippy -D warnings` with `--locked` on every push and PR.
- `Cargo.lock` is now tracked (binary workspace convention).
- Injectable `CommandRunner` entry points for the CLI:
  - `cmd_fix::run_with(project_root, runner) -> FixStats`
  - `cmd_enrich::run_with(project_root, runner) -> EnrichStats`
- 11 new unit tests covering `cmd_fix` (6) and `cmd_enrich` (5) state transitions (resolve/defer/abandon, max-retry budget, missing files, subprocess failure).

### Changed
- Crate versions bumped from `0.1.0` to `0.2.0` to match `.claude-plugin/plugin.json`.
- `SessionStart` hook message now surfaces unresolved ICM violations alongside source drift counts.
- `doc_drift.insert` dedupes on `(affected_doc, drift_type, description)` for unresolved rows so repeated hook fires do not accumulate.
- Drift attribution now walks ancestor `CLAUDE.md` / `CONTEXT.md` pairs instead of matching only the nearest directory.
- `cmd_status` splits source drift from ICM violations to prevent double-counting.
- `IcmSettings` uses `#[derive(Default)]` (replaces a manual impl).
- `doc_drift`'s test module moved below public functions to satisfy `clippy::items_after_test_module`.

### Removed
- v1 concepts no longer supported: decisions, goals, stage contracts, pre-commit git hook, watcher daemon, `PRISM:ASK` directives, ICM renaming.
- DB tables: `schema_migrations`, `document_dependencies`, `enrichment_runs` (6 tables → 4).
- Unused deps: `tracing`, `tracing-subscriber`, `globset`, `walkdir`, `pulldown-cmark`, `rstest`, `serde_yaml_ng`.
- Dead modules/types: `WatcherConfig`, `StageDefinition`, `HooksConfig`, `ScaffoldKind`, `InitialScaffoldMode`, `SOURCE_AUTOPILOT`, `DbError::NotFound`, `rollback_hash`, `mark_pending`, `list_targets_in_state`, `batch_size`, `invoke_claude_haiku_blocking`, `validate`, `content`, `DriftReport`, 5 `doc_drift` helpers.

### Fixed
- Enrich `claude-cli` preflight with `max_retries` honours the configured retry budget before abandoning a directive.
- Duplicate hook fires no longer accumulate drift rows with identical `(doc, description)` pairs.

## [0.1.0]

Initial internal scaffold. Not released.

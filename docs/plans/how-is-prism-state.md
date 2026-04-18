# Prism State After Porting — Audit + Test Report

## Context

User requested examination of `prism` (Claude Code plugin, Rust workspace) after a series of pruning commits (`29ec619` → `769631f`) that stripped v1 concepts (decisions, goals, stage contracts, watcher daemon, PRISM:ASK) and collapsed the schema from 6 → 4 tables. Goal: verify the post-porting state is coherent and all test tiers pass.

This is a **read-only audit** — no code changes proposed. Findings below inform whether further work is needed.

---

## Workspace Map

**3 crates, all compile clean (`cargo build --workspace --tests`):**

| Crate | Modules | Purpose |
|-------|---------|---------|
| `prism-cli` | 7 cmd_* modules + main.rs | Subcommands: `start`, `stop`, `status`, `enrich`, `hook`, `lint`, `fix` |
| `prism-core` | hooks/, icm/, templates/, config, enrich, command_runner, hashing | Discovery, drift, scaffold, hooks protocol, ICM rules |
| `prism-db` | schema, document_registry, file_hashes, doc_drift, directive_log | SQLite state (rusqlite bundled) |

**DB tables (4):** `document_registry`, `file_hashes`, `doc_drift`, `directive_log` — matches commit `0296065` ("5 to 4").

**Hooks:** `SessionStart` ([crates/prism-core/src/hooks/session_start/mod.rs:11](crates/prism-core/src/hooks/session_start/mod.rs#L11)), `PostToolUse` ([crates/prism-core/src/hooks/post_tool_use/mod.rs:17](crates/prism-core/src/hooks/post_tool_use/mod.rs#L17)).

**Templates:** 5 root (PRISM/CLAUDE/CONTEXT/dir-CLAUDE/dir-CONTEXT) + 6 rules + 5 refs.

**Skills (5):** `prism-start`, `prism-stop`, `prism-status`, `prism-lint`, `prism-fix`.

---

## Test Results

Command: `cargo test --workspace --no-fail-fast`

| Suite | Path | Pass | Fail | Ignored |
|-------|------|-----:|-----:|--------:|
| prism-cli unit | crates/prism-cli/src | 0 | 0 | 0 |
| prism-cli integration (e2e) | [crates/prism-cli/tests/integration.rs](crates/prism-cli/tests/integration.rs) | **11** | 0 | 0 |
| prism-core unit (smoke) | crates/prism-core/src | **89** | 0 | 0 |
| prism-db unit (smoke) | crates/prism-db/src | **11** | 0 | 0 |
| prism-core doctests | hashing.rs:19 | 1 | 0 | 0 |
| **Total** | | **112** | **0** | **0** |

**E2E coverage (11 tests in `integration.rs`):**
- `start_status_hook_pipeline` — full start → 2× post-tool-use → status → session-start → stop
- `hook_is_noop_without_prism_dir` — hook graceful when `.prism/` absent
- `enrich_without_claude_cli_bails` — enrich requires `claude` on PATH
- `icm_lint_clean_project_succeeds` / `icm_lint_detects_missing_context_md` — lint flow
- `icm_violation_detected_and_queued_by_hook` — PostToolUse queues violations
- `session_start_surfaces_icm_violations_in_message` — commit `868958d` test
- `duplicate_hook_fires_do_not_accumulate_drift_rows` — dedupe
- `allow_em_dash_config_disables_em_dash_rule` — config override
- `prism_fix_without_claude_cli_bails` — fix requires claude
- 1 additional uninitialized-project case

---

## Scope Boundary Check

**Confirmed ABSENT (correctly pruned):**
- decisions / goals / stage contracts (v1) — zero matches under `templates/`
- `schema_migrations`, `document_dependencies`, `enrichment_runs` tables — removed
- `WatcherConfig`, `StageDefinition`, `HooksConfig`, `ScaffoldKind`, `InitialScaffoldMode`, `SOURCE_AUTOPILOT`, `DbError::NotFound`, `rollback_hash`, `mark_pending`, `list_targets_in_state`, `batch_size`, `invoke_claude_haiku_blocking` — removed
- Watcher daemon, pre-commit git hook, PRISM:ASK directives — absent
- Unused deps (`tracing`, `globset`, `walkdir`, `pulldown-cmark`, `rstest`, `tracing-subscriber`, `serde_yaml_ng`) — dropped

**Confirmed PRESENT (in-scope v2.1):**
- ICM enforcement via 5 rule modules: `layer_existence`, `sections`, `style`, `budgets`, `stage_shape` ([crates/prism-core/src/icm/rules/](crates/prism-core/src/icm/rules/))
- Non-blocking PostToolUse validator + `/prism:lint` + `/prism:fix` (v0.2.0, commit `3479d3e`)
- Plugin manifest [.claude-plugin/plugin.json](.claude-plugin/plugin.json) at v0.2.0

**Stale-audit flags rechecked — both resolved:**
- `templates/rules/prism-document-standard.md.template:10` "decisions" reference → **no match, already pruned**
- Root [CLAUDE.md:15](CLAUDE.md#L15) subcommand list → **already lists all 7: start, stop, status, enrich, hook, lint, fix**

---

## Verdict

**State: clean.** Port looks complete:
- 112/112 tests pass across unit, integration, e2e, and doctests
- Zero compile errors in build
- Scope boundary from root CLAUDE.md matches code reality
- Two flags from `docs/plans/stale-dead-code-audit-prism-v2-1.md` already fixed

---

## Clippy Findings (`cargo clippy --workspace --all-targets`)

**2 warnings, 0 errors:**

1. [crates/prism-db/src/doc_drift.rs:53](crates/prism-db/src/doc_drift.rs#L53) — `clippy::items_after_test_module`. `mod exists_unresolved_tests` declared at line 53 but `pub fn insert` and other items follow at line 108+. Fix: move the `#[cfg(test)] mod` block to the bottom of the file.

2. [crates/prism-core/src/icm/mod.rs:94](crates/prism-core/src/icm/mod.rs#L94) — `clippy::derivable_impls`. Manual `impl Default for IcmSettings { ... allow_em_dash: false }`. Fix: `#[derive(Default)]` on the struct (line 89) and drop lines 94-100.

---

## Follow-Up Plan (approved 2026-04-18)

### Task 1 — Delete stale audit plan
- Remove [docs/plans/stale-dead-code-audit-prism-v2-1.md](docs/plans/stale-dead-code-audit-prism-v2-1.md). Both flags inside (`decisions` in prism-document-standard.md.template, CLAUDE.md subcommand list) are already resolved in code.

### Task 2 — Update README.md
Current [README.md](README.md) lists 3 commands, missing `enrich`, `lint`, `fix`. Edit:
- **Scope section:** add bullets for "Enforce ICM layer / section / style / budget / stage-shape rules" and "Auto-fix ICM violations via Haiku"
- **Commands section:** add `/prism:lint`, `/prism:fix` entries. Note: `prism enrich` CLI runs internally via hooks; no user-facing slash command (intentional per fresh-rebuild-plan).

### Task 3 — Run `cargo clippy`
Already executed — findings above. Remediation folded into Task 5.

### Task 4 — Add CI workflow
Create `.github/workflows/ci.yml`:
```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --workspace --tests
      - run: cargo test --workspace --no-fail-fast
      - run: cargo clippy --workspace --all-targets -- -D warnings
```

### Task 5 — Fix clippy warnings (so CI passes green)
- `prism-db/src/doc_drift.rs` — move `#[cfg(test)] mod exists_unresolved_tests` block to end of file
- `prism-core/src/icm/mod.rs` — replace manual `Default` impl with `#[derive(Default)]` on `IcmSettings`
- Rerun `cargo test --workspace` to confirm green

### Task 6 — Housekeeping
- Copy this plan to `docs/plans/how-is-prism-state.md` (per global CLAUDE.md rule)

---

## Verification

```bash
cd /Users/unfazed-mac/Developer/artificial_intelligence/plugins/prism
cargo build --workspace --tests
cargo test --workspace --no-fail-fast                     # expect 112 pass
cargo clippy --workspace --all-targets -- -D warnings     # expect 0 warnings after Task 5
ls .github/workflows/ci.yml                               # Task 4 artifact
ls docs/plans/stale-dead-code-audit-prism-v2-1.md         # expect no-such-file after Task 1
ls docs/plans/how-is-prism-state.md                       # Task 6 artifact
```

### Files modified/created/deleted
- **Delete:** `docs/plans/stale-dead-code-audit-prism-v2-1.md`
- **Edit:** `README.md`, `crates/prism-db/src/doc_drift.rs`, `crates/prism-core/src/icm/mod.rs`
- **Create:** `.github/workflows/ci.yml`, `docs/plans/how-is-prism-state.md`

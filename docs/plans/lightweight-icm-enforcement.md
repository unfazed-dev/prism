# PRISM v2.1 — Lightweight ICM Enforcement

## Context

PRISM v2 successfully shrunk from a multi-module state manager to a doc-sync plugin (114 tests, 3965 LOC, 6 tables, 2 hooks, 3 skills). It preserves ICM *surface* conventions (`<!-- prism:managed -->` marker, CLAUDE.md/CONTEXT.md split, section scaffolds) but does **not** enforce them — Claude can still write broken ICM docs and nothing stops it.

Goal: bolt a **non-blocking ICM validator** onto the existing surface that warns + queues Haiku fixes whenever an `.md` edit violates canonical ICM shape. Must NOT regress into v1's watcher/goals/stages/freshness architecture.

User-confirmed scope:
- **Mode**: warn + queue fix (PostToolUse hook inserts violations into db + enqueues Haiku fix). No blocking PreToolUse denies.
- **Spec**: full RinDig ICM — canonical source `_core/CONVENTIONS.md` at https://github.com/RinDig/Interpreted-Context-Methdology. Validator targets layers **L0 (root CLAUDE.md)**, **L1 (root CONTEXT.md)**, **L2 (per-stage CONTEXT.md)**. L3/L4 out of scope for v2.1.

Research findings that shaped the plan:
- No existing plugin enforces ICM dual-file shape — real gap (`agnix` is closest but doesn't know ICM). Native PRISM rules it is.
- RinDig spec defines zero managed-markers — PRISM's `<!-- prism:managed -->` is a PRISM extension and stays PRISM-extension, not cited as ICM-required.
- Canonical separator for stage folders is hyphen (`01-discovery/`), not underscore — spec says so; arXiv paper contradicts itself.
- 8 load-bearing mechanical rules exist; the other "Pattern" material is philosophy (not lintable).

## Design

### Approach

1. New pure-logic `icm` module in `prism-core` — no IO, no db, no hook coupling. Just takes a project-root path and returns `Vec<IcmViolation>`.
2. `post_tool_use` hook extended — after hashing an `.md` edit, call the validator on the affected file's scope, insert one `doc_drift` row per violation with `drift_type="IcmViolation"`, enqueue one `directive_log` row with `kind="FIX_ICM"`.
3. Existing `prism enrich` pipeline drains `FIX_ICM` directives using a targeted Haiku prompt that names the violated rule and cites the canonical fix.
4. Two new user-invocable skills `/prism:lint` + `/prism:fix` wrap `prism lint` + `prism fix` CLI subcommands for manual inspection.
5. One new rules template `icm-conventions.md.template` scaffolded at `prism start` — Claude reads it via the existing rules-loading path and self-corrects before even writing bad shape (free prevention on top of post-hoc validation).

Zero new SQLite tables. Table budget stays 6. LOC budget: estimated +700 net (validator + tests + skills + cmd files), landing ≈ 4700 — under plan's 5k target.

### The 8 ICM rules (MVP)

Each rule gets its own file under `crates/prism-core/src/icm/rules/` and its own unit tests. All 8 are mechanical — no NLP, no LLM calls inside the validator.

| Rule id | What it checks | Fail mode |
|---|---|---|
| `L0_EXISTS` | Exactly one `CLAUDE.md` at project root | Missing/misplaced |
| `L1_EXISTS` | Exactly one `CONTEXT.md` at project root | Missing |
| `L2_ONE_PER_STAGE` | Every `NN-slug/` stage folder has a `CONTEXT.md` | Missing |
| `STAGE_FOLDER_SHAPE` | Stage folders match `^\d{2}-[a-z0-9-]+$`, sequential from `01`, no gaps | Regex or gap fail |
| `CONTEXT_LINE_BUDGET` | `CONTEXT.md` ≤ 80 lines; reference files ≤ 200 | Over budget |
| `STAGE_CONTEXT_SECTIONS` | Stage `CONTEXT.md` contains `Inputs`, `Process`, `Outputs` headings (Audit only for creative/build stages) | Missing heading |
| `INPUTS_TABLE_COLUMNS` | `Inputs` table columns are exactly `Source \| File/Location \| Section/Scope \| Why` | Schema mismatch |
| `NO_EM_DASH` | No `—` character anywhere in managed `.md` files | Present |

Non-mechanical "one-way references" and "output/ gitkeep-only" rules deferred to v2.2 — they need cross-file graph walks that would balloon the validator.

### File map

**New files:**
- `crates/prism-core/src/icm/mod.rs` — entry `validate_icm(root: &Path, scope: Scope) -> Vec<IcmViolation>`, `IcmRule` enum, `IcmViolation { rule, file, line, message }`, `Scope::{Project, File(PathBuf)}`
- `crates/prism-core/src/icm/rules/layer_existence.rs` — `L0_EXISTS`, `L1_EXISTS`, `L2_ONE_PER_STAGE`
- `crates/prism-core/src/icm/rules/stage_shape.rs` — `STAGE_FOLDER_SHAPE` (regex + gap detector)
- `crates/prism-core/src/icm/rules/budgets.rs` — `CONTEXT_LINE_BUDGET`
- `crates/prism-core/src/icm/rules/sections.rs` — `STAGE_CONTEXT_SECTIONS`, `INPUTS_TABLE_COLUMNS`
- `crates/prism-core/src/icm/rules/style.rs` — `NO_EM_DASH`
- `crates/prism-cli/src/cmd_lint.rs` — runs `validate_icm(cwd, Scope::Project)`, prints violations, exits non-zero if any
- `crates/prism-cli/src/cmd_fix.rs` — drains `directive_log` entries where `kind="FIX_ICM"`, routes each through `enrich_directory_with_prompt` using the new `build_icm_fix_prompt()`
- `skills/prism-lint/SKILL.md` — user-invocable wrapper around `prism lint`
- `skills/prism-fix/SKILL.md` — user-invocable wrapper around `prism fix`
- `templates/icm-conventions.md.template` — renders to `.claude/rules/icm-conventions.md` at `prism start`; states the 8 rules in prose so Claude reads them via rules loading and self-corrects

**Modified files:**
- `crates/prism-core/src/lib.rs` — `pub mod icm;`
- `crates/prism-core/src/hooks/post_tool_use/mod.rs` — after hash update, if `rel_path` ends `.md`, run `icm::validate_icm(project_root, Scope::File(rel_path))`; per violation, insert `doc_drift` row with `drift_type="IcmViolation"` and description=`{rule_id}: {message}`; dedupe-enqueue one `directive_log` row with `kind="FIX_ICM"`, `target_path=rel_dir`
- `crates/prism-core/src/enrich.rs` — add `build_icm_fix_prompt(dir, violations)` that names the rule ids + cites canonical ICM fix; add `enrich_directory_for_fix(...)` that runs the fix prompt and marks completed only if re-running `validate_icm` returns zero violations for that file
- `crates/prism-db/src/doc_drift.rs` — add `pub const DRIFT_TYPE_ICM: &str = "IcmViolation";`
- `crates/prism-db/src/directive_log.rs` — add `pub const KIND_FIX_ICM: &str = "FIX_ICM";`
- `crates/prism-cli/src/main.rs` — add `Lint` and `Fix` subcommand variants
- `crates/prism-cli/src/cmd_start.rs` — scaffold `templates/icm-conventions.md.template` into `.claude/rules/icm-conventions.md`; register the rendered file in `document_registry` alongside existing rules files
- `crates/prism-cli/src/cmd_status.rs` — also print `icm violations:` count (filter `doc_drift` where `drift_type='IcmViolation' AND resolved=0`)
- `crates/prism-cli/tests/integration.rs` — add `icm_violation_detected_and_queued` test: scaffold project, write bad CONTEXT.md, simulate PostToolUse hook, assert status shows ≥1 IcmViolation + ≥1 FIX_ICM directive

### Existing utilities to reuse (do NOT recreate)

- `crate::hashing::hash_file` — already in use by post_tool_use
- `crate::enrich::enrich_directory_with` + `SystemRunner` / `MockRunner` — already supports prompt injection; just swap the prompt builder
- `prism_db::directive_log::{insert, latest_for_target, mark_completed, mark_abandoned, increment_retry_count}` — ICM fix flow reuses verbatim
- `prism_db::doc_drift::insert` — already takes free-form `drift_type` string
- `prism_db::PrismDb::list_unresolved_drift` — already filters `resolved=0`; new status line just counts by `drift_type`

### Haiku fix prompt shape (sketch)

`build_icm_fix_prompt` emits:

```
The file `{rel_path}` violates the ICM (Interpreted Context Methodology) spec.

Violated rules:
- STAGE_CONTEXT_SECTIONS: missing `Outputs` heading.
- CONTEXT_LINE_BUDGET: 104 lines (limit: 80).

Canonical spec: https://github.com/RinDig/Interpreted-Context-Methdology/blob/main/_core/CONVENTIONS.md

Fix the file in place. Preserve the `<!-- prism:managed -->` marker on line 1. Do not introduce em dashes. Keep CONTEXT.md routing-only (links + short prose), not duplicating structure from CLAUDE.md.

Return the corrected file content only.
```

Directive is marked completed iff `validate_icm` returns zero violations for that file on retry; else retry count increments, abandoned after `EnrichmentConfig.max_retries`.

### Data-model changes (zero new tables)

- `doc_drift.drift_type` — already free-form TEXT. New constant `"IcmViolation"` joins existing `"OutdatedContextFile"`. No schema change.
- `directive_log.kind` — new constant `"FIX_ICM"` joins existing `"ENRICH"`. No schema change.
- `description` column on `doc_drift` carries `{rule_id}: {details}` so `prism lint` can read back structured data from violation history without a join.

### Scaffold changes at `prism start`

- Render `templates/icm-conventions.md.template` → `.claude/rules/icm-conventions.md` via the existing rules scaffold path (`scaffold_rules` in `prism-core/src/templates/scaffold/mod.rs`). Register in `document_registry` the same way other rules files are.
- No new stage folders are scaffolded. ICM stage folders (`01-discovery/` etc.) are a project-author decision; PRISM validates them if they exist but never creates them automatically — that would trip the "heavy v1" line.

### Plugin manifest updates

- `.claude-plugin/plugin.json` — add the two new skills under `skills` so `/prism:lint` and `/prism:fix` are discoverable. Bump version to `0.2.0`. No new capabilities field needed (hook wiring is already declared via `.claude/settings.json` injection at `prism start`).

### What stays OUT of scope (explicit anti-list)

- No goals / decisions / stages-as-state-entities (keep the v2 line)
- No token-budget enforcement at L0 (the spec gives a *target*, not a hard limit; linting L0 size at ~800 tokens produces noise)
- No pre-commit git hook (user rejected)
- No watcher daemon (user rejected)
- No PreToolUse blocking (user chose non-blocking)
- No structural duplication detector between CLAUDE.md↔CONTEXT.md (hard to mechanise without NLP; belongs to Haiku's judgment during `fix`)
- No cross-file one-way-reference graph walk (defer to v2.2)
- No MCP server (hooks already receive tool_input directly)

## Verification

### Unit tests

- One unit-test module per rule file: bad input → expected violation; good input → empty vec.
- `icm::validate_icm` aggregate test: mixed-violation fixture returns all expected rule ids with stable ordering.

### Integration tests (extend `crates/prism-cli/tests/integration.rs`)

1. `icm_lint_on_clean_project_returns_zero` — `prism start`; `prism lint`; assert success + zero violations printed.
2. `icm_violation_detected_and_queued` — `prism start`; write `CONTEXT.md` missing `Outputs` heading + 100 lines long; simulate `PostToolUse` hook; `prism status` reports `icm violations: ≥1` and `pending enrich: ≥1` (target_path has `kind="FIX_ICM"` when queried).
3. `icm_fix_with_mock_runner_marks_completed` — unit-level in `enrich.rs`: inject `MockRunner` returning a corrected file; call `enrich_directory_for_fix`; assert directive marked completed.
4. `icm_fix_bails_without_claude_cli` — tighten existing `enrich_without_claude_cli_bails` to also exercise `prism fix` with empty PATH.

### Manual smoke

1. In a fresh tempdir: `prism start`.
2. `rm CONTEXT.md`; run a hook for `CLAUDE.md`: `prism lint` should report `L1_EXISTS` violation.
3. Create `01-discovery/`, `03-build/` (gap), `02-exploration/CONTEXT.md`: `prism lint` reports `STAGE_FOLDER_SHAPE` for the 01→03 gap.
4. Write a CONTEXT.md containing `—`: `prism lint` reports `NO_EM_DASH`.
5. With `claude` on PATH: `prism fix` drains one FIX_ICM directive; Haiku rewrites the file; re-run `prism lint` shows zero violations; directive marked completed.

### Budget / budget-check

- `cargo test --workspace` — all existing tests still green (116 unit + 4 integration + 3+ new integration + 8 new rule-unit-test modules).
- Non-blank non-comment LOC ≤ 5000 after the add (budget ceiling).
- SQLite table count unchanged at 6.
- Hook count unchanged at 2.
- User skills: 3 → 5 (`/prism:start`, `/prism:stop`, `/prism:status`, `/prism:lint`, `/prism:fix`).
- CLI subcommands: 5 → 7 (`start`, `stop`, `status`, `enrich`, `hook`, `lint`, `fix`).

## Execution order

1. Add `icm` module skeleton (`mod.rs` + empty rules files) — wire into `lib.rs`; build green.
2. Implement + unit-test the 8 rules one file at a time (layer_existence → stage_shape → budgets → sections → style).
3. Wire validator into `post_tool_use`; extend integration test with a seeded violation.
4. Add `cmd_lint` + `Lint` clap variant; smoke against a fixture; add `prism lint` integration test.
5. Add `FIX_ICM` constant + `build_icm_fix_prompt` + `enrich_directory_for_fix`; add `cmd_fix` + `Fix` clap variant.
6. Add `templates/icm-conventions.md.template`; extend `cmd_start` to scaffold + register it.
7. Add the two new skills + plugin.json bump.
8. Extend `cmd_status` to print the `icm violations:` line.
9. Run full test suite + manual smoke. Commit + push. Version bump to `0.2.0`.

## Critical files at a glance

- `crates/prism-core/src/icm/mod.rs` — validator entry
- `crates/prism-core/src/icm/rules/*.rs` — 5 rule files covering 8 rules
- `crates/prism-core/src/hooks/post_tool_use/mod.rs` — hook wiring
- `crates/prism-core/src/enrich.rs` — fix-mode prompt
- `crates/prism-cli/src/{cmd_lint.rs,cmd_fix.rs,main.rs,cmd_start.rs,cmd_status.rs}` — CLI surface
- `crates/prism-db/src/{doc_drift.rs,directive_log.rs}` — string constants only
- `skills/prism-{lint,fix}/SKILL.md` — user-invocable wrappers
- `templates/icm-conventions.md.template` — rules-as-prose for Claude self-correction
- `crates/prism-cli/tests/integration.rs` — end-to-end proof

## Open questions / caveats

- RinDig canonical text says stage separator is hyphen (`01-discovery/`), the companion arXiv paper shows underscore (`01_research/`). Plan goes with hyphen — that's what `CONVENTIONS.md` says, and `CONVENTIONS.md` is self-declared canonical. If a project in the wild uses underscore, the `STAGE_FOLDER_SHAPE` rule will flag it — acceptable trade.
- "One-way reference" (Pattern 3) and "output/ gitkeep-only" rules are load-bearing per the spec but require cross-file graph walks; deferred to v2.2 to keep v2.1 scope tight.
- The `NO_EM_DASH` rule is aggressive; some teams write legitimate prose with em dashes. Make it project-overridable via a simple `.prism/config.json` field `icm.allow_em_dash: true`, defaulting to `false` (spec-accurate).

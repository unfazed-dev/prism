# Stale/dead code audit — prism v2.1

## Context

User asked to find stale or dead code left over from previous version of `prism`. Recent commits show an active pruning campaign (c5bc096, 11df375, 0296065, 29ec619). Goal of this audit: finish the job — identify anything still carrying v1 assumptions that v2 / v2.1 no longer honors.

Authoritative scope source: `docs/plans/lightweight-icm-enforcement.md` (v2.1 plan). Its explicit anti-list says: **"No goals / decisions / stages-as-state-entities (keep the v2 line)"**. v2.1 intentionally added `lint`/`fix` subcommands + skills + ICM module — those are **not** stale.

`CLAUDE.md` at repo root is itself stale relative to the v2.1 plan (lists 5 subcommands, v2.1 has 7; lists 3 skills, v2.1 has 5). Fixing it is part of this plan.

## Findings — confirmed dead

### 1. Template goal/decision blocks (never bound by Rust code)

No Rust code anywhere in `crates/` binds `goals`, `active_goal`, `decisions`, `paused_goals`, or `design_decisions` into a Minijinja context. Confirmed via grep across `crates/` — zero hits outside template files. The `{% if goals %}` / `{% if decisions %}` blocks therefore always render the `else` branch at runtime — dead, unreachable content that advertises features the code doesn't have.

Cut these blocks entirely (not just the `{% if %}` wrappers — the whole section, heading and all):

- [templates/PRISM.md.template:7-26](templates/PRISM.md.template#L7-L26) — `## Active Goal`, `## Goal Stack`, `## Decisions`
- [templates/CONTEXT.md.template:6-15](templates/CONTEXT.md.template#L6-L15) — `## Active Goal`, `## Recent Decisions`
- [templates/CONTEXT.md.template:30-33](templates/CONTEXT.md.template#L30-L33) — `## Paused Goals`
- [templates/dir-CONTEXT.md.template:16-19](templates/dir-CONTEXT.md.template#L16-L19) — `## Related Decisions`
- [templates/refs/architecture.md.template:27-30](templates/refs/architecture.md.template#L27-L30) — `## Key Design Decisions`

### 2. Template prose that lies about nonexistent features

- [templates/CLAUDE.md.template:35](templates/CLAUDE.md.template#L35) — row `| Decisions | .prism/PRISM.md |` in the routing table. No decision tracking exists. Remove the row.
- [templates/rules/general-conventions.md.template:37](templates/rules/general-conventions.md.template#L37) — line "Decisions are auto-logged from conversation". False. Remove the line.
- [templates/rules/prism-document-standard.md.template:10](templates/rules/prism-document-standard.md.template#L10) — phrase `"dynamic state, decisions, activity"`. Drop the word "decisions" from that list; rest is accurate.
- [templates/rules/prism-document-standard.md.template:31](templates/rules/prism-document-standard.md.template#L31) — `classification` enum lists `decision, goal, progress`. Drop those three; the other values (core, architecture, api, guide, reference, template, other) stay.
- [templates/refs/dependencies.md.template:37](templates/refs/dependencies.md.template#L37) — default prose `"require a decision record"`. Soften to `"require review"`.

### 3. `enrich.rs` prompt references decisions

- [crates/prism-core/src/enrich.rs:207](crates/prism-core/src/enrich.rs#L207) — Haiku CONTEXT.md prompt says `"Describe active work, recent changes, open questions, and decisions in progress."` Generic prose (Haiku is free to omit), but the word "decisions" invites behavior the rest of the system doesn't support. Change to `"Describe active work, recent changes, and open questions."`

### 4. Root CLAUDE.md lists stale subcommand + skill counts

- [CLAUDE.md](CLAUDE.md) — the project-root CLAUDE.md bullet "subcommands: start, stop, status, enrich, hook" is pre-v2.1. Update to include `lint`, `hook`, `fix`. (Not code-dead, but a stale spec that contradicts [docs/plans/lightweight-icm-enforcement.md](docs/plans/lightweight-icm-enforcement.md).)

## Findings — NOT stale (ruled out)

- `crates/prism-cli/src/cmd_lint.rs`, `cmd_fix.rs` — intentional v2.1 additions.
- `crates/prism-core/src/icm/**` including `stage_shape.rs` — v2.1 validator, active. "Stage contracts" (v1 concept) ≠ ICM stage-folder naming rule (v2.1).
- `skills/prism-lint/`, `skills/prism-fix/` — v2.1 user-invocable skills.
- All 4 SQLite tables (`document_registry`, `file_hashes`, `doc_drift`, `directive_log`) — each has live readers and writers. (Note: earlier audit claimed 6 tables; current count is 4 after the 29ec619 and 0296065 prunes.)
- All workspace deps in `Cargo.toml`s — prior pruning pass removed the unused ones; remaining set is tight.
- All 16 template files registered in [crates/prism-core/src/templates/registry.rs](crates/prism-core/src/templates/registry.rs) — all referenced.

## Critical files

- [templates/PRISM.md.template](templates/PRISM.md.template)
- [templates/CONTEXT.md.template](templates/CONTEXT.md.template)
- [templates/dir-CONTEXT.md.template](templates/dir-CONTEXT.md.template)
- [templates/CLAUDE.md.template](templates/CLAUDE.md.template)
- [templates/refs/architecture.md.template](templates/refs/architecture.md.template)
- [templates/refs/dependencies.md.template](templates/refs/dependencies.md.template)
- [templates/rules/general-conventions.md.template](templates/rules/general-conventions.md.template)
- [templates/rules/prism-document-standard.md.template](templates/rules/prism-document-standard.md.template)
- [crates/prism-core/src/enrich.rs](crates/prism-core/src/enrich.rs)
- [CLAUDE.md](CLAUDE.md)

## Verification

1. `cargo test --workspace` — all existing tests still pass (tests never asserted on goal/decision sections, so removing them does not break coverage).
2. Snapshot any test that renders a template (grep `minijinja` in tests): re-run and compare output; only the cut sections should disappear.
3. Manual smoke: `prism start` in a tempdir; `cat .prism/PRISM.md` should have no "Active Goal" / "Goal Stack" / "Decisions" headings; `cat CONTEXT.md` should have no "Active Goal" / "Recent Decisions" / "Paused Goals".
4. Grep the workspace for `goal`, `decision`, `design_decisions`, `active_goal`, `paused_goals` — only surviving hits should be in `docs/plans/` (history) or this plan file.
5. `prism enrich` against a fixture dir — inspect the Haiku prompt; confirm the word "decisions" is gone from the CONTEXT.md guidance.

## Commit plan

One commit, conventional format:

```
fix: remove stale v1 goal/decision references from templates and enrich prompt; update CLAUDE.md scope to v2.1 subcommand set
```

## Out of scope for this pass

- Touching `docs/plans/*.md` historical records (they describe v1 accurately for that time).
- Touching `crates/prism-core/src/icm/` — in-scope per v2.1 plan.
- Renaming or restructuring — prune only.

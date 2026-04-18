# PRISM v2 — Fresh Rebuild Plan

## Context

Current PRISM grew from a doc-sync plugin into a multi-layered session state manager: decisions, goals, stage contracts, ICM renaming, pre-commit regeneration, watcher daemon, 4 enrichment spawn paths, 6 crates, 20+ core modules, 5 SQLite tables beyond core needs.

Decision: **fresh rebuild**. New repo/layout built doc-sync-first. Cherry-pick only what serves that mission. Old repo archived, not migrated.

## Target Scope (v2)

**Mission**: keep `CLAUDE.md` / `CONTEXT.md` in sync with the codebase. Nothing else.

**In**:
- Discover managed doc pairs
- Detect drift between docs and source
- Scaffold missing docs from templates
- Enrich placeholder content via Haiku
- 3 skills: `/prism:start`, `/prism:stop`, `/prism:status`

**Out** (never ported):
- Decisions, goals, stage contracts
- ICM numbering / directory renaming
- Pre-commit git hook
- Watcher daemon
- Multiple enrichment spawn paths
- PRISM:ASK rule directives

## Proposed v2 Layout

Target: 2-3 crates, ~5k LOC, 2-3 Claude hooks.

```
prism/
├── crates/
│   ├── prism-cli/          # 3 subcommands: start, stop, status, enrich (internal), hook (dispatcher)
│   ├── prism-core/         # discovery, drift, scaffold, enrich, templates, hashing, config
│   └── prism-db/           # SQLite: document_registry, file_hashes, doc_drift, enrichment_runs, schema_migrations
├── skills/
│   ├── prism-start/
│   ├── prism-stop/
│   └── prism-status/
├── templates/
│   ├── PRISM.md.template
│   ├── CLAUDE.md.template
│   └── CONTEXT.md.template
└── .claude/
    └── settings.json       # 2 hooks: SessionStart, PostToolUse
```

No separate `prism-hooks`, `prism-templates`, `prism-watcher` crates — merge into `prism-core`.

## Cherry-Pick Source List

From current PRISM, port (adapting to v2 shape):

| v1 source | v2 destination | Adapt |
|---|---|---|
| `prism-core/src/drift.rs` | `prism-core/src/drift.rs` | strip stage + decision-drift dimensions; keep content/freshness/hash |
| `prism-core/src/discovery.rs` | `prism-core/src/discovery.rs` | drop ICM numbering awareness |
| `prism-core/src/enrich.rs` | `prism-core/src/enrich.rs` | single entry, no watcher coupling |
| `prism-core/src/hashing.rs` | `prism-core/src/hashing.rs` | straight port |
| `prism-core/src/freshness.rs` | `prism-core/src/freshness.rs` | straight port |
| `prism-core/src/document.rs` | `prism-core/src/document.rs` | drop L0-L4 layer scaffolding if unused |
| `prism-core/src/frontmatter.rs` | `prism-core/src/frontmatter.rs` | straight port |
| `prism-core/src/config.rs` | `prism-core/src/config.rs` | strip WatcherConfig + StageDefinition |
| `prism-db/src/schema.rs` | `prism-db/src/schema.rs` | rewrite with only kept tables |
| `prism-db/src/document_registry.rs` | same | straight port |
| `prism-db/src/file_hashes.rs` | same | straight port |
| `prism-db/src/doc_drift.rs` | same | straight port |
| `prism-hooks/src/session_start/` | `prism-core/src/hooks/session_start.rs` | drop decision/goal/stage branches |
| `prism-hooks/src/post_tool_use/` | `prism-core/src/hooks/post_tool_use.rs` | keep only hash update + drift enqueue |
| `prism-templates/templates/PRISM.md.template` | `templates/PRISM.md.template` | strip decisions/goals sections |
| `prism-templates/templates/CLAUDE.md.template` | same | straight port |
| `prism-templates/templates/CONTEXT.md.template` | same | straight port |
| `skills/prism-{start,stop,status}/SKILL.md` | same | strip references to decisions/goals/stages |

Do NOT port: `decisions.rs`, `goals.rs`, `numbering.rs`, `rename_plan.rs`, `hierarchy.rs`, `hook_migration.rs`, `patterns.rs`, `watcher_control.rs`, `analysis.rs`, `context_mode.rs`, `command_runner.rs` (unless drift needs it), `pre_commit.rs`, `user_prompt_submit.rs`, `cmd_decide.rs`, `cmd_goal.rs`, `cmd_stage.rs`, `cmd_rename.rs`, `cmd_reorder.rs`, `cmd_hierarchy.rs`, `prism-watcher/` crate, `prism-enrich/` skill, `decision-record.md.template`, `stage-*.md.template`, `.claude/rules/06-prism-automation.md`.

## Build Order

1. **Scaffold v2 repo**: new `prism-v2/` directory, workspace `Cargo.toml`, 3 crates, empty `lib.rs` per crate
2. **Port schema**: write fresh `prism-db/src/schema.rs` with only kept tables, no migration chain needed (v2 starts at v1)
3. **Port hashing + discovery + document + frontmatter**: leaf modules, no cross-deps
4. **Port drift + freshness + enrich**: depend on leaf modules
5. **Port config**: strip unused sections
6. **Wire CLI**: `cmd_start`, `cmd_stop`, `cmd_status`, `cmd_hook`, `cmd_enrich`
7. **Wire hooks**: `session_start` + `post_tool_use` only
8. **Port templates + skills**: strip stale sections
9. **Port tests**: only for retained modules; write fresh integration tests for the 3 skills
10. **Cutover**: archive old `prism/` → `prism-legacy/`; rename `prism-v2/` → `prism/`; reinstall plugin

## Verification

- `cargo build --workspace` + `cargo test --workspace` green
- Fresh repo: `/prism:start` scaffolds PRISM.md + CLAUDE.md pairs
- Edit a source file → `/prism:status` shows drift
- `prism enrich` fills placeholders via Haiku
- `/prism:stop` cleanly disables hooks
- Total LOC < 5k (current is ~15-20k+)
- Exactly 2 Claude hooks registered: SessionStart, PostToolUse
- Exactly 3 user-facing skills
- SQLite has ≤ 6 tables

## Critical Files (current, for reference during port)

- [crates/prism-core/src/drift.rs](crates/prism-core/src/drift.rs)
- [crates/prism-core/src/discovery.rs](crates/prism-core/src/discovery.rs)
- [crates/prism-core/src/enrich.rs](crates/prism-core/src/enrich.rs)
- [crates/prism-core/src/document.rs](crates/prism-core/src/document.rs)
- [crates/prism-core/src/config.rs](crates/prism-core/src/config.rs)
- [crates/prism-db/src/schema.rs](crates/prism-db/src/schema.rs)
- [crates/prism-templates/templates/PRISM.md.template](crates/prism-templates/templates/PRISM.md.template)
- [crates/prism-templates/templates/CLAUDE.md.template](crates/prism-templates/templates/CLAUDE.md.template)

## Open Questions (pre-approval)

1. **Repo strategy**: fresh `prism-v2/` sibling folder or new branch `v2-rebuild`?
2. **Archive location**: rename current to `prism-legacy/` on device, or tag + branch in git only?
3. **Ghostwriter manifest**: user invoked `/ghostwriter:verify`. Does v2 rebuild run through ghostwriter TDD flow, or standard manual dev?

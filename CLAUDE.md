# prism

> Claude Code plugin — doc-sync only. Keeps `CLAUDE.md` / `CONTEXT.md` aligned with code.

## Tech Stack

- Rust workspace (3 crates: `prism-cli`, `prism-core`, `prism-db`)
- SQLite (rusqlite, bundled) for drift + registry state
- Minijinja for templates

## Layout

```
crates/
  prism-cli/   # subcommands: start, stop, status, enrich, hook, lint, fix
  prism-core/  # discovery, drift, scaffold, enrich, hooks, config, icm
  prism-db/    # schema, document_registry, file_hashes, doc_drift, directive_log
skills/        # prism-start, prism-stop, prism-status, prism-lint, prism-fix
templates/     # PRISM.md, CLAUDE.md, CONTEXT.md, rules/, refs/
```

## Scope Boundary

Out of scope (do NOT add back): decisions, goals, stage contracts (v1 concept, ≠ v2.1 ICM stage-folder lint), ICM renaming, pre-commit git hook, watcher daemon, PRISM:ASK directives.

v2.1 ICM enforcement (in scope): see [docs/plans/lightweight-icm-enforcement.md](docs/plans/lightweight-icm-enforcement.md).

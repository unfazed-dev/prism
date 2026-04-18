# prism

> Claude Code plugin — doc-sync only. Keeps `CLAUDE.md` / `CONTEXT.md` aligned with code.

## Tech Stack

- Rust workspace (3 crates: `prism-cli`, `prism-core`, `prism-db`)
- SQLite (rusqlite, bundled) for drift + registry state
- Minijinja for templates

## Layout

```
crates/
  prism-cli/   # subcommands: start, stop, status, enrich, hook
  prism-core/  # discovery, drift, scaffold, enrich, hooks, config
  prism-db/    # schema, document_registry, file_hashes, doc_drift
skills/        # prism-start, prism-stop, prism-status
templates/     # PRISM.md, CLAUDE.md, CONTEXT.md
```

## Scope Boundary

Out of scope (do NOT add back): decisions, goals, stage contracts, ICM renaming, pre-commit git hook, watcher daemon, PRISM:ASK directives.

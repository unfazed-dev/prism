# prism

Claude Code plugin that keeps `CLAUDE.md` / `CONTEXT.md` files in sync with the codebase.

## Scope

- Discover managed doc pairs
- Detect drift between docs and source
- Scaffold missing docs from templates
- Enrich placeholder content via Haiku
- Enforce ICM layer / section / style / budget / stage-shape rules
- Auto-fix ICM violations via Haiku

## Commands

- `/prism:start` — enable the plugin in a project
- `/prism:stop` — disable hooks
- `/prism:status` — show sync state
- `/prism:lint` — audit ICM conventions (read-only)
- `/prism:fix` — auto-fix ICM violations via Haiku

Enrichment runs automatically via hooks; no user-facing slash command. The `prism enrich` CLI subcommand exists for internal use.

---
name: prism-start
description: Initialize PRISM in this project — creates .prism/ directory, installs Claude Code hooks, and scaffolds root CLAUDE.md + CONTEXT.md.
user_invocable: true
---

# /prism:start

Run `prism start` in the current project.

## What it does

1. Creates `.prism/` directory and initializes `prism.db` (SQLite).
2. Writes `.claude/settings.json` with two hooks: `SessionStart` and `PostToolUse`.
3. Scaffolds `CLAUDE.md` + `CONTEXT.md` at the project root from templates.

## When to use

- First-time setup in a new project.
- After the plugin is installed and you want automatic documentation sync enabled.

## What NOT to expect

PRISM v2 is doc-sync only. It does **not** track decisions, goals, stages, or inject `[PRISM:ASK]` directives.

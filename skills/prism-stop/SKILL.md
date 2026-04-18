---
name: prism-stop
description: Disable PRISM in this project — removes hook registration from .claude/settings.json. Does not delete .prism/ or prism.db.
user_invocable: true
---

# /prism:stop

Run `prism stop` in the current project.

## What it does

Removes the `hooks` section from `.claude/settings.json`. PRISM stops running on SessionStart and PostToolUse.

The `.prism/` directory and database are left untouched so you can re-enable with `/prism:start` without losing state.

## When to use

- Temporarily disable sync on a misbehaving project.
- Before deleting `.prism/` manually if you want a clean uninstall.

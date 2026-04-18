---
name: prism-fix
description: Drain pending ICM-fix directives from the directive queue. Spawns a Haiku subprocess per flagged file and marks the directive resolved only when re-validation returns zero violations.
user_invocable: true
---

# /prism:fix

Run `prism fix` in the current project.

## What it does

1. Preflight — requires the `claude` CLI on PATH. Bails with an actionable error otherwise.
2. Reads every pending `FIX_ICM` directive from `directive_log`.
3. For each directive, re-validates the target file. If clean already, marks the directive resolved.
4. Otherwise spawns `claude -p --model claude-haiku-4-5` with a prompt naming the violated rule ids and the canonical ICM spec URL.
5. After the subprocess, re-runs the validator. If violations are gone the directive is marked resolved; otherwise it's deferred (retry count increments) or abandoned once `enrichment.max_retries` is hit.

## When to use

- After `/prism:lint` surfaces violations.
- Before committing — run once to auto-repair then re-run `/prism:lint` to confirm.

## What it will NOT do

- Block your edits — PRISM's enforcement model is warn-and-fix, never deny.
- Fix non-ICM drift (that's what `prism enrich` does for source-vs-doc drift).
- Write to files PRISM does not consider managed markdown.

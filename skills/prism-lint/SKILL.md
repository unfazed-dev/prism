---
name: prism-lint
description: Audit the project for ICM (Interpreted Context Methodology) spec violations — read-only. Reports layer-existence, stage-folder shape, CONTEXT.md line budget, required headings, Inputs-table schema, and em-dash rules.
user_invocable: true
---

# /prism:lint

Run `prism lint` in the current project. This is a read-only audit; no database writes, no file changes.

## Output

- `ICM: clean — 0 violations.` on success.
- Otherwise one line per violation: `[RULE_ID] path:line — message`.
- Exit code is non-zero when violations are found so CI / hooks can branch on it.

## Rules enforced

- `L0_EXISTS`, `L1_EXISTS`, `L2_ONE_PER_STAGE` — layer presence.
- `STAGE_FOLDER_SHAPE` — `^\d{2}-[a-z0-9-]+$`, sequential from `01`, no gaps.
- `CONTEXT_LINE_BUDGET` — CONTEXT.md ≤ 80 lines, reference files ≤ 200.
- `STAGE_CONTEXT_SECTIONS` — stage CONTEXT.md needs `## Inputs`, `## Process`, `## Outputs`.
- `INPUTS_TABLE_COLUMNS` — `Source | File/Location | Section/Scope | Why`.
- `NO_EM_DASH` — em dashes (U+2014) are banned.

Canonical spec: https://github.com/RinDig/Interpreted-Context-Methdology

## When to use

- Before committing markdown edits.
- To inspect state that the PostToolUse hook has already recorded as drift.
- Pair with `/prism:fix` to auto-repair flagged files via Haiku.

---
name: prism-status
description: Show PRISM sync state — managed document count, unresolved drift, pending enrichment queue size.
user_invocable: true
---

# /prism:status

Run `prism status` in the current project.

## Output

- **managed docs** — number of CLAUDE.md / CONTEXT.md / ref docs tracked in `document_registry`.
- **unresolved drift** — count of drift events detected but not yet resolved.
- **pending enrich** — count of `directive_log` rows waiting for Haiku enrichment.

## When to use

- Quick health check mid-session.
- Before committing code to verify docs aren't drifting.
- To see if enrichment queue is draining.

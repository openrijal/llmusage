---
id: P1-dedup-null-session-ids
priority: P1
status: completed
file: src/db/schema.rs:26
---

# Make the dedup key handle NULL session IDs

## Problem

`INSERT OR IGNORE` is not idempotent for records with `NULL` session_id because
SQLite treats `NULL` values as distinct inside a `UNIQUE` index. The Anthropic,
OpenAI, and Ollama collectors always write `session_id: None`, so rerunning
`llmusage sync` inserts duplicate rows and inflates every summary.

## Fix

Replace the `UNIQUE` index with a composite unique index that uses `COALESCE` to
map `NULL` session IDs to a sentinel value (`''`), and switch `INSERT OR IGNORE`
to an upsert-style `INSERT ... ON CONFLICT DO NOTHING` using a generated-column
approach. The simplest correct fix: use `COALESCE(session_id, '')` in the unique
index expression.

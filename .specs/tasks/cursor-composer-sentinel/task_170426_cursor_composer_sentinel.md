# Task Record - Cursor Composer Sentinel Records

## Objective

Make `llmusage sync` surface every Cursor conversation found on disk — even ones where Cursor never recorded token counts — so `daily`/`summary` stats stop silently hiding Cursor activity for users on Cursor 3.x.

A user reported `llmusage sync` showed `cursor... ok (0 records)` on Cursor 3.1.15 Linux, and `no usage` in daily stats, despite actively using Cursor. Investigation proved that Cursor 3.x does not persist token counts to local disk for most requests — the canonical usage data lives on Cursor's dashboard API (accessible only via the `WorkosCursorSessionToken` cookie or Enterprise Analytics API). So the collector cannot recover tokens locally, but it can still recover conversation metadata: composer id, model, creation time, bubble counts.

## Scope

- [x] Fact-check whether token counts exist anywhere under `~/.config/Cursor` or `~/.cursor` on Cursor 3.1.15
- [x] Rewrite `CursorCollector::collect` to emit **one record per composer** (conversation) rather than per bubble
- [x] Aggregate bubble tokens up to the composer level for older Cursor versions that still persist tokens
- [x] Emit composer records even when token sum is 0 — with `model`, `session_id`, `recorded_at` all valid so dedup across syncs works
- [x] Preserve real model attribution (`gpt-5.2-codex`, `claude-4.5-sonnet-thinking`, etc.) when Cursor names a specific model
- [x] Normalize Cursor's internal `"default"` modelName to `"cursor-default"` so reports don't accidentally price a generic `default` string
- [x] Fix the dedup bug introduced while implementing the above: skip composers that have no timestamp we can trust, to keep `recorded_at` stable across syncs
- [x] Replace the short-lived zero-records hint (PR #72) with a zero-tokens-all-records hint that fires when every emitted record has 0 tokens
- [x] Unit tests for aggregation, sentinel emission, mixed composers, empty composers, and missing table
- [x] Specs
- [x] Supersede PR #72 (close, referencing this PR)

## Decisions

- **Per-composer granularity.** One record per Cursor conversation, not per bubble. Reasons: bubbles are an internal concept, composer = user-facing conversation, and per-composer lines up naturally with how `daily`/`summary` work. Cost tradeoff: existing per-bubble records in users' SQLite DBs stay put (different `session_id`), so both shapes coexist temporarily. Over time only composer records accumulate.
- **Skip composers with no timestamp.** `recorded_at` must be stable across syncs or the `(provider, model, recorded_at, session_id, …)` unique index can't dedup. Falling back to `Utc::now()` breaks dedup (verified — caused 30 duplicate rows per sync on my real DB). Better to drop the few undateable composers than pollute the DB.
- **Stay local, no remote API (yet).** The only way to recover accurate tokens + cost for Cursor 3.x is to hit `cursor.com`'s dashboard API with a `WorkosCursorSessionToken` cookie. That requires the user to paste a session token, which is out of scope for this change and is a different shape of feature (explicit-config collector, like anthropic/openai API keys). Tracked separately for a future iteration.
- **`"default"` → `"cursor-default"`.** Keeps the model column semantically meaningful and prevents collisions with any upstream model literally called "default".
- **Hint rewording.** The old "found N conversations but none recorded token usage" message (PR #72) was right for the plan-gate scenario but missed the broader 3.x reality. New message references both the dashboard-only reality and the legacy plan-gate scenario.

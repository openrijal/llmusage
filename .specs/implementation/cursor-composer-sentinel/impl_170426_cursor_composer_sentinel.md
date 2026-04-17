# Implementation Details - Cursor Composer Sentinel Records

## Summary

`src/collectors/cursor.rs` is rewritten to aggregate token usage at the composer (conversation) level instead of emitting one record per bubble. Composers with at least one bubble always produce a record, even when the summed tokens are zero — giving `llmusage daily`/`summary` visibility into Cursor activity on Cursor 3.x, where individual bubbles no longer carry token counts on disk.

## Fact-Check Summary

Before rewriting, we verified that Cursor 3.1.15 does not persist token counts anywhere under `~/.config/Cursor` or `~/.cursor`:

| Location | Has tokens? | Evidence |
|---|---|---|
| `state.vscdb` `cursorDiskKV` `bubbleId:*.tokenCount` | Legacy only | 75/2411 nonzero on an older Cursor install, 0/12 on 3.1.15 |
| `~/.cursor/projects/*/agent-transcripts/*.jsonl` (CLI) | No | grep for any `*token*` field returned empty |
| `~/.cursor/chats/*/*/store.db` blobs (IDE) | No | grep across all blobs for `*token*` or `"usage":` returned empty |
| `~/.cursor/ai-tracking/ai-code-tracking.db` | No | schema tracks line counts, not tokens |
| `~/.cursor/*.json` config/state files | No | only match was a docs example in a skill markdown |

Confirmed external corroboration: third-party Cursor usage trackers (VS Code "Cursor Usage & Cost Tracker", cursortokens.vercel.app) all use the dashboard API with a `WorkosCursorSessionToken` cookie — none read local files. Cursor's official Analytics API is Enterprise-only.

## File Changes

- `src/collectors/cursor.rs` — rewritten; see structure below.
- `.specs/tasks/cursor-composer-sentinel/task_170426_cursor_composer_sentinel.md`
- `.specs/implementation/cursor-composer-sentinel/impl_170426_cursor_composer_sentinel.md`
- `.specs/validation/cursor-composer-sentinel/validate_170426_cursor_composer_sentinel.md`

## Collector Structure

`CursorCollector::collect` now:

1. Copies the live `state.vscdb` to a per-call unique temp file (pid + thread id + counter, to avoid parallel-test races).
2. Calls `load_composers(&conn)` which reads all `composerData:*` rows into `HashMap<String, ComposerInfo>` carrying `model`, `created_at_ms`, `last_updated_at_ms`, `name`. Timestamps can be either Unix ms integers (3.x) or RFC 3339 strings (older) — `parse_timestamp_ms` handles both.
3. Calls `aggregate_bubbles(&conn)` which walks all `bubbleId:composer_id:bubble_id` rows, grouping per composer into `ComposerAggregate { input_tokens, output_tokens, bubble_count, nonzero_bubble_count, latest_ms }`. Zero-token bubbles still count toward `bubble_count` but don't contribute to sums.
4. For each composer with at least one bubble:
   - Resolves `model` via `composerData.modelConfig.modelName`, mapping `None`/`"default"` → `"cursor-default"`.
   - Resolves `recorded_at` from `composer.createdAt` → `composer.lastUpdatedAt` → bubble `latest_ms`; **skips the composer entirely** if none of those exist (stable-recorded_at is required for dedup).
   - Resolves `cost_usd` via `infer_priced_provider()` → `costs::calculate_cost()`; returns `None` for `cursor-default` (by design — we can't price Auto without the dashboard API).
   - Emits one `UsageRecord` with `session_id = composer_id`, `metadata = {composer_id, name, bubble_count, nonzero_token_bubble_count}`.
5. Sorts records by `recorded_at` ascending.
6. Prints a stderr hint when every emitted record has zero tokens, explaining 3.x's dashboard-only model and mentioning the legacy plan-gate case.

## Unique Index Compatibility

The existing `idx_dedup` unique index is `(provider, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, recorded_at, COALESCE(session_id, ''))`. Composer records dedup correctly because `session_id = composer_id` and `recorded_at` is derived from an immutable property of the composer (its createdAt / lastUpdatedAt / newest bubble timing).

Records that would have non-deterministic `recorded_at` (no composer timestamp and no bubble timing) are dropped rather than emitted with `Utc::now()` — the prior approach inserted 30 duplicate rows per sync, observed on a real DB and fixed before commit.

## Migration Note

Users who previously synced Cursor with the per-bubble collector will have old rows in their DB with different `session_id`s and different `model` values ("default" vs "cursor-default"). Those rows stay put; new syncs add composer-level rows alongside them. Over time only composer records accumulate. A future cleanup command could delete the per-bubble rows, but it isn't required for correctness.

## Out of Scope

- Hitting the Cursor dashboard API via `WorkosCursorSessionToken`. That's a separate, opt-in collector (similar to the Anthropic/OpenAI API-key collectors) and will be a follow-up PR.
- Parsing CLI agent transcripts (`~/.cursor/projects/*/agent-transcripts/*.jsonl`) for token data — proven to contain none.
- Migrating or deleting legacy per-bubble rows from users' DBs.

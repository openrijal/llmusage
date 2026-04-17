# Validation Record - Cursor Composer Sentinel Records

## Build / Lint

### cargo build
- **Status**: PASS

### cargo fmt --check
- **Status**: PASS

### cargo clippy --all-targets -- -D warnings
- **Status**: PASS

## Unit Tests

### cargo test cursor
- **Status**: PASS — 6/6

Test matrix:

| Test | What it proves |
|---|---|
| `cursor_state_path_is_relative_to_platform_config_dir` | Path resolution on macOS and Linux config dirs unchanged |
| `aggregates_tokens_across_bubbles_in_a_composer` | Two bubbles with 1000/200 and 500/100 tokens produce one composer record with 1500/300 and metadata listing 2 bubbles, 2 nonzero |
| `emits_sentinel_record_when_all_bubbles_have_zero_tokens` | Composer with three zero-token bubbles still produces one record, `model = cursor-default`, `cost_usd = None`, `nonzero_token_bubble_count = 0` |
| `skips_composer_with_no_bubbles` | Empty draft composers do not show up as records |
| `handles_multiple_composers_mixed_tokens` | Two composers — one `claude-sonnet-4-6` with real tokens, one `default` with zero — both produce records; sorted by `recorded_at` ascending |
| `returns_empty_when_no_cursor_disk_kv_table` | Missing `cursorDiskKV` table returns `Ok(vec![])` and does not panic |

## End-to-End Smoke Test (macOS, real state.vscdb)

- `cargo build --release` clean.
- First `llmusage sync --provider cursor`: `ok (30 records)`.
- `llmusage detail --provider cursor --limit 5` shows real per-composer aggregates — `gpt-5.2-codex` 43,023 / 11,086 / $0.230, `gpt-5.1` 33,553 / 13,811 / $0.180, plus several `default` conversations with tokens and no cost.
- Second `llmusage sync --provider cursor` immediately after: `ok (30 records)` — but DB still shows 30 cursor rows, confirming dedup works (the "30 records" is what the collector returned; `INSERT OR IGNORE` drops the duplicates).
- `SELECT COUNT(*) FROM usage_records WHERE provider='cursor';` = 30 after two consecutive syncs.

## Regression Check During Development

- Initial implementation fell back to `Utc::now()` when no composer timestamp was available; two consecutive syncs produced 60 rows (30 duplicated). Fixed by skipping composers without a stable timestamp before commit. Post-fix the count stabilizes at 30 across repeated syncs, confirmed by direct `sqlite3` inspection.

## Platform Coverage — To Verify Manually

- Run on the Linux Cursor 3.1.15 machine that originally hit the bug. Expected: `ok (N records)` for some N ≥ number of conversations with bubbles, all with `tokens = 0` and `cost = -`, and a stderr hint explaining Cursor 3.x's dashboard-only usage model. `daily` / `summary` should now show Cursor activity instead of the "no usage" message.
- Run on a machine where the old per-bubble cursor records already exist in the DB. Expected: both old per-bubble rows and new per-composer rows coexist; new syncs only add composer rows.

## Validation Notes

- The hint fires only when *every* emitted record has zero tokens. A mixed session (some composers with real tokens, some zero) produces records without the warning.
- Record metadata carries `bubble_count` and `nonzero_token_bubble_count` so users who want to inspect why a composer has zero tokens can drill in via `llmusage detail`.
- No change to the auto-detection logic in `src/collectors/mod.rs`. Cursor is still registered whenever its `state.vscdb` exists.

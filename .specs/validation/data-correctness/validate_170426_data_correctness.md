# Validation Record - Data Correctness

## Automated Checks

- `cargo fmt --all --check` — PASS
- `cargo build` — PASS
- `cargo clippy --all-targets -- -D warnings` — PASS (fixed a `clippy::cloned_ref_to_slice_refs` lint in a new test by using `std::slice::from_ref`)
- `cargo test` — PASS (17 pre-existing + 8 new = 25 tests)

## New Tests (`src/display.rs::csv_and_filter_tests`)

| Test | Asserts |
|------|---------|
| `csv_escape_plain_field_not_quoted` | `gpt-4o` → `gpt-4o` (no quoting) |
| `csv_escape_comma_gets_quoted` | `foo,bar` → `"foo,bar"` |
| `csv_escape_quotes_are_doubled` | `he said "hi"` → `"he said ""hi"""` |
| `csv_escape_newline_gets_quoted` | `\n` and `\r\n` both trigger quoting |
| `to_csv_escapes_tricky_provider_and_model` | full-row integration: `"acme,co","weird""model",...` |
| `filter_daily_rows_recomputes_totals_from_visible_entries` | `total_input` drops from 100+0 to 100 when zero entry is removed |
| `filter_daily_rows_preserves_first_seen_model_order` | `[zeta, alpha, zeta]` → `[zeta, alpha]` |
| `filter_daily_rows_show_all_returns_unchanged` | `show_all=true` short-circuits |

## Manual Smoke — Recommended Post-Merge

- **#31**: inject a record with `model = "weird\"model"` and run `llmusage export --format csv`. Confirm the output round-trips through `python -c "import csv,sys; list(csv.reader(sys.stdin))"`.
- **#32**: time `llmusage sync` on a large Claude Code history before/after this PR. Expected: several × speedup on sync-heavy runs. Baseline metric not yet captured in CI; user can verify locally.
- **#48**: on a day with a mix of zero-token and non-zero-token model entries, run `llmusage daily` (no `--all`) and confirm the totals row equals the arithmetic sum of the displayed rows. Also run `llmusage export --format json` twice and diff — model order should be stable.

## Regression Risk

- **`get_model_pricing` per-call allocation profile.** Previously consumed an owned `HashMap`; now clones `model_key` (String) per surviving entry. Net allocations ≈ O(models × 1 String) instead of O(1 HashMap clone). Not a hot path — called from display commands, not sync.
- **`PRICING_CACHE` staleness if `update_pricing_cache` is called in the same process after `calculate_cost`.** The cache will not refresh. In the current CLI flow, each command is its own process, so this is a non-issue. If a long-running process is ever introduced, a `refresh()` entry point would be needed.
- **CSV numeric columns still unquoted.** Correct per RFC 4180 (no hostile bytes in `i64` / `f64` Display), but worth noting if a future column stores formatted strings.

## Rollback

- #31 and #48: revert the `src/display.rs` hunks. Independent, no shared state.
- #32: reverting `src/costs.rs` restores per-call disk reads. `calculate_cost`'s call site is unchanged, so no cross-file revert needed.

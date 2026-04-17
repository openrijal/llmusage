# Validation Record - Weekly ISO + SIGPIPE

## Automated Checks

- `cargo fmt --all --check` — PASS
- `cargo build --release` — PASS
- `cargo clippy --all-targets -- -D warnings` — PASS
- `cargo test` — PASS (37 pre-existing + 4 new = 41 tests)

## New Tests (`src/db/queries.rs::iso_week_tests`)

| Test | Asserts |
|------|---------|
| `groups_days_within_same_iso_week` | Mon/Wed/Sun of 2026-W16 merge into one row with summed totals |
| `splits_across_iso_weeks_at_monday_boundary` | Sun 2026-04-12 (W15) and Mon 2026-04-13 (W16) go to separate rows |
| `january_first_2027_is_w53_of_2026` | Year-boundary rollover — ISO year differs from calendar year |
| `merges_same_model_across_days_into_one_entry` | Two entries for the same `(provider, model)` across days collapse to one |

## Manual Smoke

Run locally against the real DB (v0.1.3 + merged bug-bundles + this fix):

```text
$ llmusage weekly
┌──────────┬─────────────────────┬───────────┬───────────┬────────────┐
│ Date     │ Models              │     Input │    Output │ Cost (USD) │
...
│ 2026 W15 │ claude_code         │    14,340 │   284,214 │    $175.40 │
│ 2026 W16 │ claude_code         │     3,316 │   989,525 │    $366.85 │
```

- **Before this PR**: `Error: Invalid column type Null at index: 0, name: period`. After: renders cleanly.
- **Pipe tests**:
  - `llmusage export --format csv | head` → exit 0, no panic.
  - `llmusage export --format json | head` → exit 0, no panic.
  - `llmusage export --format xml` → `Error: Unknown export format: 'xml'. Supported: csv, json`, exit 1.

## Regression Risk

- **Weekly cost aggregation drift.** The totals now come from Rust-side summation instead of SQL `SUM`. Both sum the same floating-point values; ordering differences could theoretically change the last ULP. Acceptable for cost display.
- **`SIGPIPE` default behavior in all commands.** Any command writing to stdout/stderr now terminates with signal death instead of panic on `EPIPE`. This is the intended UX for pipelines (`| head`, `| grep`, etc.) and matches every other Unix tool. Scripts relying on the prior panic behavior would need to catch exit status 141 instead of 101 — unlikely to exist.
- **`libc` dependency.** Small, stable, widely audited, already a transitive dep of `tokio`.

## Rollback

- `src/db/queries.rs`: reverting `query_weekly` to `strftime('%G-W%V', ...)` restores the NULL-crash regression — do not roll back without an alternate fix.
- `src/main.rs` + `Cargo.toml` libc dep: reverting restores the EPIPE panic. Independent rollback is safe if the panic is preferred for some reason.

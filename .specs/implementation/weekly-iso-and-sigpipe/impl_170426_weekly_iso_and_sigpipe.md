# Implementation Details - Weekly ISO + SIGPIPE

## Summary

Three files touched: `Cargo.toml` (+ `libc` dep), `src/db/queries.rs` (ISO-week rebucketing), `src/main.rs` (SIGPIPE reset).

## `Cargo.toml`

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

New target-gated dep — not pulled in on Windows builds.

## `src/db/queries.rs`

### `query_weekly`

```rust
pub fn query_weekly(conn: &Connection, weeks: u32, provider: Option<&str>)
    -> Result<Vec<DailyRow>>
{
    let daily = query_grouped(
        conn,
        "DATE(recorded_at)",
        &format!("-{} days", weeks * 7),
        provider,
    )?;
    Ok(rebucket_daily_by_iso_week(daily))
}
```

`DATE(recorded_at)` is portable on every SQLite version.

### `rebucket_daily_by_iso_week`

```rust
fn rebucket_daily_by_iso_week(daily: Vec<DailyRow>) -> Vec<DailyRow> {
    // BTreeMap<week_label, BTreeMap<(provider, model), ModelEntry>>
    // Iterate daily rows; for each, compute ISO week via chrono and merge
    // each ModelEntry into the (provider, model) accumulator under that week.
    // Assemble back into DailyRow list, preserving week-order via BTreeMap.
}
```

Behavior:

- Group key is `format!("{}-W{:02}", iw.year(), iw.week())`, e.g. `2026-W16`.
- Across days in the same ISO week, `ModelEntry`s with the same `(provider, model)` merge — no duplicate rows.
- `DailyRow.total_*` is recomputed from the merged entries.
- Unparseable dates are skipped defensively (`DATE(recorded_at)` always yields `YYYY-MM-DD` in practice).

## `src/main.rs`

```rust
#[cfg(unix)]
fn reset_sigpipe() {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

#[tokio::main]
async fn main() -> Result<()> {
    reset_sigpipe();
    // ... rest unchanged
}
```

After this, writing to a closed pipe terminates the process with the conventional signal-death exit status rather than panicking inside `println!`.

## No Public API Changes

`query_weekly` signature unchanged; return shape unchanged (`Vec<DailyRow>` with week labels in the `date` field). `main` unchanged aside from the one pre-call.

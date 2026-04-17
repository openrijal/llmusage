# Implementation Details - Quick-Bug Bundle

## Summary

Four independent fixes in one PR. No shared helpers, no cross-file coupling.

## Changes

### `src/config.rs` — #33

```rust
let dir = cfg
    .config_path
    .parent()
    .ok_or_else(|| anyhow::anyhow!("config_path has no parent directory"))?;
```

Replaces `.parent().unwrap()`. `save_config` now surfaces a clear error when called with a `Config` whose `config_path` has not been populated (e.g., deserialized from an external source without going through `load_config`).

### `src/display.rs` — #30

```rust
fn format_tokens_comma(n: i64) -> String {
    let negative = n < 0;
    let magnitude = (n as i128).unsigned_abs();
    let digits = magnitude.to_string();
    // ...iterate digit bytes, push ',' every 3 from the right...
    if negative { result.push('-') first; }
}
```

The sign is no longer in the byte stream that drives comma placement. `i128` handles `i64::MIN` without overflow.

Unit tests added (at the end of the file so we don't trip `clippy::items-after-test-module`):

- `positive_numbers_get_commas`
- `negative_numbers_get_commas_and_sign`
- `i64_min_does_not_panic`

### `src/db/queries.rs` — #34

```rust
"strftime('%G-W%V', recorded_at)"
```

Was `strftime('%Y-W%W', ...)`. `%G` = ISO week-numbering year, `%V` = ISO week number (01..53). SQLite ≥ 3.44 required; the bundled build in `rusqlite` satisfies this.

### `src/main.rs` — #36

```rust
let content = match format {
    "json" => display::to_json(&rows)?,
    "csv" => display::to_csv(&rows)?,
    other => anyhow::bail!("Unknown export format: '{}'. Supported: csv, json", other),
};
```

The catch-all `_ => to_csv(...)` was removed. Unknown formats fail fast with a helpful message listing the accepted values.

## No Public API Changes

All four fixes are internal. No signature changes, no new public functions, no crate-dependency churn.

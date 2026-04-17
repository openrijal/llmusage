# Implementation Details - Data Correctness

## Summary

Two files: `src/display.rs` (CSV escape + daily-totals recompute) and `src/costs.rs` (pricing cache). ~174 insertions / ~19 deletions. No dependency changes.

## `src/display.rs`

### CSV escaping — #31

```rust
fn csv_escape(field: &str) -> String {
    let needs_quoting = field
        .bytes()
        .any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r'));
    if !needs_quoting { return field.to_string(); }
    let escaped = field.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}
```

`to_csv` now wraps `r.provider`, `r.model`, and `r.recorded_at` in `csv_escape`. Numeric columns (`input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `cost_usd`) are emitted via `{}` unchanged — they can't contain CSV-hostile bytes.

### `filter_daily_rows` totals & ordering — #48

```rust
// Preserve first-seen ordering of model names.
let mut seen = HashSet::new();
let mut models: Vec<String> = Vec::new();
for entry in &filtered {
    if seen.insert(entry.model.clone()) {
        models.push(entry.model.clone());
    }
}

// Recompute totals from visible entries.
let total_input:  i64 = filtered.iter().map(|e| e.input_tokens).sum();
let total_output: i64 = filtered.iter().map(|e| e.output_tokens).sum();
let total_cost:   f64 = filtered.iter().map(|e| e.cost).sum();
```

The `show_all: true` branch still early-returns `rows.to_vec()`, so unfiltered totals are untouched.

## `src/costs.rs`

### Process-lifetime pricing cache — #32

```rust
use std::sync::OnceLock;

static PRICING_CACHE: OnceLock<Option<HashMap<String, LiteLLMEntry>>> = OnceLock::new();

fn load_cached_pricing() -> Option<&'static HashMap<String, LiteLLMEntry>> {
    PRICING_CACHE
        .get_or_init(|| {
            let path = cache_path();
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        })
        .as_ref()
}
```

Return type changed from `Option<HashMap<...>>` to `Option<&'static HashMap<...>>`.

### Caller updates

- `get_model_pricing` now iterates with `.iter()` and clones `model_key` per surviving entry. Previous `into_iter()` consumed an owned map; no longer possible with a borrowed static.
- `calculate_cost` uses `.get(model)` and `.get(&prefixed)` on the reference — source unchanged (already compatible with `&HashMap`).

## No Public API Changes

`to_csv`, `filter_daily_rows`, `calculate_cost`, and `get_model_pricing` keep their existing signatures. The internal `load_cached_pricing` is module-private.

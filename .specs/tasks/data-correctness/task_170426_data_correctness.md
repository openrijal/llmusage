# Task Record - Data Correctness: CSV Escaping, Pricing Cache, Daily Totals

## Objective

Close three correctness/performance issues that share a "wrong data out" flavor:

- **#31** — `display::to_csv` manually concatenates fields with commas; no RFC 4180 quoting. Provider or model names containing commas/quotes/newlines produce malformed CSV that downstream parsers (Excel, pandas) silently misinterpret.
- **#32** — `costs::load_cached_pricing` reads and parses the ~2MB LiteLLM JSON on every call. `calculate_cost` is invoked once per usage record, so `llmusage sync` on a large Claude Code history re-parses the file thousands of times. This is the dominant sync bottleneck.
- **#48** — `display::filter_daily_rows` preserves original `total_input`/`total_output`/`total_cost` after filtering zero-token entries, so the totals row can disagree with the sum of visible entries. The `models` list is also rebuilt via `HashSet`, which randomizes order and breaks JSON-export determinism.

Bundle them: all three are in the display + costs modules and touch the "what the user sees / exports" contract. Fixing them together keeps the reviewer context coherent.

## Scope

- [x] **#31** — introduce `csv_escape(&str) -> String` that quotes fields containing `,`, `"`, `\n`, or `\r`, and doubles embedded quotes. Apply it to `provider`, `model`, `recorded_at`. Numeric columns stay unquoted.
- [x] **#32** — cache the parsed `HashMap<String, LiteLLMEntry>` in a `OnceLock`. `load_cached_pricing` returns `Option<&'static HashMap<...>>`. Callers updated to work with references.
- [x] **#48** — recompute `total_input`/`total_output`/`total_cost` from the post-filter entries. Build `models` via a `HashSet`-backed insertion that preserves first-seen order.
- [x] Unit tests for each fix.

## Decisions

- **Inline CSV escaping over adding the `csv` crate.** Three-column escaping with ~10 lines of code doesn't justify a new dependency. If CSV needs grow (streaming writer, custom delimiters, BOM), revisit.
- **`OnceLock` over `LazyLock` or `Mutex<Option<...>>`.** `OnceLock` is std, initialized on first use, zero overhead after init, and doesn't require the `lazy_static` or `once_cell` crates. `update_pricing_cache` still writes the file; the next CLI invocation picks it up. In a single CLI invocation both paths aren't typically called, so staleness is a non-issue.
- **Borrow the static map, don't clone.** `get_model_pricing` now iterates `entries.iter()` and clones only the `model_key` (String) per-entry. Avoids a full `HashMap` clone per call.
- **Recompute totals, don't change the struct shape.** `DailyRow` already carries totals; recomputing from `filtered` keeps the contract (`totals == sum(model_entries)`) without touching upstream query code.
- **Preserve first-seen model order.** `HashSet` is fine for dedup; a small hand-rolled "seen set + ordered vec" avoids pulling in `indexmap` for one call site.

## Out of Scope

- Not switching to a streaming CSV writer — `to_csv` builds a `String` and callers write it whole. Not a memory bottleneck at realistic dataset sizes.
- No invalidation hook on `PRICING_CACHE` after `update_pricing_cache` within the same process — in practice `update_pricing_cache` is its own CLI command and runs in a fresh process.
- No change to the `--all` code path that forces unfiltered output (early-returns on `show_all`).

---
id: P2-alignment-and-ux
priority: P2
status: completed
file: src/display.rs:95
---

# Table alignment and UX improvements

## Problem

The daily/weekly/monthly table display had several UX issues:
1. Hardcoded column widths didn't adapt to actual content
2. Date column was too narrow for full YYYY-MM-DD
3. Weekly mode had significant alignment issues
4. Models listed flat without provider context or per-model values
5. No visual separation between time periods
6. No per-model token/cost breakdown — only period-level aggregates

## Fix

1. Dynamic column widths computed by scanning all row data before rendering
2. Single-line date format (no multi-line split)
3. `--all` flag to control visibility of zero-value models
4. Solid separator lines between time periods
5. Models grouped by provider with:
   - Provider aggregate row (magenta) showing total input/output/cost
   - Per-model rows (dimmed) showing individual token/cost breakdown
   - Dotted separators (·) between models within a provider
6. New `ModelEntry` struct with per-model cost, `DailyRow.model_entries` field
7. Query updated to GROUP BY period, provider, model for per-model data

## Codex Review Fixes (2026-04-15)

Three issues identified by Codex review, all resolved:

### P2: Table border alignment regression
- **Issue**: Data rows used inconsistent padding (missing trailing space on Date/Models cells), causing vertical separators to drift left of the border lines.
- **Root cause**: Format string `"│ {:<cw$}│ {}│ ..."` lacked the trailing space before `│` that the border widths (`col_* + 2`) accounted for.
- **Fix**: Changed all data/header/totals/dotted format strings to use `"│ {:<cw$} │ {} │ ..."` with consistent 1-space padding on both sides of every cell.

### P2: Zero-token filter discarding billable rows
- **Issue**: Default filter dropped models with `input_tokens == 0 && output_tokens == 0`, even when `cost > 0` (e.g., cache read/write billing). This caused visible cost totals with no corresponding model rows.
- **Fix**: Added `&& entry.cost == 0.0` to the filter condition in `build_period_rows`. Rows with non-zero cost are now always shown.

### P3: `--all` flag ignored in JSON output
- **Issue**: `show_all` was only passed to `display::print_daily` (text mode). JSON mode serialized unfiltered query results, making `--all` a no-op for `--json` consumers.
- **Fix**: Added `display::filter_daily_rows()` public function. All three commands (`cmd_daily`, `cmd_weekly`, `cmd_monthly`) now call it before JSON serialization, ensuring consistent filtering across output formats.

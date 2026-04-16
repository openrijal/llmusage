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

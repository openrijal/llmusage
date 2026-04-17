# Task Record - Fallback Pricing: Deterministic Matching + Cache Token Costs

## Objective

Fix two related bugs in `costs::calculate_cost_fallback` — the hardcoded rate table used when the LiteLLM cache is unavailable:

- **#55** — pattern matching uses `contains()` in an order-sensitive chain. `o1` matches any model string containing `"o1"` (e.g., `gpt-4o1-preview`), and `o3` has no entry at all despite being an active family.
- **#54** — the function only accepts `input_tokens` and `output_tokens`, ignoring `cache_read_tokens` and `cache_write_tokens`. For Anthropic models with heavy prompt caching, the fallback underestimates cost significantly.

Bundle them: both touch the same ~30 lines and share the same design pressure (explicit model disambiguation).

## Scope

- [x] Rewrite the match into a `fallback_rates(model) -> Option<FallbackRates>` helper with explicit ordering (longest/most-specific prefix first).
- [x] Add word-boundary matching (`has_reasoning_token`) for `o1`/`o3`/`o4-mini` so they cannot collide with `gpt-4o1-*` style names.
- [x] Add entries for `o3` (non-mini) and `o4-mini`.
- [x] Extend `calculate_cost_fallback` signature with `cache_read_tokens` and `cache_write_tokens`.
- [x] Encode Anthropic cache rates (opus `$1.50 / $18.75`, sonnet `$0.30 / $3.75`, haiku `$0.08 / $1.00` per MTok).
- [x] Update the only caller (`calculate_cost`) to thread cache tokens through.
- [x] Unit tests for every ordering pitfall and every cache scenario.

## Decisions

- **Explicit `if` chain over a `match` on guards.** Replacing the guard-match with a series of `if model.contains(...) { return Some(...) }` makes the priority reading top-to-bottom obvious. A `HashMap` lookup was considered but rejected because substring matching is still required (upstream provider prefixes vary: `anthropic/claude-opus-4-*`, `claude-3-opus`, etc.).
- **Word-boundary matching for reasoning models only.** `opus`/`sonnet`/`haiku`/`gpt-*`/`gemini-*` don't collide with other tokens in practice. `o1`/`o3`/`o4-mini` are the bite — they're short, not hyphenated-prefixed, and appear inside unrelated names. `has_reasoning_token` walks the string once and checks non-alphanumeric borders; no regex dependency needed.
- **Cache rates only for Anthropic.** OpenAI and Gemini entries stay `cache_read: None` / `cache_write: None`. The helper treats `None` as zero, so passing cache tokens for those models returns `$0` rather than an incorrect number. LiteLLM remains the source of truth for non-Anthropic cache pricing; this is a fallback.
- **Signature break is acceptable.** `calculate_cost_fallback` is module-private (`fn`, not `pub fn`). Only `calculate_cost` calls it, and it was already receiving `cache_read_tokens`/`cache_write_tokens`.

## Out of Scope

- No refresh of stale Gemini pricing tiers (thinking vs non-thinking). That's a data update, not a structural bug.
- No addition of `gpt-4.5` or other recent models — they belong to the LiteLLM-data refresh, not this structural fix.
- No change to `get_model_pricing` (the display-side fallback table in `get_fallback_pricing`).

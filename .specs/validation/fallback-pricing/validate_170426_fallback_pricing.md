# Validation Record - Fallback Pricing

## Automated Checks

- `cargo fmt --all --check` — PASS
- `cargo build` — PASS
- `cargo clippy --all-targets -- -D warnings` — PASS
- `cargo test` — PASS (18 pre-existing + 8 new = 26 tests)

## New Tests (`src/costs.rs::fallback_tests`)

| Test | Asserts |
|------|---------|
| `opus_includes_cache_costs` | 1M input + 1M output + 1M cache_read + 1M cache_write = $110.25 |
| `sonnet_cache_read_only` | 1M cache_read at sonnet rates = $0.30 |
| `gpt_4o1_preview_is_not_priced_as_o1` | If priced at all, cost ≠ o1's $15/MTok input |
| `o3_non_mini_has_pricing` | 1M input on `o3` = $2.00 |
| `o3_mini_takes_precedence_over_o3` | `o3-mini` matches before `o3` |
| `o4_mini_has_pricing` | 1M input on `o4-mini` = $1.10 |
| `gpt_4o_mini_takes_precedence_over_gpt_4o` | `gpt-4o-mini` at $0.15, not $2.50 |
| `unknown_model_returns_none` | fallback returns `None` for unknown models |
| `openai_no_cache_rates_ignore_cache_tokens` | cache tokens on OpenAI models contribute $0 |

## Regression Risk

- **Behavior shift for any real model that was accidentally matching `o1`.** `gpt-4o1-preview` (hypothetical) previously returned `Some($15/MTok)`; now it returns `None`. This is the intended behavior: surface "missing in fallback" rather than "silently mispriced." LiteLLM cache (the primary path) already had correct entries for real models.
- **Cost increase for Anthropic fallback users.** Users without a LiteLLM cache who were logging heavy cache-read/cache-write traffic will see higher, more accurate cost numbers after this PR. This is a correctness improvement, not a regression.
- **Anthropic cache rates are fixed constants.** If Anthropic adjusts public cache pricing, the fallback needs an update. The primary path (LiteLLM cache) picks up changes automatically via `update_pricing_cache`.

## Rollback

Single file, single commit. `git revert` restores the prior substring-based chain. Cache-token callers would still pass the extra args to the now-reverted signature — a follow-up revert of `calculate_cost`'s pass-through line would be required.

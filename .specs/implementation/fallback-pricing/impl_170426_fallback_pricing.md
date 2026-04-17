# Implementation Details - Fallback Pricing

## Summary

`src/costs.rs` — one file, ~220 insertions / ~21 deletions. Extracts the rate table into a helper, introduces word-boundary matching for reasoning models, and threads cache tokens through the fallback path.

## New Types / Helpers

```rust
struct FallbackRates {
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

fn rates_no_cache(input: f64, output: f64) -> FallbackRates { ... }

fn has_reasoning_token(model: &str, token: &str) -> bool {
    // matches `token` inside `model` only when both sides are either a
    // string boundary or a non-ASCII-alphanumeric byte. Prevents
    // `gpt-4o1-preview`.contains("o1") from being treated as "o1".
}
```

## Match Order (top → bottom)

```
opus → sonnet → haiku                             (Anthropic, with cache rates)
gpt-4o-mini → gpt-4o                              (OpenAI longest prefix first)
gpt-4.1-nano → gpt-4.1-mini → gpt-4.1
o4-mini → o3-mini → o3 → o1-mini → o1             (word-boundary via has_reasoning_token)
gemini-2.5-pro → gemini-2.5-flash → gemini-2.0-flash
_ → None
```

## Signature Change

```rust
fn calculate_cost_fallback(
    model: &str,
    _provider: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,   // NEW
    cache_write_tokens: i64,  // NEW
) -> Option<f64>
```

Cost computation:

```rust
let per_mtok = |tokens: i64, rate: f64| (tokens as f64 / 1_000_000.0) * rate;
let cost = per_mtok(input_tokens, rates.input)
    + per_mtok(output_tokens, rates.output)
    + rates.cache_read.map(|r| per_mtok(cache_read_tokens, r)).unwrap_or(0.0)
    + rates.cache_write.map(|r| per_mtok(cache_write_tokens, r)).unwrap_or(0.0);
```

## Caller Update

`calculate_cost` already had `cache_read_tokens` / `cache_write_tokens` as params. It now passes them to `calculate_cost_fallback` on the fallback path.

## Cache Rates Encoded (per MTok)

| Family | input | output | cache_read | cache_write |
|--------|-------|--------|------------|-------------|
| opus   | 15.00 | 75.00  | 1.50       | 18.75       |
| sonnet |  3.00 | 15.00  | 0.30       |  3.75       |
| haiku  |  0.80 |  4.00  | 0.08       |  1.00       |

## No Public API Changes

`calculate_cost_fallback` is module-private. `calculate_cost` (public) keeps its existing signature.

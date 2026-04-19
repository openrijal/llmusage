use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use serde::Deserialize;

use crate::models::ModelPricing;

const LITELLM_PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// LiteLLM model entry — we only deserialize the fields we need.
#[derive(Debug, Deserialize)]
struct LiteLLMEntry {
    #[serde(default)]
    litellm_provider: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    input_cost_per_token: Option<f64>,
    #[serde(default)]
    output_cost_per_token: Option<f64>,
    #[serde(default)]
    cache_read_input_token_cost: Option<f64>,
    #[serde(default)]
    cache_creation_input_token_cost: Option<f64>,
}

fn cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("llmusage")
        .join("litellm_pricing.json")
}

/// Fetch pricing from LiteLLM GitHub and cache locally.
pub async fn update_pricing_cache() -> Result<()> {
    let resp = reqwest::get(LITELLM_PRICING_URL).await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch LiteLLM pricing: {}", resp.status());
    }
    let body = resp.text().await?;

    // Validate it parses before caching
    let _: HashMap<String, serde_json::Value> = serde_json::from_str(&body)?;

    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &body)?;
    Ok(())
}

/// Process-lifetime cache of the parsed LiteLLM pricing file.
///
/// Reading and parsing the ~2MB JSON on every `calculate_cost` call was the
/// dominant cost during sync (#32). We load at most once per process — the
/// file only changes when `update_pricing_cache` is invoked, which happens as
/// a separate CLI command.
static PRICING_CACHE: OnceLock<Option<HashMap<String, LiteLLMEntry>>> = OnceLock::new();

/// Load cached pricing data. Returns None if no cache file exists or it fails
/// to parse. Subsequent calls reuse the in-memory result.
fn load_cached_pricing() -> Option<&'static HashMap<String, LiteLLMEntry>> {
    PRICING_CACHE
        .get_or_init(|| {
            let path = cache_path();
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        })
        .as_ref()
}

/// Map LiteLLM provider names to our provider names.
fn normalize_provider(litellm_provider: &str) -> Option<&'static str> {
    match litellm_provider {
        "anthropic" => Some("anthropic"),
        "openai" => Some("openai"),
        "gemini" | "vertex_ai" | "vertex_ai_beta" => Some("gemini"),
        "ollama" | "ollama_chat" => Some("ollama"),
        "bedrock" => Some("bedrock"),
        "azure" | "azure_ai" => Some("azure"),
        "deepseek" => Some("deepseek"),
        "openrouter" => Some("openrouter"),
        "groq" => Some("groq"),
        "together_ai" => Some("together"),
        "fireworks_ai" => Some("fireworks"),
        "mistral" => Some("mistral"),
        "cohere" | "cohere_chat" => Some("cohere"),
        "perplexity" => Some("perplexity"),
        _ => None,
    }
}

/// Get model pricing from LiteLLM cache, filtered by provider.
/// Only returns chat/completion models with token pricing.
pub fn get_model_pricing(provider_filter: Option<&str>) -> Vec<ModelPricing> {
    let entries = match load_cached_pricing() {
        Some(e) => e,
        None => return get_fallback_pricing(provider_filter),
    };

    let mut models: Vec<ModelPricing> = entries
        .iter()
        .filter_map(|(model_key, entry)| {
            // Only include chat models with token pricing
            let mode = entry.mode.as_deref().unwrap_or("");
            if mode != "chat" && mode != "completion" {
                return None;
            }

            let litellm_provider = entry.litellm_provider.as_deref()?;
            let provider = normalize_provider(litellm_provider)?;

            if let Some(filter) = provider_filter {
                if provider != filter {
                    return None;
                }
            }

            let input_per_token = entry.input_cost_per_token?;
            let output_per_token = entry.output_cost_per_token?;

            // Convert per-token to per-million-token for display
            Some(ModelPricing {
                provider: provider.to_string(),
                model: model_key.clone(),
                input_per_mtok: input_per_token * 1_000_000.0,
                output_per_mtok: output_per_token * 1_000_000.0,
                cache_read_per_mtok: entry.cache_read_input_token_cost.map(|c| c * 1_000_000.0),
                cache_write_per_mtok: entry
                    .cache_creation_input_token_cost
                    .map(|c| c * 1_000_000.0),
            })
        })
        .collect();

    models.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.model.cmp(&b.model)));
    models
}

/// Calculate cost in USD from token counts.
/// Tries LiteLLM cache first, falls back to hardcoded pricing.
pub fn calculate_cost(
    model: &str,
    provider: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
) -> Option<f64> {
    // Try loading from LiteLLM cache
    if let Some(entries) = load_cached_pricing() {
        // Try exact match first, then prefix match
        let prefixed = format!("{}/{}", provider, model);
        let entry = entries
            .get(model)
            .or_else(|| entries.get(&prefixed))
            .or_else(|| {
                // OpenRouter models use upstream names like "anthropic/claude-3.5-sonnet".
                // LiteLLM typically keys these as "openrouter/anthropic/claude-3.5-sonnet",
                // but when that misses we also try the bare upstream model after the last '/'.
                if provider == "openrouter" {
                    model
                        .rsplit_once('/')
                        .and_then(|(_, bare)| entries.get(bare))
                } else {
                    None
                }
            });

        if let Some(entry) = entry {
            if let (Some(input_cpt), Some(output_cpt)) =
                (entry.input_cost_per_token, entry.output_cost_per_token)
            {
                let input_cost = input_tokens as f64 * input_cpt;
                let output_cost = output_tokens as f64 * output_cpt;
                let cache_read_cost = entry
                    .cache_read_input_token_cost
                    .map(|c| cache_read_tokens as f64 * c)
                    .unwrap_or(0.0);
                let cache_write_cost = entry
                    .cache_creation_input_token_cost
                    .map(|c| cache_write_tokens as f64 * c)
                    .unwrap_or(0.0);
                return Some(input_cost + output_cost + cache_read_cost + cache_write_cost);
            }
        }
    }

    // Fallback to hardcoded pricing
    calculate_cost_fallback(
        model,
        provider,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
    )
}

/// Per-million-token rates for the fallback table.
struct FallbackRates {
    input: f64,
    output: f64,
    /// Cache read rate — `None` means no cache pricing available (falls back to 0).
    cache_read: Option<f64>,
    /// Cache creation (write) rate — `None` means no cache pricing available.
    cache_write: Option<f64>,
}

/// Resolve fallback rates for a model.
///
/// Ordering is deliberate: most-specific patterns first. OpenAI reasoning models
/// (`o1`, `o3`, `o4-mini`) use word-boundary matching so a model name like
/// `gpt-4o1-preview` can't accidentally be priced as `o1`.
fn fallback_rates(model: &str) -> Option<FallbackRates> {
    // Anthropic — cache pricing is well-defined for these families.
    if model.contains("opus") {
        return Some(FallbackRates {
            input: 15.0,
            output: 75.0,
            cache_read: Some(1.5),
            cache_write: Some(18.75),
        });
    }
    if model.contains("sonnet") {
        return Some(FallbackRates {
            input: 3.0,
            output: 15.0,
            cache_read: Some(0.3),
            cache_write: Some(3.75),
        });
    }
    if model.contains("haiku") {
        return Some(FallbackRates {
            input: 0.80,
            output: 4.0,
            cache_read: Some(0.08),
            cache_write: Some(1.0),
        });
    }

    // OpenAI GPT family — match longest prefix first.
    if model.contains("gpt-4o-mini") {
        return Some(rates_no_cache(0.15, 0.60));
    }
    if model.contains("gpt-4o") {
        return Some(rates_no_cache(2.50, 10.0));
    }
    if model.contains("gpt-4.1-nano") {
        return Some(rates_no_cache(0.10, 0.40));
    }
    if model.contains("gpt-4.1-mini") {
        return Some(rates_no_cache(0.40, 1.60));
    }
    if model.contains("gpt-4.1") {
        return Some(rates_no_cache(2.0, 8.0));
    }

    // OpenAI reasoning models — use word-boundary matching to avoid collisions
    // with names like `gpt-4o1-*` that happen to contain "o1".
    if has_reasoning_token(model, "o4-mini") {
        return Some(rates_no_cache(1.10, 4.40));
    }
    if has_reasoning_token(model, "o3-mini") {
        return Some(rates_no_cache(1.10, 4.40));
    }
    if has_reasoning_token(model, "o3") {
        return Some(rates_no_cache(2.0, 8.0));
    }
    if has_reasoning_token(model, "o1-mini") {
        return Some(rates_no_cache(3.0, 12.0));
    }
    if has_reasoning_token(model, "o1") {
        return Some(rates_no_cache(15.0, 60.0));
    }

    // DeepSeek — public pricing as of 2025-02. LiteLLM cache overrides when present.
    if model.contains("deepseek-reasoner") {
        return Some(rates_no_cache(0.55, 2.19));
    }
    if model.contains("deepseek-chat") {
        return Some(rates_no_cache(0.27, 1.10));
    }

    // Gemini family.
    if model.contains("gemini-2.5-pro") {
        return Some(rates_no_cache(1.25, 10.0));
    }
    if model.contains("gemini-2.5-flash") {
        return Some(rates_no_cache(0.15, 0.60));
    }
    if model.contains("gemini-2.0-flash") {
        return Some(rates_no_cache(0.10, 0.40));
    }

    None
}

fn rates_no_cache(input: f64, output: f64) -> FallbackRates {
    FallbackRates {
        input,
        output,
        cache_read: None,
        cache_write: None,
    }
}

/// Matches `token` inside `model` only when bordered by non-alphanumeric chars
/// (or string boundaries). Prevents `o1` from matching inside `gpt-4o1-preview`.
fn has_reasoning_token(model: &str, token: &str) -> bool {
    let bytes = model.as_bytes();
    let tb = token.as_bytes();
    if tb.is_empty() || bytes.len() < tb.len() {
        return false;
    }
    for i in 0..=bytes.len() - tb.len() {
        if &bytes[i..i + tb.len()] != tb {
            continue;
        }
        let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let right_idx = i + tb.len();
        let right_ok = right_idx == bytes.len() || !bytes[right_idx].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
    }
    false
}

fn calculate_cost_fallback(
    model: &str,
    _provider: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
) -> Option<f64> {
    let rates = fallback_rates(model)?;

    let per_mtok = |tokens: i64, rate: f64| (tokens as f64 / 1_000_000.0) * rate;
    let cost = per_mtok(input_tokens, rates.input)
        + per_mtok(output_tokens, rates.output)
        + rates
            .cache_read
            .map(|r| per_mtok(cache_read_tokens, r))
            .unwrap_or(0.0)
        + rates
            .cache_write
            .map(|r| per_mtok(cache_write_tokens, r))
            .unwrap_or(0.0);
    Some(cost)
}

/// Fallback pricing when no LiteLLM cache is available.
fn get_fallback_pricing(provider_filter: Option<&str>) -> Vec<ModelPricing> {
    let all = vec![
        mp(
            "anthropic",
            "claude-opus-4-20250514",
            15.0,
            75.0,
            Some(1.5),
            Some(18.75),
        ),
        mp(
            "anthropic",
            "claude-sonnet-4-20250514",
            3.0,
            15.0,
            Some(0.3),
            Some(3.75),
        ),
        mp(
            "anthropic",
            "claude-haiku-3-5-20241022",
            0.80,
            4.0,
            Some(0.08),
            Some(1.0),
        ),
        mp("openai", "gpt-4o", 2.50, 10.0, None, None),
        mp("openai", "gpt-4o-mini", 0.15, 0.60, None, None),
        mp("openai", "gpt-4.1", 2.0, 8.0, None, None),
        mp("openai", "gpt-4.1-mini", 0.40, 1.60, None, None),
        mp("openai", "o3-mini", 1.10, 4.40, None, None),
        mp("gemini", "gemini-2.5-pro", 1.25, 10.0, None, None),
        mp("gemini", "gemini-2.5-flash", 0.15, 0.60, None, None),
        mp("deepseek", "deepseek-chat", 0.27, 1.10, None, None),
        mp("deepseek", "deepseek-reasoner", 0.55, 2.19, None, None),
        mp("ollama", "local-models", 0.0, 0.0, None, None),
    ];

    match provider_filter {
        Some(p) => all.into_iter().filter(|m| m.provider == p).collect(),
        None => all,
    }
}

fn mp(
    provider: &str,
    model: &str,
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
) -> ModelPricing {
    ModelPricing {
        provider: provider.to_string(),
        model: model.to_string(),
        input_per_mtok: input,
        output_per_mtok: output,
        cache_read_per_mtok: cache_read,
        cache_write_per_mtok: cache_write,
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    #[test]
    fn opus_includes_cache_costs() {
        // 1M input + 1M output + 1M cache_read + 1M cache_write
        // = 15 + 75 + 1.5 + 18.75 = 110.25
        let cost = calculate_cost_fallback(
            "claude-opus-4-20250514",
            "anthropic",
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
        )
        .unwrap();
        assert!((cost - 110.25).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn sonnet_cache_read_only() {
        // 1M cache_read at $0.30/MTok
        let cost =
            calculate_cost_fallback("claude-sonnet-4", "anthropic", 0, 0, 1_000_000, 0).unwrap();
        assert!((cost - 0.30).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn gpt_4o1_preview_is_not_priced_as_o1() {
        // Regression: `gpt-4o1-preview` must not match the `o1` reasoning entry.
        // It should fall through to None (no fallback entry) because it isn't a real
        // OpenAI model; the important property is that we don't mis-price it as o1.
        let priced_as_o1 = calculate_cost_fallback("gpt-4o1-preview", "openai", 1_000_000, 0, 0, 0);
        // If this ever returns Some, it must not equal the o1 rate ($15/MTok input).
        if let Some(c) = priced_as_o1 {
            assert!((c - 15.0).abs() > 1e-6, "gpt-4o1-preview mispriced as o1");
        }
    }

    #[test]
    fn o3_non_mini_has_pricing() {
        let cost = calculate_cost_fallback("o3", "openai", 1_000_000, 0, 0, 0).unwrap();
        assert!((cost - 2.0).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn o3_mini_takes_precedence_over_o3() {
        let cost = calculate_cost_fallback("o3-mini", "openai", 1_000_000, 0, 0, 0).unwrap();
        assert!((cost - 1.10).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn o4_mini_has_pricing() {
        let cost = calculate_cost_fallback("o4-mini", "openai", 1_000_000, 0, 0, 0).unwrap();
        assert!((cost - 1.10).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn gpt_4o_mini_takes_precedence_over_gpt_4o() {
        let cost = calculate_cost_fallback("gpt-4o-mini", "openai", 1_000_000, 0, 0, 0).unwrap();
        assert!((cost - 0.15).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(calculate_cost_fallback("totally-unknown", "x", 1, 1, 0, 0).is_none());
    }

    #[test]
    fn openai_no_cache_rates_ignore_cache_tokens() {
        let cost = calculate_cost_fallback("gpt-4o", "openai", 0, 0, 1_000_000, 1_000_000).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn deepseek_fallback_pricing_included() {
        let models = get_fallback_pricing(Some("deepseek"));
        let names: Vec<&str> = models.iter().map(|m| m.model.as_str()).collect();
        assert!(names.contains(&"deepseek-chat"));
        assert!(names.contains(&"deepseek-reasoner"));
    }

    #[test]
    fn deepseek_reasoner_fallback_cost() {
        let cost =
            calculate_cost_fallback("deepseek-reasoner", "deepseek", 1_000_000, 1_000_000, 0, 0)
                .unwrap();
        // 0.55 input + 2.19 output per MTok
        assert!((cost - 2.74).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn openrouter_normalizes_to_openrouter() {
        assert_eq!(normalize_provider("openrouter"), Some("openrouter"));
    }

    #[test]
    fn deepseek_normalizes_to_deepseek() {
        assert_eq!(normalize_provider("deepseek"), Some("deepseek"));
    }

    #[test]
    fn normalize_provider_covers_known_aliases() {
        assert_eq!(normalize_provider("anthropic"), Some("anthropic"));
        assert_eq!(normalize_provider("openai"), Some("openai"));
        assert_eq!(normalize_provider("vertex_ai"), Some("gemini"));
        assert_eq!(normalize_provider("vertex_ai_beta"), Some("gemini"));
        assert_eq!(normalize_provider("ollama_chat"), Some("ollama"));
        assert_eq!(normalize_provider("azure_ai"), Some("azure"));
        assert_eq!(normalize_provider("cohere_chat"), Some("cohere"));
    }

    #[test]
    fn normalize_provider_rejects_unknown() {
        assert_eq!(normalize_provider("unknown-thing"), None);
        assert_eq!(normalize_provider(""), None);
    }
}

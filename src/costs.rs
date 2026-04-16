use std::collections::HashMap;
use std::path::PathBuf;

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

/// Load cached pricing data. Returns None if no cache exists.
fn load_cached_pricing() -> Option<HashMap<String, LiteLLMEntry>> {
    let path = cache_path();
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
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
        .into_iter()
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
                model: model_key,
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
        let entry = entries.get(model).or_else(|| {
            // Try provider/model format (e.g., "anthropic/claude-sonnet-4-20250514")
            let prefixed = format!("{}/{}", provider, model);
            entries.get(&prefixed)
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
    calculate_cost_fallback(model, provider, input_tokens, output_tokens)
}

fn calculate_cost_fallback(
    model: &str,
    _provider: &str,
    input_tokens: i64,
    output_tokens: i64,
) -> Option<f64> {
    // Hardcoded fallback for common models (per-million-token rates)
    let (input_rate, output_rate) = match model {
        m if m.contains("opus") => (15.0, 75.0),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (0.80, 4.0),
        m if m.contains("gpt-4o-mini") => (0.15, 0.60),
        m if m.contains("gpt-4o") => (2.50, 10.0),
        m if m.contains("gpt-4.1-nano") => (0.10, 0.40),
        m if m.contains("gpt-4.1-mini") => (0.40, 1.60),
        m if m.contains("gpt-4.1") => (2.0, 8.0),
        m if m.contains("o3-mini") => (1.10, 4.40),
        m if m.contains("o1-mini") => (3.0, 12.0),
        m if m.contains("o1") => (15.0, 60.0),
        m if m.contains("gemini-2.5-pro") => (1.25, 10.0),
        m if m.contains("gemini-2.5-flash") => (0.15, 0.60),
        m if m.contains("gemini-2.0-flash") => (0.10, 0.40),
        _ => return None,
    };

    let cost = (input_tokens as f64 / 1_000_000.0) * input_rate
        + (output_tokens as f64 / 1_000_000.0) * output_rate;
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

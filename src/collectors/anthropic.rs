use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct AnthropicCollector {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicCollector {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    data: Vec<UsageBucket>,
}

#[derive(Debug, Deserialize)]
struct UsageBucket {
    #[serde(default)]
    model: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    #[serde(default)]
    cache_creation_input_tokens: i64,
    #[serde(default)]
    cache_read_input_tokens: i64,
    #[serde(default)]
    date: Option<String>,
}

#[async_trait]
impl Collector for AnthropicCollector {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        // Anthropic Admin API: GET /v1/organizations/usage
        // Requires admin API key with usage:read scope
        let now = Utc::now();
        let start = (now - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        let end = now.format("%Y-%m-%d").to_string();

        let resp = self
            .client
            .get("https://api.anthropic.com/v1/organizations/usage")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .query(&[
                ("start_date", start.as_str()),
                ("end_date", end.as_str()),
                ("group_by", "model"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, body);
        }

        let usage: UsageResponse = resp.json().await?;
        let collected_at = Utc::now().to_rfc3339();

        let records = usage
            .data
            .into_iter()
            .filter(|b| b.input_tokens > 0 || b.output_tokens > 0)
            .map(|b| {
                let model = b.model.unwrap_or_else(|| "unknown".to_string());
                let cost = costs::calculate_cost(
                    &model,
                    "anthropic",
                    b.input_tokens,
                    b.output_tokens,
                    b.cache_read_input_tokens,
                    b.cache_creation_input_tokens,
                );
                UsageRecord {
                    id: None,
                    provider: "anthropic".to_string(),
                    model,
                    input_tokens: b.input_tokens,
                    output_tokens: b.output_tokens,
                    cache_read_tokens: b.cache_read_input_tokens,
                    cache_write_tokens: b.cache_creation_input_tokens,
                    cost_usd: cost,
                    session_id: None,
                    recorded_at: b.date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string()),
                    collected_at: collected_at.clone(),
                    metadata: None,
                }
            })
            .collect();

        Ok(records)
    }
}

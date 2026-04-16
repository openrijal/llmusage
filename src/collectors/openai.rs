use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct OpenAICollector {
    api_key: String,
    client: reqwest::Client,
}

impl OpenAICollector {
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
    aggregation_timestamp: i64,
    #[serde(default)]
    n_context_tokens_total: i64,
    #[serde(default)]
    n_generated_tokens_total: i64,
    #[serde(default)]
    snapshot_id: Option<String>,
}

#[async_trait]
impl Collector for OpenAICollector {
    fn name(&self) -> &str {
        "openai"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        // OpenAI Usage API: GET /v1/organization/usage
        // Groups by model (snapshot_id)
        let now = Utc::now();
        let start_time = (now - chrono::Duration::days(30)).timestamp();

        let resp = self
            .client
            .get("https://api.openai.com/v1/organization/usage")
            .bearer_auth(&self.api_key)
            .query(&[("start_time", &start_time.to_string())])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {}: {}", status, body);
        }

        let usage: UsageResponse = resp.json().await?;
        let collected_at = Utc::now().to_rfc3339();

        let records = usage
            .data
            .into_iter()
            .filter(|b| b.n_context_tokens_total > 0 || b.n_generated_tokens_total > 0)
            .map(|b| {
                let model = b.snapshot_id.unwrap_or_else(|| "unknown".to_string());
                let cost = costs::calculate_cost(
                    &model,
                    "openai",
                    b.n_context_tokens_total,
                    b.n_generated_tokens_total,
                    0,
                    0,
                );
                let recorded_at = chrono::DateTime::from_timestamp(b.aggregation_timestamp, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| now.format("%Y-%m-%d").to_string());

                UsageRecord {
                    id: None,
                    provider: "openai".to_string(),
                    model,
                    input_tokens: b.n_context_tokens_total,
                    output_tokens: b.n_generated_tokens_total,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    cost_usd: cost,
                    session_id: None,
                    recorded_at,
                    collected_at: collected_at.clone(),
                    metadata: None,
                }
            })
            .collect();

        Ok(records)
    }
}

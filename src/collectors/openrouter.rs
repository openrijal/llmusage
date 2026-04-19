use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct OpenRouterCollector {
    api_key: String,
    client: reqwest::Client,
}

impl OpenRouterCollector {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

/// OpenRouter daily activity response.
/// GET https://openrouter.ai/api/v1/activity?date_from=YYYY-MM-DD&date_to=YYYY-MM-DD
/// Returns per-day, per-model aggregated token usage and reported cost in USD.
#[derive(Debug, Deserialize)]
struct ActivityResponse {
    #[serde(default)]
    data: Vec<ActivityRow>,
}

#[derive(Debug, Deserialize)]
struct ActivityRow {
    #[serde(default)]
    date: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    usage: f64,
    #[serde(default)]
    prompt_tokens: i64,
    #[serde(default)]
    completion_tokens: i64,
    #[serde(default)]
    reasoning_tokens: i64,
}

#[async_trait]
impl Collector for OpenRouterCollector {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let now = Utc::now();
        let date_to = now.format("%Y-%m-%d").to_string();
        let date_from = (now - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();

        let collected_at = now.to_rfc3339();

        let resp = self
            .client
            .get("https://openrouter.ai/api/v1/activity")
            .bearer_auth(&self.api_key)
            .query(&[("date_from", &date_from), ("date_to", &date_to)])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenRouter activity API error {}: {}", status, body);
        }

        let activity: ActivityResponse = resp.json().await?;
        let mut records: Vec<UsageRecord> = Vec::new();

        for row in activity.data {
            let input_tokens = row.prompt_tokens;
            // Reasoning tokens bill like output on OpenRouter.
            let output_tokens = row.completion_tokens + row.reasoning_tokens;
            if input_tokens == 0 && output_tokens == 0 {
                continue;
            }

            let model = if row.model.is_empty() {
                "unknown".to_string()
            } else {
                row.model
            };

            let cost = if row.usage > 0.0 {
                Some(row.usage)
            } else {
                costs::calculate_cost(&model, "openrouter", input_tokens, output_tokens, 0, 0)
            };

            let recorded_at = if row.date.is_empty() {
                now.format("%Y-%m-%d").to_string()
            } else {
                row.date
            };

            records.push(UsageRecord {
                id: None,
                provider: "openrouter".to_string(),
                model,
                input_tokens,
                output_tokens,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_usd: cost,
                session_id: None,
                recorded_at,
                collected_at: collected_at.clone(),
                metadata: None,
            });
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_activity_response() {
        let sample = r#"{
            "data": [
                {
                    "date": "2026-04-18",
                    "model": "anthropic/claude-3.5-sonnet",
                    "model_permaslug": "anthropic/claude-3.5-sonnet",
                    "usage": 1.2345,
                    "byok_usage_inference": 0.0,
                    "requests": 10,
                    "prompt_tokens": 12000,
                    "completion_tokens": 3400,
                    "reasoning_tokens": 0
                },
                {
                    "date": "2026-04-18",
                    "model": "deepseek/deepseek-r1",
                    "usage": 0.0,
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "reasoning_tokens": 0
                }
            ]
        }"#;

        let parsed: ActivityResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].prompt_tokens, 12000);
        assert_eq!(parsed.data[0].completion_tokens, 3400);
        assert!((parsed.data[0].usage - 1.2345).abs() < 1e-9);
        assert_eq!(parsed.data[0].model, "anthropic/claude-3.5-sonnet");
    }

    #[test]
    fn handles_missing_fields() {
        let sample = r#"{"data": [{"date": "2026-04-18", "model": "x/y"}]}"#;
        let parsed: ActivityResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.data[0].prompt_tokens, 0);
        assert_eq!(parsed.data[0].usage, 0.0);
    }
}

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

/// Anthropic Messages Usage Report API response.
/// GET /v1/organizations/usage_report/messages — admin API key required.
/// See https://docs.anthropic.com/en/api/usage-cost-api
#[derive(Debug, Deserialize)]
struct UsageResponse {
    data: Vec<UsageBucket>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageBucket {
    #[serde(default)]
    starting_at: Option<String>,
    #[serde(default)]
    results: Vec<BucketResult>,
}

#[derive(Debug, Deserialize)]
struct BucketResult {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    uncached_input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_read_input_tokens: i64,
    #[serde(default)]
    cache_creation: Option<CacheCreation>,
}

#[derive(Debug, Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: i64,
    #[serde(default)]
    ephemeral_1h_input_tokens: i64,
}

#[async_trait]
impl Collector for AnthropicCollector {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let now = Utc::now();
        let start = (now - chrono::Duration::days(30))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let end = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let collected_at = now.to_rfc3339();
        let mut records: Vec<UsageRecord> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut query: Vec<(&str, String)> = vec![
                ("starting_at", start.clone()),
                ("ending_at", end.clone()),
                ("bucket_width", "1d".to_string()),
                ("group_by[]", "model".to_string()),
                ("limit", "31".to_string()),
            ];
            if let Some(p) = &page_token {
                query.push(("page", p.clone()));
            }

            let resp = self
                .client
                .get("https://api.anthropic.com/v1/organizations/usage_report/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2024-10-22")
                .query(&query)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Anthropic usage API error {}: {}", status, body);
            }

            let usage: UsageResponse = resp.json().await?;

            for bucket in usage.data {
                let recorded_at = bucket
                    .starting_at
                    .as_deref()
                    .and_then(|s| s.split('T').next())
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| now.format("%Y-%m-%d").to_string());

                for result in bucket.results {
                    let cache_write = result
                        .cache_creation
                        .as_ref()
                        .map(|c| c.ephemeral_5m_input_tokens + c.ephemeral_1h_input_tokens)
                        .unwrap_or(0);

                    if result.uncached_input_tokens == 0
                        && result.output_tokens == 0
                        && result.cache_read_input_tokens == 0
                        && cache_write == 0
                    {
                        continue;
                    }

                    let model = result.model.unwrap_or_else(|| "unknown".to_string());
                    let cost = costs::calculate_cost(
                        &model,
                        "anthropic",
                        result.uncached_input_tokens,
                        result.output_tokens,
                        result.cache_read_input_tokens,
                        cache_write,
                    );
                    records.push(UsageRecord {
                        id: None,
                        provider: "anthropic".to_string(),
                        model,
                        input_tokens: result.uncached_input_tokens,
                        output_tokens: result.output_tokens,
                        cache_read_tokens: result.cache_read_input_tokens,
                        cache_write_tokens: cache_write,
                        cost_usd: cost,
                        session_id: None,
                        recorded_at: recorded_at.clone(),
                        collected_at: collected_at.clone(),
                        metadata: None,
                    });
                }
            }

            if !usage.has_more {
                break;
            }
            match usage.next_page {
                Some(p) if !p.is_empty() => page_token = Some(p),
                _ => break,
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_report_response() {
        let sample = r#"{
            "data": [
                {
                    "starting_at": "2026-04-15T00:00:00Z",
                    "ending_at": "2026-04-16T00:00:00Z",
                    "results": [
                        {
                            "model": "claude-sonnet-4-6",
                            "uncached_input_tokens": 10000,
                            "output_tokens": 2500,
                            "cache_read_input_tokens": 3000,
                            "cache_creation": {
                                "ephemeral_5m_input_tokens": 500,
                                "ephemeral_1h_input_tokens": 200
                            },
                            "server_tool_use": { "web_search_requests": 0 }
                        }
                    ]
                }
            ],
            "has_more": false,
            "next_page": null
        }"#;

        let parsed: UsageResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.data.len(), 1);
        let r = &parsed.data[0].results[0];
        assert_eq!(r.uncached_input_tokens, 10000);
        assert_eq!(r.output_tokens, 2500);
        assert_eq!(r.cache_read_input_tokens, 3000);
        let cc = r.cache_creation.as_ref().unwrap();
        assert_eq!(cc.ephemeral_5m_input_tokens, 500);
        assert_eq!(cc.ephemeral_1h_input_tokens, 200);
        assert_eq!(r.model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn extracts_date_from_starting_at() {
        let sample = r#"{
            "data": [
                {"starting_at": "2026-04-15T00:00:00Z", "ending_at": "2026-04-16T00:00:00Z", "results": []}
            ],
            "has_more": false
        }"#;
        let parsed: UsageResponse = serde_json::from_str(sample).unwrap();
        let date = parsed.data[0]
            .starting_at
            .as_deref()
            .and_then(|s| s.split('T').next())
            .unwrap();
        assert_eq!(date, "2026-04-15");
    }
}

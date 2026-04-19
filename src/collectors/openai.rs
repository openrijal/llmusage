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

/// OpenAI organization usage (completions) API response.
/// GET /v1/organization/usage/completions — admin API key required.
/// See https://platform.openai.com/docs/api-reference/usage/completions
#[derive(Debug, Deserialize)]
struct UsageResponse {
    data: Vec<Bucket>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Bucket {
    #[serde(default)]
    start_time: i64,
    #[serde(default)]
    results: Vec<BucketResult>,
}

#[derive(Debug, Deserialize)]
struct BucketResult {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    input_cached_tokens: i64,
    #[serde(default)]
    model: Option<String>,
}

#[async_trait]
impl Collector for OpenAICollector {
    fn name(&self) -> &str {
        "openai"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let now = Utc::now();
        let start_time = (now - chrono::Duration::days(30)).timestamp();

        let collected_at = now.to_rfc3339();
        let mut records: Vec<UsageRecord> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut query: Vec<(&str, String)> = vec![
                ("start_time", start_time.to_string()),
                ("bucket_width", "1d".to_string()),
                ("group_by[]", "model".to_string()),
                ("limit", "31".to_string()),
            ];
            if let Some(p) = &page_token {
                query.push(("page", p.clone()));
            }

            let request = self
                .client
                .get("https://api.openai.com/v1/organization/usage/completions")
                .bearer_auth(&self.api_key)
                .query(&query);
            let resp = super::http::send_with_retry(request).await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("OpenAI usage API error {}: {}", status, body);
            }

            let usage: UsageResponse = resp.json().await?;

            for bucket in usage.data {
                let recorded_at = chrono::DateTime::from_timestamp(bucket.start_time, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| now.format("%Y-%m-%d").to_string());

                for result in bucket.results {
                    if result.input_tokens == 0
                        && result.output_tokens == 0
                        && result.input_cached_tokens == 0
                    {
                        continue;
                    }
                    let model = result.model.unwrap_or_else(|| "unknown".to_string());
                    let cost = costs::calculate_cost(
                        &model,
                        "openai",
                        result.input_tokens,
                        result.output_tokens,
                        result.input_cached_tokens,
                        0,
                    );
                    records.push(UsageRecord {
                        id: None,
                        provider: "openai".to_string(),
                        model,
                        input_tokens: result.input_tokens,
                        output_tokens: result.output_tokens,
                        cache_read_tokens: result.input_cached_tokens,
                        cache_write_tokens: 0,
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
    fn parses_bucket_response() {
        let sample = r#"{
            "data": [
                {
                    "object": "bucket",
                    "start_time": 1709251200,
                    "end_time": 1709337600,
                    "results": [
                        {
                            "object": "organization.usage.completions.result",
                            "input_tokens": 1200,
                            "output_tokens": 400,
                            "input_cached_tokens": 100,
                            "num_model_requests": 5,
                            "model": "gpt-4o-mini"
                        },
                        {
                            "object": "organization.usage.completions.result",
                            "input_tokens": 0,
                            "output_tokens": 0,
                            "input_cached_tokens": 0,
                            "model": "gpt-4o"
                        }
                    ]
                }
            ],
            "has_more": false,
            "next_page": null
        }"#;

        let parsed: UsageResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].results.len(), 2);
        assert_eq!(parsed.data[0].results[0].input_tokens, 1200);
        assert_eq!(parsed.data[0].results[0].input_cached_tokens, 100);
        assert_eq!(
            parsed.data[0].results[0].model.as_deref(),
            Some("gpt-4o-mini")
        );
        assert!(!parsed.has_more);
    }

    #[test]
    fn parses_paginated_response() {
        let sample = r#"{
            "data": [],
            "has_more": true,
            "next_page": "page_abc123"
        }"#;
        let parsed: UsageResponse = serde_json::from_str(sample).unwrap();
        assert!(parsed.has_more);
        assert_eq!(parsed.next_page.as_deref(), Some("page_abc123"));
    }
}

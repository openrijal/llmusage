//! DeepSeek collector.
//!
//! DeepSeek does not expose a public historical usage API. `/user/balance`
//! returns only the current balance. As a result, this collector validates
//! the API key (by calling `/user/balance`) and returns no `UsageRecord`s.
//! If DeepSeek adds a per-model usage endpoint in the future, this is the
//! place to plug it in.

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

use super::Collector;
use crate::models::UsageRecord;

pub struct DeepSeekCollector {
    api_key: String,
    client: reqwest::Client,
}

impl DeepSeekCollector {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BalanceResponse {
    #[serde(default)]
    is_available: bool,
    #[serde(default)]
    balance_infos: Vec<BalanceInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BalanceInfo {
    #[serde(default)]
    currency: String,
    #[serde(default)]
    total_balance: String,
}

#[async_trait]
impl Collector for DeepSeekCollector {
    fn name(&self) -> &str {
        "deepseek"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let resp = self
            .client
            .get("https://api.deepseek.com/user/balance")
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeepSeek balance API error {}: {}", status, body);
        }

        // Parse to verify a sane response; we don't surface balance as a
        // UsageRecord because there is no historical per-model data.
        let _balance: BalanceResponse = resp.json().await?;
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_balance_response() {
        let sample = r#"{
            "is_available": true,
            "balance_infos": [
                {"currency": "USD", "total_balance": "10.00", "granted_balance": "5.00", "topped_up_balance": "5.00"}
            ]
        }"#;
        let parsed: BalanceResponse = serde_json::from_str(sample).unwrap();
        assert!(parsed.is_available);
        assert_eq!(parsed.balance_infos.len(), 1);
        assert_eq!(parsed.balance_infos[0].currency, "USD");
        assert_eq!(parsed.balance_infos[0].total_balance, "10.00");
    }

    #[test]
    fn handles_missing_fields() {
        let sample = r#"{}"#;
        let parsed: BalanceResponse = serde_json::from_str(sample).unwrap();
        assert!(!parsed.is_available);
        assert!(parsed.balance_infos.is_empty());
    }
}

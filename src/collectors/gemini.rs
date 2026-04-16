use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use super::Collector;
use crate::models::UsageRecord;

pub struct GeminiCollector {
    #[allow(dead_code)]
    api_key: String,
}

impl GeminiCollector {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl Collector for GeminiCollector {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        // Gemini usage tracking is not straightforward programmatically.
        // Google AI Studio doesn't expose a clean usage API like Anthropic/OpenAI.
        // Options:
        //   1. Cloud Billing API (requires GCP project + billing export)
        //   2. AI Studio dashboard scraping (fragile)
        //   3. Track usage locally by intercepting API calls
        //
        // For now, this is a stub that returns empty.
        // Future: implement Cloud Billing API integration or local proxy tracking.
        let _ = Utc::now();
        anyhow::bail!(
            "Gemini collector not yet implemented. \
             Google AI Studio lacks a clean usage API. \
             Consider tracking via Cloud Billing export."
        )
    }
}

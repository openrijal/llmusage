use anyhow::Result;
use async_trait::async_trait;

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
        // As of 2026-04, Google AI Studio still exposes no first-party usage or
        // billing API for Gemini keys. The only programmatic route is the
        // Vertex AI billing export (requires a GCP project with billing export
        // configured), which is out of scope for this collector and warrants a
        // separate `vertex_ai` collector if pursued.
        //
        // Returning an empty record set keeps `llmusage sync` quiet for users
        // who have a Gemini key configured but no way to pull usage yet.
        eprintln!(
            "warning: gemini usage sync is a no-op — Google AI Studio has no usage API. \
             Track via Vertex AI billing export if you need Gemini numbers."
        );
        Ok(vec![])
    }
}

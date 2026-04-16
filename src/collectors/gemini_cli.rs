use anyhow::Result;
use async_trait::async_trait;

use super::Collector;
use crate::models::UsageRecord;

pub struct GeminiCliCollector;

impl Default for GeminiCliCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiCliCollector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Collector for GeminiCliCollector {
    fn name(&self) -> &str {
        "gemini_cli"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        // Gemini CLI stores conversations as .pb (protobuf) files in
        // ~/.gemini/antigravity/conversations/
        // Without the protobuf schema, we can't parse these reliably.
        //
        // Gemini CLI doesn't expose token usage in a readable format.
        // The tool tracks file changes but not API token consumption.
        //
        // Future options:
        //   1. Reverse-engineer the protobuf schema
        //   2. Use Google Cloud Billing API if the user has a GCP project
        //   3. Monitor Gemini API usage via the AI Studio dashboard
        //
        // For now, return empty with a note.
        Ok(vec![])
    }
}

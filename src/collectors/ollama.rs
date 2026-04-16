use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;

use super::Collector;
use crate::models::UsageRecord;

pub struct OllamaCollector {
    host: String,
    client: reqwest::Client,
}

impl OllamaCollector {
    pub fn new(host: String) -> Self {
        Self {
            host,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OllamaRunningResponse {
    models: Vec<OllamaRunningModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaRunningModel {
    name: String,
    #[serde(default)]
    size: i64,
}

#[async_trait]
impl Collector for OllamaCollector {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        // Ollama doesn't persist usage logs by default.
        // What we CAN do:
        //   1. List running models via /api/ps
        //   2. List available models via /api/tags
        //
        // For actual token tracking, you'd need to either:
        //   - Run a reverse proxy that logs request/response metadata
        //   - Patch Ollama to write usage to a file
        //   - Use the /api/generate response metadata (eval_count, prompt_eval_count)
        //
        // This collector checks if Ollama is reachable and reports available models.
        // Real usage tracking requires the proxy approach.

        let resp = self
            .client
            .get(format!("{}/api/ps", self.host))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let running: OllamaRunningResponse = r.json().await?;
                let collected_at = Utc::now().to_rfc3339();
                let now = Utc::now().format("%Y-%m-%d").to_string();

                // Report running models as a heartbeat record (0 tokens)
                // Actual token counts require proxy integration
                let records: Vec<UsageRecord> = running
                    .models
                    .into_iter()
                    .map(|m| UsageRecord {
                        id: None,
                        provider: "ollama".to_string(),
                        model: m.name,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        cost_usd: Some(0.0),
                        session_id: None,
                        recorded_at: now.clone(),
                        collected_at: collected_at.clone(),
                        metadata: Some(format!("{{\"size\": {}}}", m.size)),
                    })
                    .collect();

                if records.is_empty() {
                    Ok(vec![]) // Ollama running but no models loaded
                } else {
                    Ok(records)
                }
            }
            Ok(r) => {
                anyhow::bail!("Ollama returned status {}", r.status());
            }
            Err(_) => {
                anyhow::bail!(
                    "Could not connect to Ollama at {}. Is it running?",
                    self.host
                );
            }
        }
    }
}

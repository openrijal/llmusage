use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::path::PathBuf;

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct CodexCollector {
    codex_dir: PathBuf,
}

impl Default for CodexCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexCollector {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            codex_dir: home.join(".codex"),
        }
    }
}

/// Codex stores sessions as JSONL in ~/.codex/archived_sessions/
/// Token data is in event_msg entries with type="token_count"
/// Model info is in session_meta entries
#[derive(Debug, Deserialize)]
struct LogEntry {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    last_token_usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct TokenUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    cached_input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    reasoning_output_tokens: i64,
}

#[async_trait]
impl Collector for CodexCollector {
    fn name(&self) -> &str {
        "codex"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let sessions_dir = self.codex_dir.join("archived_sessions");
        if !sessions_dir.exists() {
            return Ok(vec![]);
        }

        let collected_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();

        for entry in std::fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Extract session ID and model from session_meta
            let mut session_id = None;
            let mut model_provider = String::from("openai");

            // First pass: get session metadata
            for line in content.lines() {
                if line.contains("session_meta") {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                        if let Some(payload) = &entry.payload {
                            if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
                                session_id = Some(id.to_string());
                            }
                            if let Some(mp) =
                                payload.get("model_provider").and_then(|v| v.as_str())
                            {
                                model_provider = mp.to_string();
                            }
                        }
                    }
                    break;
                }
            }

            // Codex uses GPT-5 or similar via OpenAI
            let model = format!("codex-{}", model_provider);

            // Second pass: collect token_count entries
            // Use last_token_usage which gives per-turn deltas
            let mut prev_input: i64 = 0;
            let mut prev_output: i64 = 0;

            for line in content.lines() {
                if !line.contains("token_count") {
                    continue;
                }

                if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                    if entry.r#type.as_deref() != Some("event_msg") {
                        continue;
                    }
                    if let Some(payload) = &entry.payload {
                        if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
                            continue;
                        }
                        if let Some(info) = payload.get("info") {
                            if info.is_null() {
                                continue;
                            }
                            if let Ok(token_info) =
                                serde_json::from_value::<TokenInfo>(info.clone())
                            {
                                if let Some(usage) = token_info.last_token_usage {
                                    // last_token_usage gives per-turn values
                                    let input = usage.input_tokens;
                                    let output =
                                        usage.output_tokens + usage.reasoning_output_tokens;

                                    // Skip if same as previous (duplicate event)
                                    if input == prev_input && output == prev_output {
                                        continue;
                                    }
                                    if input == 0 && output == 0 {
                                        continue;
                                    }
                                    prev_input = input;
                                    prev_output = output;

                                    let cost = costs::calculate_cost(
                                        &model,
                                        "openai",
                                        input,
                                        output,
                                        usage.cached_input_tokens,
                                        0,
                                    );

                                    records.push(UsageRecord {
                                        id: None,
                                        provider: "codex".to_string(),
                                        model: model.clone(),
                                        input_tokens: input,
                                        output_tokens: output,
                                        cache_read_tokens: usage.cached_input_tokens,
                                        cache_write_tokens: 0,
                                        cost_usd: cost,
                                        session_id: session_id.clone(),
                                        recorded_at: entry
                                            .timestamp
                                            .clone()
                                            .unwrap_or_else(|| {
                                                Utc::now()
                                                    .format("%Y-%m-%dT%H:%M:%S")
                                                    .to_string()
                                            }),
                                        collected_at: collected_at.clone(),
                                        metadata: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(records)
    }
}

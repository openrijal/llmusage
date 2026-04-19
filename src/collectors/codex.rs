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
            let mut model_name: Option<String> = None;

            // First pass: get session metadata
            for line in content.lines() {
                if line.contains("session_meta") {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                        if let Some(payload) = &entry.payload {
                            if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
                                session_id = Some(id.to_string());
                            }
                            if let Some(mp) = payload.get("model_provider").and_then(|v| v.as_str())
                            {
                                model_provider = mp.to_string();
                            }
                            // Codex records the actual model (e.g. "gpt-5",
                            // "o3", "gpt-4.1") on session_meta. Prefer it so
                            // costs match LiteLLM pricing and per-model
                            // breakdowns are meaningful (issue #35).
                            if let Some(m) = payload
                                .get("model")
                                .and_then(|v| v.as_str())
                                .or_else(|| payload.get("model_id").and_then(|v| v.as_str()))
                            {
                                if !m.is_empty() {
                                    model_name = Some(m.to_string());
                                }
                            }
                        }
                    }
                    break;
                }
            }

            // Fall back to the legacy `codex-{provider}` shape only when the
            // session log doesn't name a model.
            let model = model_name.unwrap_or_else(|| format!("codex-{}", model_provider));

            // Second pass: collect token_count entries.
            // Codex occasionally re-emits the exact same event line (e.g. on session
            // end), so we dedupe on the full raw line. Distinct turns that happen to
            // share identical token counts will still have different timestamps and
            // therefore different serialized lines, so they survive.
            let mut seen_lines: std::collections::HashSet<String> =
                std::collections::HashSet::new();

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
                                    if !seen_lines.insert(line.to_string()) {
                                        continue;
                                    }

                                    // last_token_usage gives per-turn values
                                    let input = usage.input_tokens;
                                    let output =
                                        usage.output_tokens + usage.reasoning_output_tokens;

                                    if input == 0 && output == 0 && usage.cached_input_tokens == 0 {
                                        continue;
                                    }

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
                                        recorded_at: entry.timestamp.clone().unwrap_or_else(|| {
                                            Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
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

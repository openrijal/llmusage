use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::path::PathBuf;

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct ClaudeCodeCollector {
    claude_dir: PathBuf,
}

impl Default for ClaudeCodeCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeCollector {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            claude_dir: home.join(".claude"),
        }
    }
}

/// Claude Code JSONL log entry.
/// Logs live at ~/.claude/projects/<project-hash>/<session-uuid>.jsonl
/// Usage data is on lines with type="assistant" inside message.usage.
#[derive(Debug, Deserialize)]
struct LogEntry {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    message: Option<MessageData>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, alias = "sessionId")]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageData {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<UsageData>,
}

#[derive(Debug, Deserialize)]
struct UsageData {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_creation_input_tokens: i64,
    #[serde(default)]
    cache_read_input_tokens: i64,
}

#[async_trait]
impl Collector for ClaudeCodeCollector {
    fn name(&self) -> &str {
        "claude_code"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(vec![]);
        }

        let collected_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();

        // Walk project directories for JSONL session logs
        for project_entry in std::fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            if !project_entry.file_type()?.is_dir() {
                continue;
            }

            // JSONL files are directly in the project directory
            let project_path = project_entry.path();
            for file_entry in std::fs::read_dir(&project_path)? {
                let file_entry = file_entry?;
                let path = file_entry.path();

                if path.extension().is_none_or(|e| e != "jsonl") {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }

                        // Quick filter: only parse lines that look like assistant messages with usage
                        if !line.contains("\"usage\"") {
                            continue;
                        }

                        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                            // Only process assistant messages with usage data
                            if entry.r#type.as_deref() != Some("assistant") {
                                continue;
                            }

                            if let Some(ref msg) = entry.message {
                                if let Some(ref usage) = msg.usage {
                                    if usage.input_tokens == 0 && usage.output_tokens == 0 {
                                        continue;
                                    }

                                    let model = msg
                                        .model
                                        .clone()
                                        .unwrap_or_else(|| "claude-unknown".to_string());

                                    let cost = costs::calculate_cost(
                                        &model,
                                        "anthropic",
                                        usage.input_tokens,
                                        usage.output_tokens,
                                        usage.cache_read_input_tokens,
                                        usage.cache_creation_input_tokens,
                                    );

                                    records.push(UsageRecord {
                                        id: None,
                                        provider: "claude_code".to_string(),
                                        model,
                                        input_tokens: usage.input_tokens,
                                        output_tokens: usage.output_tokens,
                                        cache_read_tokens: usage.cache_read_input_tokens,
                                        cache_write_tokens: usage.cache_creation_input_tokens,
                                        cost_usd: cost,
                                        session_id: entry.session_id.clone(),
                                        recorded_at: entry.timestamp.clone().unwrap_or_else(
                                            || {
                                                Utc::now()
                                                    .format("%Y-%m-%dT%H:%M:%S")
                                                    .to_string()
                                            },
                                        ),
                                        collected_at: collected_at.clone(),
                                        metadata: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Also check subagents/ subdirectories
            let subagents_dir = project_path.join("subagents");
            if subagents_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&subagents_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "jsonl") {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                for line in content.lines() {
                                    if !line.contains("\"usage\"") {
                                        continue;
                                    }
                                    if let Ok(entry) =
                                        serde_json::from_str::<LogEntry>(line)
                                    {
                                        if entry.r#type.as_deref() != Some("assistant") {
                                            continue;
                                        }
                                        if let Some(ref msg) = entry.message {
                                            if let Some(ref usage) = msg.usage {
                                                if usage.input_tokens == 0
                                                    && usage.output_tokens == 0
                                                {
                                                    continue;
                                                }
                                                let model = msg
                                                    .model
                                                    .clone()
                                                    .unwrap_or_else(|| {
                                                        "claude-unknown".to_string()
                                                    });
                                                let cost = costs::calculate_cost(
                                                    &model,
                                                    "anthropic",
                                                    usage.input_tokens,
                                                    usage.output_tokens,
                                                    usage.cache_read_input_tokens,
                                                    usage.cache_creation_input_tokens,
                                                );
                                                records.push(UsageRecord {
                                                    id: None,
                                                    provider: "claude_code".to_string(),
                                                    model,
                                                    input_tokens: usage.input_tokens,
                                                    output_tokens: usage.output_tokens,
                                                    cache_read_tokens: usage
                                                        .cache_read_input_tokens,
                                                    cache_write_tokens: usage
                                                        .cache_creation_input_tokens,
                                                    cost_usd: cost,
                                                    session_id: entry.session_id.clone(),
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
                }
            }
        }

        Ok(records)
    }
}

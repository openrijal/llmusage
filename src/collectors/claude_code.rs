use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

fn parse_jsonl_file(path: &Path, collected_at: &str) -> Vec<UsageRecord> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut records = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() || !line.contains("\"usage\"") {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<LogEntry>(line) else {
            continue;
        };

        if entry.r#type.as_deref() != Some("assistant") {
            continue;
        }

        let Some(msg) = entry.message.as_ref() else {
            continue;
        };
        let Some(usage) = msg.usage.as_ref() else {
            continue;
        };
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
            recorded_at: entry
                .timestamp
                .clone()
                .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()),
            collected_at: collected_at.to_string(),
            metadata: None,
        });
    }
    records
}

fn collect_jsonl_in_dir(dir: &Path, collected_at: &str, records: &mut Vec<UsageRecord>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            records.extend(parse_jsonl_file(&path, collected_at));
        }
    }
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

        for project_entry in std::fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            if !project_entry.file_type()?.is_dir() {
                continue;
            }
            let project_path = project_entry.path();

            collect_jsonl_in_dir(&project_path, &collected_at, &mut records);

            let subagents_dir = project_path.join("subagents");
            if subagents_dir.exists() {
                collect_jsonl_in_dir(&subagents_dir, &collected_at, &mut records);
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tempfile_path(tag: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llmusage-{tag}-{}-{nanos}.jsonl",
            std::process::id()
        ))
    }

    #[test]
    fn parse_jsonl_file_extracts_assistant_usage_and_skips_others() {
        let path = tempfile_path("claude-code-jsonl");
        let file = std::fs::File::create(&path).unwrap();
        let mut w = std::io::BufWriter::new(file);
        writeln!(w, r#"{{"type":"user","message":{{"content":"hi"}}}}"#).unwrap();
        writeln!(
            w,
            r#"{{"type":"assistant","sessionId":"s1","timestamp":"2026-04-01T00:00:00Z","message":{{"model":"claude-sonnet-4","usage":{{"input_tokens":0,"output_tokens":0}}}}}}"#
        )
        .unwrap();
        writeln!(
            w,
            r#"{{"type":"assistant","sessionId":"s1","timestamp":"2026-04-01T00:00:01Z","message":{{"model":"claude-sonnet-4","usage":{{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":2,"cache_creation_input_tokens":3}}}}}}"#
        )
        .unwrap();
        drop(w);

        let recs = parse_jsonl_file(&path, "2026-04-01T00:00:02Z");
        let _ = std::fs::remove_file(&path);

        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.provider, "claude_code");
        assert_eq!(r.input_tokens, 10);
        assert_eq!(r.output_tokens, 5);
        assert_eq!(r.cache_read_tokens, 2);
        assert_eq!(r.cache_write_tokens, 3);
        assert_eq!(r.session_id.as_deref(), Some("s1"));
    }
}

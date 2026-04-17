use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct GeminiCliCollector {
    gemini_dir: PathBuf,
}

impl Default for GeminiCliCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiCliCollector {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            gemini_dir: home.join(".gemini"),
        }
    }

    #[cfg(test)]
    fn with_dir(dir: PathBuf) -> Self {
        Self { gemini_dir: dir }
    }
}

/// Flattened union over the record shapes found in Gemini CLI JSONL session files.
/// See gemini-cli packages/core/src/services/chatRecordingTypes.ts — records are one of:
/// initial metadata, `{$set: {...}}` update, `{$rewindTo: <id>}`, or a message record.
#[derive(Debug, Deserialize)]
struct Line {
    #[serde(default, rename = "$rewindTo")]
    rewind_to: Option<String>,
    #[serde(default, rename = "$set")]
    set: Option<serde_json::Value>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    tokens: Option<TokensSummary>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct TokensSummary {
    #[serde(default)]
    input: i64,
    #[serde(default)]
    output: i64,
    #[serde(default)]
    cached: i64,
    #[serde(default)]
    total: i64,
    #[serde(default)]
    thoughts: Option<i64>,
    #[serde(default)]
    tool: Option<i64>,
}

struct MsgData {
    timestamp: Option<String>,
    tokens: TokensSummary,
    model: Option<String>,
}

#[async_trait]
impl Collector for GeminiCliCollector {
    fn name(&self) -> &str {
        "gemini_cli"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let tmp_dir = self.gemini_dir.join("tmp");
        let collected_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();

        if tmp_dir.exists() {
            for project_entry in std::fs::read_dir(&tmp_dir)?.flatten() {
                if !project_entry
                    .file_type()
                    .map(|t| t.is_dir())
                    .unwrap_or(false)
                {
                    continue;
                }
                let chats_dir = project_entry.path().join("chats");
                if chats_dir.exists() {
                    collect_chats_dir(&chats_dir, &collected_at, &mut records);
                }
            }
        }

        if records.is_empty() && has_legacy_pb_files(&self.gemini_dir) {
            eprintln!(
                "llmusage: found legacy Gemini CLI .pb session files but no JSONL sessions. \
                 Protobuf sessions are opaque and will be skipped — upgrade Gemini CLI and \
                 start a new session to record token usage."
            );
        }

        Ok(records)
    }
}

fn collect_chats_dir(chats_dir: &Path, collected_at: &str, records: &mut Vec<UsageRecord>) {
    let entries = match std::fs::read_dir(chats_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };

        if ft.is_dir() {
            // Subagent sessions live one level deeper under the parent session id.
            collect_chats_dir(&path, collected_at, records);
            continue;
        }

        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        parse_jsonl(&content, collected_at, records);
    }
}

fn parse_jsonl(content: &str, collected_at: &str, records: &mut Vec<UsageRecord>) {
    let mut session_id: Option<String> = None;
    let mut messages: HashMap<String, MsgData> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Line = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // $rewindTo drops the referenced message and everything after it.
        if let Some(rewind_id) = parsed.rewind_to {
            if let Some(pos) = order.iter().position(|id| id == &rewind_id) {
                for id in order.drain(pos..) {
                    messages.remove(&id);
                }
            } else {
                messages.clear();
                order.clear();
            }
            continue;
        }

        if parsed.set.is_some() {
            continue;
        }

        if let Some(msg_id) = parsed.id.clone() {
            if parsed.r#type.as_deref() != Some("gemini") {
                continue;
            }
            // Messages are appended twice (first without tokens, then with tokens).
            // Only care about lines that actually have a tokens summary.
            let tokens = match parsed.tokens {
                Some(t) => t,
                None => continue,
            };
            let data = MsgData {
                timestamp: parsed.timestamp,
                tokens,
                model: parsed.model,
            };
            if messages.insert(msg_id.clone(), data).is_none() {
                order.push(msg_id);
            }
        } else if let Some(sid) = parsed.session_id {
            session_id = Some(sid);
        }
    }

    for id in &order {
        let Some(data) = messages.get(id) else {
            continue;
        };
        let tokens = &data.tokens;
        if tokens.input == 0 && tokens.output == 0 && tokens.cached == 0 && tokens.total == 0 {
            continue;
        }
        let model = data
            .model
            .clone()
            .unwrap_or_else(|| "gemini-unknown".to_string());
        let cost = costs::calculate_cost(
            &model,
            "gemini",
            tokens.input,
            tokens.output,
            tokens.cached,
            0,
        );
        let metadata = if tokens.thoughts.is_some() || tokens.tool.is_some() {
            serde_json::to_string(&serde_json::json!({
                "thoughts_tokens": tokens.thoughts,
                "tool_tokens": tokens.tool,
            }))
            .ok()
        } else {
            None
        };
        records.push(UsageRecord {
            id: None,
            provider: "gemini_cli".to_string(),
            model,
            input_tokens: tokens.input,
            output_tokens: tokens.output,
            cache_read_tokens: tokens.cached,
            cache_write_tokens: 0,
            cost_usd: cost,
            session_id: session_id.clone(),
            recorded_at: data
                .timestamp
                .clone()
                .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()),
            collected_at: collected_at.to_string(),
            metadata,
        });
    }
}

fn has_legacy_pb_files(gemini_dir: &Path) -> bool {
    let legacy_dirs = [
        gemini_dir.join("antigravity").join("conversations"),
        gemini_dir.join("antigravity").join("implicit"),
    ];
    for d in &legacy_dirs {
        if !d.exists() {
            continue;
        }
        if let Ok(mut it) = std::fs::read_dir(d) {
            if it.any(|e| {
                e.ok()
                    .is_some_and(|e| e.path().extension().is_some_and(|ext| ext == "pb"))
            }) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_jsonl(path: &Path, lines: &[&str]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, lines.join("\n")).unwrap();
    }

    #[tokio::test]
    async fn parses_tokens_from_gemini_messages() {
        let tmp = tempdir();
        let chats = tmp.join("tmp").join("project-a").join("chats");
        write_jsonl(
            &chats.join("session-2026-04-16T10-00-abcd.jsonl"),
            &[
                r#"{"sessionId":"sess-1","projectHash":"project-a","startTime":"2026-04-16T10:00:00Z","lastUpdated":"2026-04-16T10:05:00Z"}"#,
                r#"{"id":"m1","timestamp":"2026-04-16T10:00:01Z","type":"user","content":"hi"}"#,
                r#"{"id":"m2","timestamp":"2026-04-16T10:00:02Z","type":"gemini","content":"","model":"gemini-2.5-pro","tokens":{"input":120,"output":40,"cached":10,"total":175,"thoughts":5,"tool":0}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.provider, "gemini_cli");
        assert_eq!(r.model, "gemini-2.5-pro");
        assert_eq!(r.input_tokens, 120);
        assert_eq!(r.output_tokens, 40);
        assert_eq!(r.cache_read_tokens, 10);
        assert_eq!(r.session_id.as_deref(), Some("sess-1"));
        assert_eq!(r.recorded_at, "2026-04-16T10:00:02Z");
        assert!(r
            .metadata
            .as_deref()
            .unwrap_or("")
            .contains("thoughts_tokens"));

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn dedupes_duplicate_message_writes() {
        // The same message id is appended twice: once without tokens, then with.
        let tmp = tempdir();
        let chats = tmp.join("tmp").join("project-b").join("chats");
        write_jsonl(
            &chats.join("session-dup.jsonl"),
            &[
                r#"{"sessionId":"sess-dup","projectHash":"project-b"}"#,
                r#"{"id":"g1","timestamp":"2026-04-16T11:00:00Z","type":"gemini","content":""}"#,
                r#"{"id":"g1","timestamp":"2026-04-16T11:00:05Z","type":"gemini","content":"","model":"gemini-2.5-flash","tokens":{"input":50,"output":20,"cached":0,"total":70}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].input_tokens, 50);
        assert_eq!(records[0].model, "gemini-2.5-flash");

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn honors_rewind_records() {
        let tmp = tempdir();
        let chats = tmp.join("tmp").join("project-c").join("chats");
        write_jsonl(
            &chats.join("s.jsonl"),
            &[
                r#"{"sessionId":"sess-c","projectHash":"project-c"}"#,
                r#"{"id":"a","timestamp":"t1","type":"gemini","model":"gemini-2.5-pro","content":"","tokens":{"input":10,"output":5,"cached":0,"total":15}}"#,
                r#"{"id":"b","timestamp":"t2","type":"gemini","model":"gemini-2.5-pro","content":"","tokens":{"input":20,"output":10,"cached":0,"total":30}}"#,
                r#"{"$rewindTo":"b"}"#,
                r#"{"id":"c","timestamp":"t3","type":"gemini","model":"gemini-2.5-pro","content":"","tokens":{"input":7,"output":3,"cached":0,"total":10}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].input_tokens, 10);
        assert_eq!(records[1].input_tokens, 7);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn skips_malformed_lines_and_non_gemini_types() {
        let tmp = tempdir();
        let chats = tmp.join("tmp").join("project-d").join("chats");
        write_jsonl(
            &chats.join("s.jsonl"),
            &[
                r#"{"sessionId":"sess-d","projectHash":"project-d"}"#,
                r#"not json at all"#,
                r#"{"id":"u1","type":"user","content":"hello"}"#,
                r#"{"id":"w1","type":"warning","content":"heads up"}"#,
                r#"{"id":"g1","type":"gemini","model":"gemini-2.5-flash","content":"","tokens":{"input":1,"output":2,"cached":0,"total":3}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].input_tokens, 1);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn ignores_zero_token_messages() {
        let tmp = tempdir();
        let chats = tmp.join("tmp").join("project-e").join("chats");
        write_jsonl(
            &chats.join("s.jsonl"),
            &[
                r#"{"sessionId":"sess-e","projectHash":"project-e"}"#,
                r#"{"id":"g1","type":"gemini","model":"gemini-2.5-pro","content":"","tokens":{"input":0,"output":0,"cached":0,"total":0}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert!(records.is_empty());

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn walks_subagent_subdirectories() {
        let tmp = tempdir();
        let chats = tmp
            .join("tmp")
            .join("project-f")
            .join("chats")
            .join("parent-session-id");
        write_jsonl(
            &chats.join("subagent.jsonl"),
            &[
                r#"{"sessionId":"sub","projectHash":"project-f"}"#,
                r#"{"id":"g1","type":"gemini","model":"gemini-2.5-flash","content":"","tokens":{"input":9,"output":3,"cached":0,"total":12}}"#,
            ],
        );

        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].input_tokens, 9);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn returns_empty_when_no_tmp_dir() {
        let tmp = tempdir();
        let c = GeminiCliCollector::with_dir(tmp.clone());
        let records = c.collect().await.unwrap();
        assert!(records.is_empty());
        cleanup(&tmp);
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "llmusage-gemini-test-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            n
        ));
        // Start fresh even if a prior crashed run left a directory behind.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(p: &Path) {
        let _ = std::fs::remove_dir_all(p);
    }
}

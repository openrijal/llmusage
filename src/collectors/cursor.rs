use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Collector;
use crate::costs;
use crate::models::UsageRecord;

pub struct CursorCollector {
    db_path: PathBuf,
}

impl Default for CursorCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorCollector {
    pub fn new() -> Self {
        Self {
            db_path: cursor_state_db_path(),
        }
    }

    #[cfg(test)]
    fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

#[derive(Debug, Deserialize)]
struct ComposerData {
    #[serde(default, rename = "modelConfig")]
    model_config: Option<ModelConfig>,
}

#[derive(Debug, Deserialize)]
struct ModelConfig {
    #[serde(default, rename = "modelName")]
    model_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BubbleData {
    #[serde(default, rename = "tokenCount")]
    token_count: Option<TokenCount>,
    #[serde(default, rename = "timingInfo")]
    timing_info: Option<TimingInfo>,
    #[serde(default, rename = "usageUuid")]
    usage_uuid: Option<String>,
    #[serde(default, rename = "serverBubbleId")]
    server_bubble_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "isAgentic")]
    is_agentic: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TokenCount {
    #[serde(default, rename = "inputTokens")]
    input_tokens: i64,
    #[serde(default, rename = "outputTokens")]
    output_tokens: i64,
}

#[derive(Debug, Deserialize)]
struct TimingInfo {
    #[serde(default, rename = "clientEndTime")]
    client_end_time: Option<i64>,
    #[serde(default, rename = "clientSettleTime")]
    client_settle_time: Option<i64>,
    #[serde(default, rename = "clientStartTime")]
    client_start_time: Option<i64>,
}

#[async_trait]
impl Collector for CursorCollector {
    fn name(&self) -> &str {
        "cursor"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        if !self.db_path.exists() {
            return Ok(vec![]);
        }

        let temp_db = copy_to_temp(&self.db_path, "cursor-state.vscdb")?;
        let collected_at = Utc::now().to_rfc3339();
        let conn = Connection::open(&temp_db)?;
        if !has_cursor_disk_kv(&conn)? {
            let _ = std::fs::remove_file(temp_db);
            return Ok(vec![]);
        }

        let model_names = load_model_names(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT key, CAST(value AS TEXT)
             FROM cursorDiskKV
             WHERE key LIKE 'bubbleId:%'
               AND (
                 COALESCE(json_extract(CAST(value AS TEXT), '$.tokenCount.inputTokens'), 0) > 0
                 OR COALESCE(json_extract(CAST(value AS TEXT), '$.tokenCount.outputTokens'), 0) > 0
               )",
        )?;

        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let payload: Option<String> = row.get(1)?;
            Ok((key, payload))
        })?;

        let mut records = Vec::new();
        for row in rows {
            let (key, payload) = row?;
            let Some(payload) = payload else {
                continue;
            };
            if let Some(record) = parse_bubble_row(&key, &payload, &collected_at, &model_names)? {
                records.push(record);
            }
        }

        let _ = std::fs::remove_file(temp_db);
        Ok(records)
    }
}

pub fn cursor_state_db_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

fn copy_to_temp(source: &Path, suffix: &str) -> Result<PathBuf> {
    let temp_path =
        std::env::temp_dir().join(format!("llmusage-{}-{}", std::process::id(), suffix));
    std::fs::copy(source, &temp_path)?;
    Ok(temp_path)
}

fn has_cursor_disk_kv(conn: &Connection) -> Result<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = 'cursorDiskKV'
        )",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists == 1)
}

fn load_model_names(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut model_names = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT key, CAST(value AS TEXT)
         FROM cursorDiskKV
         WHERE key LIKE 'composerData:%'",
    )?;
    let rows = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let payload: Option<String> = row.get(1)?;
        Ok((key, payload))
    })?;

    for row in rows {
        let (key, payload) = row?;
        let Some(payload) = payload else {
            continue;
        };
        let Some(composer_id) = key.strip_prefix("composerData:") else {
            continue;
        };
        let parsed: ComposerData = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(model_name) = parsed
            .model_config
            .and_then(|cfg| cfg.model_name)
            .filter(|name| !name.trim().is_empty())
        {
            model_names.insert(composer_id.to_string(), model_name);
        }
    }

    Ok(model_names)
}

fn parse_bubble_row(
    key: &str,
    payload: &str,
    collected_at: &str,
    model_names: &HashMap<String, String>,
) -> Result<Option<UsageRecord>> {
    let Some(raw_ids) = key.strip_prefix("bubbleId:") else {
        return Ok(None);
    };
    let Some((composer_id, bubble_id)) = raw_ids.split_once(':') else {
        return Ok(None);
    };

    let parsed: BubbleData = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let Some(token_count) = parsed.token_count.as_ref() else {
        return Ok(None);
    };
    if token_count.input_tokens == 0 && token_count.output_tokens == 0 {
        return Ok(None);
    }

    let model = model_names
        .get(composer_id)
        .cloned()
        .unwrap_or_else(|| "cursor-default".to_string());
    let recorded_at = bubble_recorded_at(&parsed).unwrap_or_else(|| Utc::now().to_rfc3339());
    let cost_usd = infer_priced_provider(&model).and_then(|provider| {
        costs::calculate_cost(
            &model,
            provider,
            token_count.input_tokens,
            token_count.output_tokens,
            0,
            0,
        )
    });
    let metadata = serde_json::to_string(&serde_json::json!({
        "bubble_id": bubble_id,
        "usage_uuid": parsed.usage_uuid,
        "server_bubble_id": parsed.server_bubble_id,
        "is_agentic": parsed.is_agentic,
        "preview": parsed.text.as_deref().map(trim_preview),
    }))
    .ok();

    Ok(Some(UsageRecord {
        id: None,
        provider: "cursor".to_string(),
        model,
        input_tokens: token_count.input_tokens,
        output_tokens: token_count.output_tokens,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        cost_usd,
        session_id: Some(composer_id.to_string()),
        recorded_at,
        collected_at: collected_at.to_string(),
        metadata,
    }))
}

fn bubble_recorded_at(parsed: &BubbleData) -> Option<String> {
    let millis = parsed
        .timing_info
        .as_ref()
        .and_then(|info| info.client_end_time)
        .or_else(|| {
            parsed
                .timing_info
                .as_ref()
                .and_then(|info| info.client_settle_time)
        })
        .or_else(|| {
            parsed
                .timing_info
                .as_ref()
                .and_then(|info| info.client_start_time)
        })?;

    DateTime::from_timestamp_millis(millis).map(|ts| ts.to_rfc3339())
}

fn infer_priced_provider(model: &str) -> Option<&'static str> {
    let normalized = model.strip_prefix("cursor-").unwrap_or(model);
    if normalized.starts_with("claude") {
        Some("anthropic")
    } else if normalized.starts_with("gpt")
        || normalized.starts_with("o1")
        || normalized.starts_with("o3")
    {
        Some("openai")
    } else if normalized.starts_with("gemini") {
        Some("gemini")
    } else {
        None
    }
}

fn trim_preview(text: &str) -> String {
    const LIMIT: usize = 160;
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        let preview: String = trimmed.chars().take(LIMIT).collect();
        format!("{}...", preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("llmusage-test-{}-{}", std::process::id(), name))
    }

    #[test]
    fn parses_cursor_bubble_row() {
        let mut model_names = HashMap::new();
        model_names.insert("composer-1".to_string(), "gpt-4.1".to_string());

        let payload = serde_json::json!({
            "tokenCount": {
                "inputTokens": 1200,
                "outputTokens": 300
            },
            "timingInfo": {
                "clientEndTime": 1741448289691_i64
            },
            "usageUuid": "usage-1",
            "serverBubbleId": "server-1",
            "isAgentic": true,
            "text": "A fairly long assistant response"
        })
        .to_string();

        let record = parse_bubble_row(
            "bubbleId:composer-1:bubble-1",
            &payload,
            "2026-04-16T12:00:00Z",
            &model_names,
        )
        .unwrap()
        .unwrap();

        assert_eq!(record.provider, "cursor");
        assert_eq!(record.model, "gpt-4.1");
        assert_eq!(record.input_tokens, 1200);
        assert_eq!(record.output_tokens, 300);
        assert_eq!(record.session_id.as_deref(), Some("composer-1"));
        assert!(record.recorded_at.starts_with("2025-03-"));
        assert!(record
            .metadata
            .as_deref()
            .unwrap_or("")
            .contains("\"bubble_id\":\"bubble-1\""));
    }

    #[tokio::test]
    async fn collector_reads_temp_copied_cursor_db() {
        let db_path = temp_file("cursor-state.vscdb");
        let _ = fs::remove_file(&db_path);

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE cursorDiskKV (key TEXT, value BLOB)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
            (
                "composerData:composer-1",
                serde_json::json!({
                    "modelConfig": {
                        "modelName": "gpt-4.1"
                    }
                })
                .to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
            (
                "bubbleId:composer-1:bubble-1",
                serde_json::json!({
                    "tokenCount": {
                        "inputTokens": 42,
                        "outputTokens": 9
                    },
                    "timingInfo": {
                        "clientEndTime": 1741448289691_i64
                    },
                    "text": "hello"
                })
                .to_string(),
            ),
        )
        .unwrap();
        drop(conn);

        let collector = CursorCollector::with_db_path(db_path.clone());
        let records = collector.collect().await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider, "cursor");
        assert_eq!(records[0].model, "gpt-4.1");
        assert_eq!(records[0].input_tokens, 42);
        assert_eq!(records[0].output_tokens, 9);

        let _ = fs::remove_file(db_path);
    }
}

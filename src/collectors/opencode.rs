use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::path::PathBuf;

use super::Collector;
use crate::models::UsageRecord;

pub struct OpenCodeCollector {
    db_path: PathBuf,
}

impl Default for OpenCodeCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeCollector {
    pub fn new() -> Self {
        // OpenCode uses ~/.local/share/opencode/ on all platforms (XDG convention)
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            db_path: home
                .join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db"),
        }
    }
}

/// OpenCode stores messages in SQLite at ~/.local/share/opencode/opencode.db
/// Message data JSON contains: modelID, providerID, cost, tokens { input, output, reasoning, cache { read, write } }
#[derive(Debug, Deserialize)]
struct MessageData {
    #[serde(default, alias = "modelID")]
    model_id: Option<String>,
    #[serde(default, alias = "providerID")]
    provider_id: Option<String>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default)]
    tokens: Option<TokenData>,
    #[serde(default)]
    time: Option<TimeData>,
}

#[derive(Debug, Deserialize)]
struct TokenData {
    #[serde(default)]
    input: i64,
    #[serde(default)]
    output: i64,
    #[serde(default)]
    reasoning: i64,
    #[serde(default)]
    cache: Option<CacheData>,
}

#[derive(Debug, Deserialize)]
struct CacheData {
    #[serde(default)]
    read: i64,
    #[serde(default)]
    write: i64,
}

#[derive(Debug, Deserialize)]
struct TimeData {
    #[serde(default)]
    created: Option<i64>,
}

#[async_trait]
impl Collector for OpenCodeCollector {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        if !self.db_path.exists() {
            return Ok(vec![]);
        }

        let conn =
            rusqlite::Connection::open_with_flags(&self.db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        let collected_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT m.data, m.session_id, m.time_created
             FROM message m
             ORDER BY m.time_created ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        for row in rows {
            let (data_json, session_id, time_created) = row?;

            let msg: MessageData = match serde_json::from_str(&data_json) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if let Some(ref tokens) = msg.tokens {
                if tokens.input == 0 && tokens.output == 0 {
                    continue;
                }

                let model = msg
                    .model_id
                    .unwrap_or_else(|| "unknown".to_string());
                let output = tokens.output + tokens.reasoning;
                let (cache_read, cache_write) = tokens
                    .cache
                    .as_ref()
                    .map(|c| (c.read, c.write))
                    .unwrap_or((0, 0));

                // Use timestamp from message data or fallback to time_created
                let recorded_at = msg
                    .time
                    .and_then(|t| t.created)
                    .or(Some(time_created))
                    .and_then(|ms| {
                        chrono::DateTime::from_timestamp_millis(ms)
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
                    })
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string());

                records.push(UsageRecord {
                    id: None,
                    provider: "opencode".to_string(),
                    model,
                    input_tokens: tokens.input,
                    output_tokens: output,
                    cache_read_tokens: cache_read,
                    cache_write_tokens: cache_write,
                    cost_usd: msg.cost,
                    session_id: Some(session_id.clone()),
                    recorded_at,
                    collected_at: collected_at.clone(),
                    metadata: msg.provider_id.map(|p| format!("{{\"provider\": \"{}\"}}", p)),
                });
            }
        }

        Ok(records)
    }
}

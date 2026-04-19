use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use std::path::PathBuf;

use super::Collector;
use crate::models::UsageRecord;

pub struct OpenCodeCollector {
    db_path: PathBuf,
    /// Only ingest messages with `time_created` strictly greater than this
    /// epoch-millisecond watermark. `None` means a full scan (first sync or
    /// if the watermark cannot be determined). Issue #39.
    since_ms: Option<i64>,
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
            since_ms: None,
        }
    }

    pub fn with_watermark(mut self, since_ms: Option<i64>) -> Self {
        self.since_ms = since_ms;
        self
    }

    #[cfg(test)]
    fn with_db_path(mut self, path: PathBuf) -> Self {
        self.db_path = path;
        self
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

        let conn = rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;

        let collected_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();

        // Incremental sync: skip messages we've already ingested. The watermark
        // is the max recorded_at stored in the llmusage DB for provider='opencode',
        // converted to epoch ms. recorded_at is second-precision so a message
        // that lands within the same second as the watermark will be re-read
        // and filtered by INSERT OR IGNORE downstream — bounded and fine.
        let fetched: Vec<(String, String, i64)> = {
            let map_row = |row: &rusqlite::Row<'_>| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            };
            match self.since_ms {
                Some(since) => {
                    let mut stmt = conn.prepare(
                        "SELECT m.data, m.session_id, m.time_created
                         FROM message m
                         WHERE m.time_created >= ?1
                         ORDER BY m.time_created ASC",
                    )?;
                    let iter = stmt.query_map([since], map_row)?;
                    iter.collect::<rusqlite::Result<Vec<_>>>()?
                }
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT m.data, m.session_id, m.time_created
                         FROM message m
                         ORDER BY m.time_created ASC",
                    )?;
                    let iter = stmt.query_map([], map_row)?;
                    iter.collect::<rusqlite::Result<Vec<_>>>()?
                }
            }
        };

        for (data_json, session_id, time_created) in fetched {
            let msg: MessageData = match serde_json::from_str(&data_json) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if let Some(ref tokens) = msg.tokens {
                if tokens.input == 0 && tokens.output == 0 {
                    continue;
                }

                let model = msg.model_id.unwrap_or_else(|| "unknown".to_string());
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
                    metadata: msg
                        .provider_id
                        .map(|p| serde_json::json!({ "provider": p }).to_string()),
                });
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llmusage-opencode-{label}-{}-{nanos}.sqlite",
            std::process::id()
        ))
    }

    fn build_fixture(path: &PathBuf) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL
            );",
        )
        .unwrap();

        let rows: Vec<(i64, &str)> = vec![
            (
                1_700_000_000_000,
                r#"{"modelID":"m","providerID":"p","tokens":{"input":100,"output":10}}"#,
            ),
            (
                1_700_000_005_000,
                r#"{"modelID":"m","providerID":"p","tokens":{"input":200,"output":20}}"#,
            ),
            (
                1_700_000_010_000,
                r#"{"modelID":"m","providerID":"p","tokens":{"input":300,"output":30}}"#,
            ),
        ];
        for (i, (ts, data)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, 's', ?2, ?3)",
                rusqlite::params![format!("m{i}"), ts, data],
            )
            .unwrap();
        }
    }

    #[tokio::test]
    async fn watermark_skips_older_messages() {
        let path = temp_db_path("watermark");
        build_fixture(&path);

        let c = OpenCodeCollector::new()
            .with_db_path(path.clone())
            .with_watermark(Some(1_700_000_005_000));
        let records = c.collect().await.unwrap();

        assert_eq!(
            records.len(),
            2,
            "watermark should include boundary + newer"
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn no_watermark_reads_everything() {
        let path = temp_db_path("full");
        build_fixture(&path);

        let c = OpenCodeCollector::new().with_db_path(path.clone());
        let records = c.collect().await.unwrap();

        assert_eq!(records.len(), 3);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn watermark_in_future_returns_empty() {
        let path = temp_db_path("future");
        build_fixture(&path);

        let c = OpenCodeCollector::new()
            .with_db_path(path.clone())
            .with_watermark(Some(9_999_999_999_999));
        let records = c.collect().await.unwrap();

        assert!(records.is_empty());
        let _ = std::fs::remove_file(path);
    }
}

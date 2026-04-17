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
struct ComposerDataFull {
    #[serde(default, rename = "modelConfig")]
    model_config: Option<ModelConfig>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<serde_json::Value>,
    #[serde(default, rename = "lastUpdatedAt")]
    last_updated_at: Option<serde_json::Value>,
    #[serde(default)]
    name: Option<String>,
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
}

#[derive(Debug, Deserialize, Clone, Copy, Default)]
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

struct ComposerInfo {
    model: Option<String>,
    created_at_ms: Option<i64>,
    last_updated_at_ms: Option<i64>,
    name: Option<String>,
}

#[derive(Default)]
struct ComposerAggregate {
    input_tokens: i64,
    output_tokens: i64,
    bubble_count: usize,
    nonzero_bubble_count: usize,
    latest_ms: Option<i64>,
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

        let composers = load_composers(&conn)?;
        let aggregates = aggregate_bubbles(&conn)?;

        let mut records: Vec<UsageRecord> = Vec::new();
        let mut zero_token_records: usize = 0;

        for (composer_id, agg) in &aggregates {
            // Skip composers with no bubbles at all — these are empty drafts,
            // not real conversations worth tracking.
            if agg.bubble_count == 0 {
                continue;
            }

            let composer = composers.get(composer_id);
            let raw_model = composer
                .and_then(|c| c.model.clone())
                .filter(|m| !m.trim().is_empty());
            // Cursor uses "default" internally for Auto-mode. Rename so it's
            // readable in reports and doesn't accidentally pick up pricing for
            // an unrelated upstream model called "default".
            let model = match raw_model.as_deref() {
                Some("default") | None => "cursor-default".to_string(),
                Some(other) => other.to_string(),
            };

            let cost_usd = infer_priced_provider(&model).and_then(|provider| {
                costs::calculate_cost(&model, provider, agg.input_tokens, agg.output_tokens, 0, 0)
            });

            if agg.input_tokens == 0 && agg.output_tokens == 0 {
                zero_token_records += 1;
            }

            // Must have a stable timestamp — otherwise dedup against future
            // syncs breaks (collected_at changes every run). Skip composers
            // we can't attribute to a point in time.
            let Some(recorded_at_ms) = composer
                .and_then(|c| c.created_at_ms.or(c.last_updated_at_ms))
                .or(agg.latest_ms)
            else {
                continue;
            };
            let Some(recorded_at) =
                DateTime::from_timestamp_millis(recorded_at_ms).map(|dt| dt.to_rfc3339())
            else {
                continue;
            };

            let metadata = serde_json::to_string(&serde_json::json!({
                "composer_id": composer_id,
                "name": composer.and_then(|c| c.name.clone()),
                "bubble_count": agg.bubble_count,
                "nonzero_token_bubble_count": agg.nonzero_bubble_count,
            }))
            .ok();

            records.push(UsageRecord {
                id: None,
                provider: "cursor".to_string(),
                model,
                input_tokens: agg.input_tokens,
                output_tokens: agg.output_tokens,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_usd,
                session_id: Some(composer_id.clone()),
                recorded_at,
                collected_at: collected_at.clone(),
                metadata,
            });
        }

        // Sort oldest → newest so detail views make chronological sense.
        records.sort_by(|a, b| a.recorded_at.cmp(&b.recorded_at));

        let _ = std::fs::remove_file(temp_db);

        if !records.is_empty() && zero_token_records == records.len() {
            eprintln!(
                "cursor: found {} conversation(s) but none recorded token counts. \
                 Cursor 3.x does not persist token counts locally — usage lives in the \
                 Cursor dashboard. Tokens will show as 0 and costs will be unavailable \
                 until a usage-API integration is added. For older Cursor versions, this \
                 can also mean plan-gate errors blocked the requests.",
                records.len()
            );
        }

        Ok(records)
    }
}

pub fn cursor_state_db_path() -> PathBuf {
    cursor_state_db_path_from_config_dir(&dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")))
}

fn cursor_state_db_path_from_config_dir(config_dir: &Path) -> PathBuf {
    config_dir
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

fn copy_to_temp(source: &Path, suffix: &str) -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Include pid + per-thread + monotonic counter so parallel collects
    // (including parallel tests) never share a temp file.
    let temp_path = std::env::temp_dir().join(format!(
        "llmusage-{}-{:?}-{}-{}",
        std::process::id(),
        std::thread::current().id(),
        n,
        suffix
    ));
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

fn load_composers(conn: &Connection) -> Result<HashMap<String, ComposerInfo>> {
    let mut out = HashMap::new();
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
        let parsed: ComposerDataFull = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let model = parsed
            .model_config
            .and_then(|cfg| cfg.model_name)
            .filter(|name| !name.trim().is_empty());
        let created_at_ms = parse_timestamp_ms(parsed.created_at.as_ref());
        let last_updated_at_ms = parse_timestamp_ms(parsed.last_updated_at.as_ref());
        out.insert(
            composer_id.to_string(),
            ComposerInfo {
                model,
                created_at_ms,
                last_updated_at_ms,
                name: parsed.name,
            },
        );
    }

    Ok(out)
}

fn aggregate_bubbles(conn: &Connection) -> Result<HashMap<String, ComposerAggregate>> {
    let mut out: HashMap<String, ComposerAggregate> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT key, CAST(value AS TEXT)
         FROM cursorDiskKV
         WHERE key LIKE 'bubbleId:%'",
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
        let Some(raw_ids) = key.strip_prefix("bubbleId:") else {
            continue;
        };
        let Some((composer_id, _bubble_id)) = raw_ids.split_once(':') else {
            continue;
        };
        let parsed: BubbleData = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let entry = out.entry(composer_id.to_string()).or_default();
        entry.bubble_count += 1;
        let tokens = parsed.token_count.unwrap_or_default();
        if tokens.input_tokens != 0 || tokens.output_tokens != 0 {
            entry.nonzero_bubble_count += 1;
            entry.input_tokens += tokens.input_tokens;
            entry.output_tokens += tokens.output_tokens;
        }
        if let Some(ts) = bubble_latest_ms(&parsed) {
            entry.latest_ms = Some(entry.latest_ms.map_or(ts, |cur| cur.max(ts)));
        }
    }

    Ok(out)
}

fn bubble_latest_ms(b: &BubbleData) -> Option<i64> {
    let t = b.timing_info.as_ref()?;
    t.client_end_time
        .or(t.client_settle_time)
        .or(t.client_start_time)
}

fn parse_timestamp_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.timestamp_millis()),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_file(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "llmusage-cursor-test-{}-{:?}-{}-{}",
            std::process::id(),
            std::thread::current().id(),
            n,
            name
        ))
    }

    fn insert_composer(conn: &Connection, composer_id: &str, payload: serde_json::Value) {
        conn.execute(
            "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
            (format!("composerData:{}", composer_id), payload.to_string()),
        )
        .unwrap();
    }

    fn insert_bubble(
        conn: &Connection,
        composer_id: &str,
        bubble_id: &str,
        payload: serde_json::Value,
    ) {
        conn.execute(
            "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
            (
                format!("bubbleId:{}:{}", composer_id, bubble_id),
                payload.to_string(),
            ),
        )
        .unwrap();
    }

    fn setup_db(path: &Path) -> Connection {
        let _ = fs::remove_file(path);
        let conn = Connection::open(path).unwrap();
        conn.execute("CREATE TABLE cursorDiskKV (key TEXT, value BLOB)", [])
            .unwrap();
        conn
    }

    #[test]
    fn cursor_state_path_is_relative_to_platform_config_dir() {
        let macos = cursor_state_db_path_from_config_dir(Path::new(
            "/Users/alice/Library/Application Support",
        ));
        assert_eq!(
            macos,
            PathBuf::from(
                "/Users/alice/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
            )
        );

        let linux = cursor_state_db_path_from_config_dir(Path::new("/home/alice/.config"));
        assert_eq!(
            linux,
            PathBuf::from("/home/alice/.config/Cursor/User/globalStorage/state.vscdb")
        );
    }

    #[tokio::test]
    async fn aggregates_tokens_across_bubbles_in_a_composer() {
        let path = temp_file("aggregate.vscdb");
        let conn = setup_db(&path);

        insert_composer(
            &conn,
            "comp-1",
            serde_json::json!({
                "modelConfig": { "modelName": "gpt-4.1" },
                "createdAt": 1_776_400_200_000_i64,
                "name": "My conversation"
            }),
        );
        insert_bubble(
            &conn,
            "comp-1",
            "b1",
            serde_json::json!({
                "type": 2,
                "tokenCount": { "inputTokens": 1000, "outputTokens": 200 },
                "timingInfo": { "clientEndTime": 1_776_400_260_000_i64 }
            }),
        );
        insert_bubble(
            &conn,
            "comp-1",
            "b2",
            serde_json::json!({
                "type": 2,
                "tokenCount": { "inputTokens": 500, "outputTokens": 100 },
                "timingInfo": { "clientEndTime": 1_776_400_300_000_i64 }
            }),
        );
        drop(conn);

        let records = CursorCollector::with_db_path(path.clone())
            .collect()
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.provider, "cursor");
        assert_eq!(r.model, "gpt-4.1");
        assert_eq!(r.input_tokens, 1500);
        assert_eq!(r.output_tokens, 300);
        assert_eq!(r.session_id.as_deref(), Some("comp-1"));
        let meta = r.metadata.as_deref().unwrap_or("");
        assert!(meta.contains("\"bubble_count\":2"));
        assert!(meta.contains("\"nonzero_token_bubble_count\":2"));
        assert!(meta.contains("\"name\":\"My conversation\""));
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn emits_sentinel_record_when_all_bubbles_have_zero_tokens() {
        let path = temp_file("sentinel.vscdb");
        let conn = setup_db(&path);

        insert_composer(
            &conn,
            "comp-z",
            serde_json::json!({
                "modelConfig": { "modelName": "default" },
                "createdAt": 1_776_400_200_000_i64
            }),
        );
        for i in 0..3 {
            insert_bubble(
                &conn,
                "comp-z",
                &format!("b{}", i),
                serde_json::json!({
                    "type": 2,
                    "tokenCount": { "inputTokens": 0, "outputTokens": 0 }
                }),
            );
        }
        drop(conn);

        let records = CursorCollector::with_db_path(path.clone())
            .collect()
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.model, "cursor-default");
        assert_eq!(r.input_tokens, 0);
        assert_eq!(r.output_tokens, 0);
        assert_eq!(r.cost_usd, None);
        let meta = r.metadata.as_deref().unwrap_or("");
        assert!(meta.contains("\"bubble_count\":3"));
        assert!(meta.contains("\"nonzero_token_bubble_count\":0"));
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn skips_composer_with_no_bubbles() {
        let path = temp_file("empty-composer.vscdb");
        let conn = setup_db(&path);

        insert_composer(
            &conn,
            "empty-comp",
            serde_json::json!({
                "modelConfig": { "modelName": "gpt-4.1" },
                "createdAt": 1_776_400_200_000_i64
            }),
        );
        drop(conn);

        let records = CursorCollector::with_db_path(path.clone())
            .collect()
            .await
            .unwrap();
        assert!(records.is_empty());
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn handles_multiple_composers_mixed_tokens() {
        let path = temp_file("mixed.vscdb");
        let conn = setup_db(&path);

        insert_composer(
            &conn,
            "comp-a",
            serde_json::json!({
                "modelConfig": { "modelName": "claude-sonnet-4-6" },
                "createdAt": 1_776_400_100_000_i64
            }),
        );
        insert_bubble(
            &conn,
            "comp-a",
            "b1",
            serde_json::json!({
                "tokenCount": { "inputTokens": 2000, "outputTokens": 500 },
                "timingInfo": { "clientEndTime": 1_776_400_150_000_i64 }
            }),
        );

        insert_composer(
            &conn,
            "comp-b",
            serde_json::json!({
                "modelConfig": { "modelName": "default" },
                "createdAt": 1_776_400_300_000_i64
            }),
        );
        insert_bubble(
            &conn,
            "comp-b",
            "b1",
            serde_json::json!({
                "tokenCount": { "inputTokens": 0, "outputTokens": 0 }
            }),
        );
        drop(conn);

        let records = CursorCollector::with_db_path(path.clone())
            .collect()
            .await
            .unwrap();
        assert_eq!(records.len(), 2);
        // Sorted by recorded_at ascending → comp-a first.
        assert_eq!(records[0].session_id.as_deref(), Some("comp-a"));
        assert_eq!(records[0].model, "claude-sonnet-4-6");
        assert_eq!(records[0].input_tokens, 2000);
        assert_eq!(records[1].session_id.as_deref(), Some("comp-b"));
        assert_eq!(records[1].model, "cursor-default");
        assert_eq!(records[1].input_tokens, 0);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn returns_empty_when_no_cursor_disk_kv_table() {
        let path = temp_file("no-table.vscdb");
        let _ = fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE unrelated (k TEXT)", []).unwrap();
        drop(conn);
        let records = CursorCollector::with_db_path(path.clone())
            .collect()
            .await
            .unwrap();
        assert!(records.is_empty());
        let _ = fs::remove_file(path);
    }
}

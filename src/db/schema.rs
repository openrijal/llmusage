use anyhow::{bail, Result};
use rusqlite::Connection;

const LATEST_SCHEMA_VERSION: i64 = 4;

const CREATE_USAGE_RECORDS_TABLE_SQL: &str = "
    CREATE TABLE IF NOT EXISTS usage_records (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        provider TEXT NOT NULL,
        model TEXT NOT NULL,
        input_tokens INTEGER NOT NULL,
        output_tokens INTEGER NOT NULL,
        cache_read_tokens INTEGER DEFAULT 0,
        cache_write_tokens INTEGER DEFAULT 0,
        cost_usd REAL,
        session_id TEXT,
        recorded_at TEXT NOT NULL,
        collected_at TEXT NOT NULL,
        metadata TEXT
    );
";

const CREATE_INDEXES_SQL: &str = "
    CREATE INDEX IF NOT EXISTS idx_provider ON usage_records(provider);
    CREATE INDEX IF NOT EXISTS idx_recorded_at ON usage_records(recorded_at);
    CREATE INDEX IF NOT EXISTS idx_model ON usage_records(model);
    CREATE INDEX IF NOT EXISTS idx_provider_recorded ON usage_records(provider, recorded_at);
    CREATE UNIQUE INDEX IF NOT EXISTS idx_dedup ON usage_records(
        provider,
        model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        recorded_at,
        COALESCE(session_id, ''),
        COALESCE(cost_usd, -1)
    );
";

pub fn initialize(conn: &Connection) -> Result<()> {
    let current = current_schema_version(conn)?;

    if current > LATEST_SCHEMA_VERSION {
        bail!(
            "Database schema version {} is newer than supported version {}",
            current,
            LATEST_SCHEMA_VERSION
        );
    }

    if current == 0 {
        if table_exists(conn, "usage_records")? {
            migrate(conn, 1, LATEST_SCHEMA_VERSION)?;
        } else {
            let tx = conn.unchecked_transaction()?;
            create_schema_v1(&tx)?;
            set_schema_version(&tx, LATEST_SCHEMA_VERSION)?;
            tx.commit()?;
        }
        return Ok(());
    }

    migrate(conn, current, LATEST_SCHEMA_VERSION)
}

fn current_schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

fn set_schema_version(conn: &Connection, version: i64) -> Result<()> {
    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}

fn create_schema_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(CREATE_USAGE_RECORDS_TABLE_SQL)?;
    conn.execute_batch(CREATE_INDEXES_SQL)?;
    Ok(())
}

fn migrate(conn: &Connection, from: i64, to: i64) -> Result<()> {
    if from > to {
        bail!(
            "Cannot migrate database backwards from version {} to {}",
            from,
            to
        );
    }

    if from == to {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    let mut version = from;

    while version < to {
        match version {
            1 => migrate_v1_to_v2(&tx)?,
            2 => migrate_v2_to_v3(&tx)?,
            3 => migrate_v3_to_v4(&tx)?,
            _ => bail!("No migration path from schema version {}", version),
        }
        version += 1;
        set_schema_version(&tx, version)?;
    }

    tx.commit()?;
    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(CREATE_INDEXES_SQL)?;
    Ok(())
}

// Drop legacy 0-token Ollama heartbeat rows (issues #17/#27). Other providers
// (e.g. cursor) emit intentional zero-token sentinel records, so the cleanup
// is scoped to provider = 'ollama'.
fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM usage_records
         WHERE provider = 'ollama'
           AND input_tokens = 0
           AND output_tokens = 0
           AND cache_read_tokens = 0
           AND cache_write_tokens = 0",
        [],
    )?;
    Ok(())
}

// Rebuild idx_dedup with cost_usd as a trailing column so that two legitimately
// distinct API calls with identical token counts but different costs are no
// longer dropped by INSERT OR IGNORE (issue #50).
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_dedup;
         CREATE UNIQUE INDEX idx_dedup ON usage_records(
             provider,
             model,
             input_tokens,
             output_tokens,
             cache_read_tokens,
             cache_write_tokens,
             recorded_at,
             COALESCE(session_id, ''),
             COALESCE(cost_usd, -1)
         );",
    )?;
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = ?1
        )",
        [name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists != 0)
}

#[cfg(test)]
fn index_exists(conn: &Connection, name: &str) -> Result<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'index' AND name = ?1
        )",
        [name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llmusage-{name}-{}-{nanos}.sqlite",
            std::process::id()
        ))
    }

    #[test]
    fn initializes_fresh_database_to_latest_version() {
        let conn = Connection::open_in_memory().unwrap();

        initialize(&conn).unwrap();

        assert_eq!(
            current_schema_version(&conn).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        assert!(table_exists(&conn, "usage_records").unwrap());
        assert!(index_exists(&conn, "idx_dedup").unwrap());
    }

    #[test]
    fn upgrades_legacy_database_without_data_loss() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema_v1(&conn).unwrap();
        conn.execute(
            "INSERT INTO usage_records (
                provider, model, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, cost_usd, session_id,
                recorded_at, collected_at, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                "openai",
                "gpt-4o-mini",
                10,
                5,
                0,
                0,
                0.12_f64,
                Option::<String>::None,
                "2026-04-18",
                "2026-04-18T12:00:00Z",
                Option::<String>::None,
            ],
        )
        .unwrap();

        assert_eq!(current_schema_version(&conn).unwrap(), 0);

        initialize(&conn).unwrap();

        assert_eq!(
            current_schema_version(&conn).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM usage_records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
        assert!(index_exists(&conn, "idx_provider_recorded").unwrap());
    }

    #[test]
    fn reopening_latest_database_is_a_noop() {
        let path = temp_db_path("schema-reopen");

        {
            let conn = Connection::open(&path).unwrap();
            initialize(&conn).unwrap();
            conn.execute(
                "INSERT INTO usage_records (
                    provider, model, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, cost_usd, session_id,
                    recorded_at, collected_at, metadata
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    "anthropic",
                    "claude-sonnet-4-6",
                    20,
                    8,
                    3,
                    0,
                    0.45_f64,
                    "session-1",
                    "2026-04-18",
                    "2026-04-18T12:30:00Z",
                    "{\"k\":\"v\"}",
                ],
            )
            .unwrap();
        }

        {
            let conn = Connection::open(&path).unwrap();
            initialize(&conn).unwrap();
            assert_eq!(
                current_schema_version(&conn).unwrap(),
                LATEST_SCHEMA_VERSION
            );
            let rows: i64 = conn
                .query_row("SELECT COUNT(*) FROM usage_records", [], |row| row.get(0))
                .unwrap();
            assert_eq!(rows, 1);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migration_failure_rolls_back_version_bump() {
        let conn = Connection::open_in_memory().unwrap();
        set_schema_version(&conn, 1).unwrap();

        let err = initialize(&conn).unwrap_err();

        assert!(err.to_string().contains("usage_records"));
        assert_eq!(current_schema_version(&conn).unwrap(), 1);
        assert!(!index_exists(&conn, "idx_provider").unwrap());
    }

    #[test]
    fn dedup_index_admits_records_differing_only_by_cost() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let insert = "INSERT OR IGNORE INTO usage_records (
            provider, model, input_tokens, output_tokens,
            cache_read_tokens, cache_write_tokens, cost_usd, session_id,
            recorded_at, collected_at, metadata
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";

        let first = conn
            .execute(
                insert,
                rusqlite::params![
                    "anthropic",
                    "claude-sonnet-4-6",
                    100,
                    50,
                    0,
                    0,
                    0.10_f64,
                    Option::<String>::None,
                    "2026-04-18",
                    "2026-04-18T12:00:00Z",
                    Option::<String>::None,
                ],
            )
            .unwrap();
        assert_eq!(first, 1);

        // Same token shape, different cost → should NOT be dropped.
        let second = conn
            .execute(
                insert,
                rusqlite::params![
                    "anthropic",
                    "claude-sonnet-4-6",
                    100,
                    50,
                    0,
                    0,
                    0.20_f64,
                    Option::<String>::None,
                    "2026-04-18",
                    "2026-04-18T12:00:00Z",
                    Option::<String>::None,
                ],
            )
            .unwrap();
        assert_eq!(second, 1);

        // Exact duplicate (same cost too) → dropped.
        let third = conn
            .execute(
                insert,
                rusqlite::params![
                    "anthropic",
                    "claude-sonnet-4-6",
                    100,
                    50,
                    0,
                    0,
                    0.10_f64,
                    Option::<String>::None,
                    "2026-04-18",
                    "2026-04-18T12:00:00Z",
                    Option::<String>::None,
                ],
            )
            .unwrap();
        assert_eq!(third, 0);
    }

    #[test]
    fn upgrades_v3_database_to_v4_with_cost_aware_dedup_index() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema_v1(&conn).unwrap();
        set_schema_version(&conn, 3).unwrap();

        initialize(&conn).unwrap();

        assert_eq!(current_schema_version(&conn).unwrap(), 4);
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='index' AND name='idx_dedup'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("cost_usd"),
            "idx_dedup should include cost_usd after migration, got: {sql}"
        );
    }

    #[test]
    fn rejects_unknown_future_schema_versions() {
        let conn = Connection::open_in_memory().unwrap();
        set_schema_version(&conn, LATEST_SCHEMA_VERSION + 1).unwrap();

        let err = initialize(&conn).unwrap_err();

        assert!(err.to_string().contains("newer than supported"));
    }
}

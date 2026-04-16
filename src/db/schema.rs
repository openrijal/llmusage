use anyhow::Result;
use rusqlite::Connection;

pub fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
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

        CREATE INDEX IF NOT EXISTS idx_provider ON usage_records(provider);
        CREATE INDEX IF NOT EXISTS idx_recorded_at ON usage_records(recorded_at);
        CREATE INDEX IF NOT EXISTS idx_model ON usage_records(model);
        CREATE INDEX IF NOT EXISTS idx_provider_recorded ON usage_records(provider, recorded_at);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_dedup ON usage_records(provider, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, recorded_at, COALESCE(session_id, ''));
        ",
    )?;
    Ok(())
}

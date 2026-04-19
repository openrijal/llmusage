mod queries;
mod schema;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        schema::initialize(&conn)?;
        Ok(Self { conn })
    }

    pub fn insert_record(&self, record: &crate::models::UsageRecord) -> Result<()> {
        queries::insert_record(&self.conn, record)
    }

    pub fn query_summary(
        &self,
        days: u32,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<Vec<crate::models::SummaryRow>> {
        queries::query_summary(&self.conn, days, provider, model)
    }

    pub fn query_daily(
        &self,
        days: u32,
        provider: Option<&str>,
        model: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<Vec<crate::models::DailyRow>> {
        queries::query_daily(&self.conn, days, provider, model, since, until)
    }

    pub fn query_weekly(
        &self,
        weeks: u32,
        provider: Option<&str>,
        model: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<Vec<crate::models::DailyRow>> {
        queries::query_weekly(&self.conn, weeks, provider, model, since, until)
    }

    pub fn query_monthly(
        &self,
        months: u32,
        provider: Option<&str>,
        model: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<Vec<crate::models::DailyRow>> {
        queries::query_monthly(&self.conn, months, provider, model, since, until)
    }

    pub fn query_detail(
        &self,
        model: Option<&str>,
        provider: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::UsageRecord>> {
        queries::query_detail(&self.conn, model, provider, since, until, limit)
    }
}

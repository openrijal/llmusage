use anyhow::Result;
use rusqlite::{params, Connection};

use std::collections::BTreeMap;

use crate::models::{DailyRow, SummaryRow, UsageRecord};

pub fn insert_record(conn: &Connection, record: &UsageRecord) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO usage_records (provider, model, input_tokens, output_tokens,
         cache_read_tokens, cache_write_tokens, cost_usd, session_id,
         recorded_at, collected_at, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            record.provider,
            record.model,
            record.input_tokens,
            record.output_tokens,
            record.cache_read_tokens,
            record.cache_write_tokens,
            record.cost_usd,
            record.session_id,
            record.recorded_at,
            record.collected_at,
            record.metadata,
        ],
    )?;
    Ok(())
}

pub fn query_summary(
    conn: &Connection,
    days: u32,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<SummaryRow>> {
    let mut sql = String::from(
        "SELECT provider, model,
         SUM(input_tokens) as total_input,
         SUM(output_tokens) as total_output,
         SUM(cache_read_tokens) as total_cache_read,
         SUM(cache_write_tokens) as total_cache_write,
         COALESCE(SUM(cost_usd), 0) as total_cost,
         COUNT(*) as record_count
         FROM usage_records
         WHERE recorded_at >= datetime('now', ?1)",
    );

    let days_param = format!("-{} days", days);
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(days_param.clone())];

    if let Some(p) = provider {
        sql.push_str(&format!(" AND provider = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p.to_string()));
    }
    if let Some(m) = model {
        sql.push_str(&format!(" AND model LIKE ?{}", param_values.len() + 1));
        param_values.push(Box::new(format!("%{}%", m)));
    }

    sql.push_str(" GROUP BY provider, model ORDER BY total_cost DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(SummaryRow {
            provider: row.get(0)?,
            model: row.get(1)?,
            total_input: row.get(2)?,
            total_output: row.get(3)?,
            total_cache_read: row.get(4)?,
            total_cache_write: row.get(5)?,
            total_cost: row.get(6)?,
            record_count: row.get(7)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn query_detail(
    conn: &Connection,
    model: Option<&str>,
    provider: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageRecord>> {
    let mut sql = String::from("SELECT * FROM usage_records WHERE 1=1");
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

    if let Some(m) = model {
        sql.push_str(&format!(" AND model LIKE ?{}", param_values.len() + 1));
        param_values.push(Box::new(format!("%{}%", m)));
    }
    if let Some(p) = provider {
        sql.push_str(&format!(" AND provider = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p.to_string()));
    }
    if let Some(s) = since {
        sql.push_str(&format!(" AND recorded_at >= ?{}", param_values.len() + 1));
        param_values.push(Box::new(s.to_string()));
    }
    if let Some(u) = until {
        sql.push_str(&format!(" AND recorded_at <= ?{}", param_values.len() + 1));
        param_values.push(Box::new(u.to_string()));
    }

    sql.push_str(&format!(
        " ORDER BY recorded_at DESC LIMIT {}",
        limit
    ));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(UsageRecord {
            id: row.get(0)?,
            provider: row.get(1)?,
            model: row.get(2)?,
            input_tokens: row.get(3)?,
            output_tokens: row.get(4)?,
            cache_read_tokens: row.get(5)?,
            cache_write_tokens: row.get(6)?,
            cost_usd: row.get(7)?,
            session_id: row.get(8)?,
            recorded_at: row.get(9)?,
            collected_at: row.get(10)?,
            metadata: row.get(11)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn query_daily(
    conn: &Connection,
    days: u32,
    provider: Option<&str>,
) -> Result<Vec<DailyRow>> {
    query_grouped(conn, "DATE(recorded_at)", &format!("-{} days", days), provider)
}

pub fn query_weekly(
    conn: &Connection,
    weeks: u32,
    provider: Option<&str>,
) -> Result<Vec<DailyRow>> {
    // strftime %W = week number, %Y = year → group by year-week
    query_grouped(
        conn,
        "strftime('%Y-W%W', recorded_at)",
        &format!("-{} days", weeks * 7),
        provider,
    )
}

pub fn query_monthly(
    conn: &Connection,
    months: u32,
    provider: Option<&str>,
) -> Result<Vec<DailyRow>> {
    query_grouped(
        conn,
        "strftime('%Y-%m', recorded_at)",
        &format!("-{} months", months),
        provider,
    )
}

fn query_grouped(
    conn: &Connection,
    group_expr: &str,
    lookback: &str,
    provider: Option<&str>,
) -> Result<Vec<DailyRow>> {
    let mut sql = format!(
        "SELECT {group_expr} as period, model,
         SUM(input_tokens) as total_input,
         SUM(output_tokens) as total_output,
         COALESCE(SUM(cost_usd), 0) as total_cost
         FROM usage_records
         WHERE recorded_at >= datetime('now', ?1)"
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(lookback.to_string())];

    if let Some(p) = provider {
        sql.push_str(&format!(" AND provider = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p.to_string()));
    }

    sql.push_str(" GROUP BY period, model ORDER BY period ASC, total_cost DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;

    let mut map: BTreeMap<String, DailyRow> = BTreeMap::new();

    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    for row in rows {
        let (period, model, input, output, cost) = row?;
        let entry = map.entry(period.clone()).or_insert_with(|| DailyRow {
            date: period,
            models: Vec::new(),
            total_input: 0,
            total_output: 0,
            total_cost: 0.0,
        });
        if !entry.models.contains(&model) {
            entry.models.push(model);
        }
        entry.total_input += input;
        entry.total_output += output;
        entry.total_cost += cost;
    }

    Ok(map.into_values().collect())
}

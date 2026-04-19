use anyhow::Result;
use rusqlite::{params, Connection};

use std::collections::BTreeMap;

use crate::models::{DailyRow, ModelEntry, SummaryRow, UsageRecord};

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
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(days_param.clone())];

    if let Some(p) = provider {
        sql.push_str(&format!(" AND provider = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p.to_string()));
    }
    if let Some(m) = model {
        sql.push_str(&format!(" AND model LIKE ?{}", param_values.len() + 1));
        param_values.push(Box::new(format!("%{}%", m)));
    }

    sql.push_str(" GROUP BY provider, model ORDER BY total_cost DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
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
    limit: Option<usize>,
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
        // When the user passes a bare date (YYYY-MM-DD), append end-of-day so
        // timestamped records on that date are included.
        let until_val =
            if u.len() == 10 && u.chars().nth(4) == Some('-') && u.chars().nth(7) == Some('-') {
                format!("{}T23:59:59", u)
            } else {
                u.to_string()
            };
        param_values.push(Box::new(until_val));
    }

    sql.push_str(" ORDER BY recorded_at DESC");
    if let Some(n) = limit {
        sql.push_str(&format!(" LIMIT {}", n));
    }

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
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
    model: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<DailyRow>> {
    query_grouped(
        conn,
        "DATE(recorded_at)",
        &format!("-{} days", days),
        provider,
        model,
        since,
        until,
    )
}

pub fn query_weekly(
    conn: &Connection,
    weeks: u32,
    provider: Option<&str>,
    model: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<DailyRow>> {
    // SQLite's `%V`/`%G` strftime modifiers were added in SQLite 3.46 (May 2024).
    // The bundled build in `rusqlite = 0.31` ships SQLite 3.45, where those
    // modifiers silently return NULL and break row decoding. Instead, query
    // per-day rows (portable on every SQLite version) and rebucket by ISO
    // week in Rust using chrono's `iso_week`.
    let daily = query_grouped(
        conn,
        "DATE(recorded_at)",
        &format!("-{} days", weeks * 7),
        provider,
        model,
        since,
        until,
    )?;
    Ok(rebucket_daily_by_iso_week(daily))
}

fn rebucket_daily_by_iso_week(daily: Vec<DailyRow>) -> Vec<DailyRow> {
    use chrono::{Datelike, NaiveDate};

    // week_label -> (provider, model) -> accumulated ModelEntry
    let mut buckets: BTreeMap<String, BTreeMap<(String, String), ModelEntry>> = BTreeMap::new();

    for row in daily {
        let Ok(date) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") else {
            // DATE(recorded_at) always yields YYYY-MM-DD; skip defensively if not.
            continue;
        };
        let iw = date.iso_week();
        let label = format!("{}-W{:02}", iw.year(), iw.week());
        let week = buckets.entry(label).or_default();
        for entry in row.model_entries {
            let key = (entry.provider.clone(), entry.model.clone());
            let agg = week.entry(key).or_insert_with(|| ModelEntry {
                provider: entry.provider.clone(),
                model: entry.model.clone(),
                input_tokens: 0,
                output_tokens: 0,
                cost: 0.0,
            });
            agg.input_tokens += entry.input_tokens;
            agg.output_tokens += entry.output_tokens;
            agg.cost += entry.cost;
        }
    }

    let mut out: Vec<DailyRow> = Vec::with_capacity(buckets.len());
    for (label, entries_by_model) in buckets {
        let mut row = DailyRow {
            date: label,
            models: Vec::new(),
            model_entries: Vec::new(),
            total_input: 0,
            total_output: 0,
            total_cost: 0.0,
        };
        for (_, entry) in entries_by_model {
            if !row.models.contains(&entry.model) {
                row.models.push(entry.model.clone());
            }
            row.total_input += entry.input_tokens;
            row.total_output += entry.output_tokens;
            row.total_cost += entry.cost;
            row.model_entries.push(entry);
        }
        out.push(row);
    }
    out
}

pub fn query_monthly(
    conn: &Connection,
    months: u32,
    provider: Option<&str>,
    model: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<DailyRow>> {
    query_grouped(
        conn,
        "strftime('%Y-%m', recorded_at)",
        &format!("-{} months", months),
        provider,
        model,
        since,
        until,
    )
}

fn query_grouped(
    conn: &Connection,
    group_expr: &str,
    lookback: &str,
    provider: Option<&str>,
    model: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<DailyRow>> {
    let mut sql = format!(
        "SELECT {group_expr} as period, provider, model,
         SUM(input_tokens) as total_input,
         SUM(output_tokens) as total_output,
         COALESCE(SUM(cost_usd), 0) as total_cost
         FROM usage_records
         WHERE 1=1"
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    // Explicit --since overrides the rolling lookback; otherwise apply the
    // lookback window computed from --days/--weeks/--months.
    if let Some(s) = since {
        sql.push_str(&format!(" AND recorded_at >= ?{}", param_values.len() + 1));
        param_values.push(Box::new(s.to_string()));
    } else {
        sql.push_str(&format!(
            " AND recorded_at >= datetime('now', ?{})",
            param_values.len() + 1
        ));
        param_values.push(Box::new(lookback.to_string()));
    }

    if let Some(u) = until {
        sql.push_str(&format!(" AND recorded_at <= ?{}", param_values.len() + 1));
        let until_val =
            if u.len() == 10 && u.chars().nth(4) == Some('-') && u.chars().nth(7) == Some('-') {
                format!("{}T23:59:59", u)
            } else {
                u.to_string()
            };
        param_values.push(Box::new(until_val));
    }

    if let Some(p) = provider {
        sql.push_str(&format!(" AND provider = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p.to_string()));
    }

    if let Some(m) = model {
        sql.push_str(&format!(" AND model LIKE ?{}", param_values.len() + 1));
        param_values.push(Box::new(format!("%{}%", m)));
    }

    sql.push_str(
        " GROUP BY period, provider, model ORDER BY period ASC, provider ASC, total_cost DESC",
    );

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;

    let mut map: BTreeMap<String, DailyRow> = BTreeMap::new();

    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, f64>(5)?,
        ))
    })?;

    for row in rows {
        let (period, prov, model, input, output, cost) = row?;
        let entry = map.entry(period.clone()).or_insert_with(|| DailyRow {
            date: period,
            models: Vec::new(),
            model_entries: Vec::new(),
            total_input: 0,
            total_output: 0,
            total_cost: 0.0,
        });
        if !entry.models.contains(&model) {
            entry.models.push(model.clone());
        }
        entry.model_entries.push(ModelEntry {
            provider: prov,
            model,
            input_tokens: input,
            output_tokens: output,
            cost,
        });
        entry.total_input += input;
        entry.total_output += output;
        entry.total_cost += cost;
    }

    Ok(map.into_values().collect())
}

#[cfg(test)]
mod iso_week_tests {
    use super::*;

    fn day(date: &str, model: &str, input: i64, output: i64, cost: f64) -> DailyRow {
        DailyRow {
            date: date.to_string(),
            models: vec![model.to_string()],
            model_entries: vec![ModelEntry {
                provider: "p".to_string(),
                model: model.to_string(),
                input_tokens: input,
                output_tokens: output,
                cost,
            }],
            total_input: input,
            total_output: output,
            total_cost: cost,
        }
    }

    #[test]
    fn groups_days_within_same_iso_week() {
        // 2026-04-13 (Mon) through 2026-04-19 (Sun) = ISO week 16 of 2026.
        let daily = vec![
            day("2026-04-13", "m", 10, 5, 0.10),
            day("2026-04-15", "m", 20, 5, 0.20),
            day("2026-04-19", "m", 30, 10, 0.30),
        ];
        let weekly = rebucket_daily_by_iso_week(daily);
        assert_eq!(weekly.len(), 1);
        assert_eq!(weekly[0].date, "2026-W16");
        assert_eq!(weekly[0].total_input, 60);
        assert_eq!(weekly[0].total_output, 20);
        assert!((weekly[0].total_cost - 0.60).abs() < 1e-9);
    }

    #[test]
    fn splits_across_iso_weeks_at_monday_boundary() {
        // Sunday 2026-04-12 = W15; Monday 2026-04-13 = W16.
        let daily = vec![
            day("2026-04-12", "m", 10, 0, 0.10),
            day("2026-04-13", "m", 20, 0, 0.20),
        ];
        let weekly = rebucket_daily_by_iso_week(daily);
        assert_eq!(weekly.len(), 2);
        assert_eq!(weekly[0].date, "2026-W15");
        assert_eq!(weekly[1].date, "2026-W16");
    }

    #[test]
    fn january_first_2027_is_w53_of_2026() {
        // 2027-01-01 is Friday; ISO week 53 of 2026.
        let daily = vec![day("2027-01-01", "m", 1, 1, 0.01)];
        let weekly = rebucket_daily_by_iso_week(daily);
        assert_eq!(weekly[0].date, "2026-W53");
    }

    #[test]
    fn merges_same_model_across_days_into_one_entry() {
        let daily = vec![
            day("2026-04-13", "m", 10, 5, 0.10),
            day("2026-04-15", "m", 20, 5, 0.20),
        ];
        let weekly = rebucket_daily_by_iso_week(daily);
        assert_eq!(weekly[0].model_entries.len(), 1);
        let entry = &weekly[0].model_entries[0];
        assert_eq!(entry.input_tokens, 30);
        assert_eq!(entry.output_tokens, 10);
    }
}

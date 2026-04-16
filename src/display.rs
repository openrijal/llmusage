use crate::models::{DailyRow, ModelPricing, SummaryRow, UsageRecord};
use colored::Colorize;
use std::collections::BTreeMap;
use tabled::{
    settings::{Alignment, Modify, Style, object::Columns},
    Table, Tabled,
};

#[derive(Tabled)]
struct SummaryDisplay {
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "Input Tokens")]
    input: String,
    #[tabled(rename = "Output Tokens")]
    output: String,
    #[tabled(rename = "Cost (USD)")]
    cost: String,
    #[tabled(rename = "Records")]
    count: String,
}

#[derive(Tabled)]
struct DetailDisplay {
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "In")]
    input: String,
    #[tabled(rename = "Out")]
    output: String,
    #[tabled(rename = "Cost")]
    cost: String,
}

#[derive(Tabled)]
struct ModelDisplay {
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "Input $/MTok")]
    input_price: String,
    #[tabled(rename = "Output $/MTok")]
    output_price: String,
    #[tabled(rename = "Cache Read")]
    cache_read: String,
    #[tabled(rename = "Cache Write")]
    cache_write: String,
}

pub fn print_summary(rows: &[SummaryRow]) {
    if rows.is_empty() {
        println!("{}", "No usage data found.".dimmed());
        return;
    }

    let display_rows: Vec<SummaryDisplay> = rows
        .iter()
        .map(|r| SummaryDisplay {
            provider: r.provider.clone(),
            model: shorten_model(&r.model),
            input: format_tokens_comma(r.total_input),
            output: format_tokens_comma(r.total_output),
            cost: format_cost(r.total_cost),
            count: r.record_count.to_string(),
        })
        .collect();

    let total_cost: f64 = rows.iter().map(|r| r.total_cost).sum();
    let total_input: i64 = rows.iter().map(|r| r.total_input).sum();
    let total_output: i64 = rows.iter().map(|r| r.total_output).sum();

    let table = Table::new(&display_rows)
        .with(Style::rounded())
        .with(Modify::new(Columns::new(2..=4)).with(Alignment::right()))
        .to_string();

    // Color the header line (first content line after the top border)
    print_table_colored(&table);
    println!();
    println!(
        "  {} {} input, {} output, {}",
        "Totals:".yellow().bold(),
        format_tokens_comma(total_input).yellow(),
        format_tokens_comma(total_output).yellow(),
        format_cost(total_cost).yellow().bold(),
    );
}

/// A display row: either a provider aggregate line or an individual model line.
struct DisplayRow {
    label: String,
    input: String,
    output: String,
    cost: String,
    kind: RowKind,
}

enum RowKind {
    /// Provider aggregate row (shown in magenta with totals)
    Provider,
    /// Individual model row (indented, shows per-model values)
    Model,
    /// Dotted separator between models within a provider
    DottedSep,
}

/// Build display rows for one period, grouped by provider.
/// Provider rows show aggregate input/output/cost for all models under that provider.
/// Model rows show per-model breakdown.
/// When show_all is false, models with zero input AND zero output are filtered out.
fn build_period_rows(row: &DailyRow, show_all: bool) -> Vec<DisplayRow> {
    // Group model entries by provider, preserving order
    let mut by_provider: BTreeMap<String, Vec<&crate::models::ModelEntry>> = BTreeMap::new();
    for entry in &row.model_entries {
        if !show_all && entry.input_tokens == 0 && entry.output_tokens == 0 && entry.cost == 0.0 {
            continue;
        }
        by_provider
            .entry(entry.provider.clone())
            .or_default()
            .push(entry);
    }

    let mut rows = Vec::new();
    for (provider, models) in &by_provider {
        // Provider aggregate
        let prov_input: i64 = models.iter().map(|m| m.input_tokens).sum();
        let prov_output: i64 = models.iter().map(|m| m.output_tokens).sum();
        let prov_cost: f64 = models.iter().map(|m| m.cost).sum();

        rows.push(DisplayRow {
            label: provider.clone(),
            input: format_tokens_comma(prov_input),
            output: format_tokens_comma(prov_output),
            cost: format_cost(prov_cost),
            kind: RowKind::Provider,
        });

        for (i, entry) in models.iter().enumerate() {
            // Dotted separator between models (not before first)
            if i > 0 {
                rows.push(DisplayRow {
                    label: String::new(),
                    input: String::new(),
                    output: String::new(),
                    cost: String::new(),
                    kind: RowKind::DottedSep,
                });
            }
            rows.push(DisplayRow {
                label: format!("  {}", shorten_model(&entry.model)),
                input: format_tokens_comma(entry.input_tokens),
                output: format_tokens_comma(entry.output_tokens),
                cost: format_cost(entry.cost),
                kind: RowKind::Model,
            });
        }
    }
    rows
}

/// Filter DailyRow model_entries based on the --all flag.
/// When show_all is false, entries with zero input, zero output, AND zero cost are removed.
pub fn filter_daily_rows(rows: &[DailyRow], show_all: bool) -> Vec<DailyRow> {
    if show_all {
        return rows.to_vec();
    }
    rows.iter()
        .map(|r| {
            let filtered: Vec<_> = r.model_entries.iter()
                .filter(|e| e.input_tokens != 0 || e.output_tokens != 0 || e.cost != 0.0)
                .cloned()
                .collect();
            let models: Vec<String> = filtered.iter()
                .map(|e| e.model.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            DailyRow {
                date: r.date.clone(),
                models,
                model_entries: filtered,
                total_input: r.total_input,
                total_output: r.total_output,
                total_cost: r.total_cost,
            }
        })
        .collect()
}

pub fn print_daily(rows: &[DailyRow], title: &str, show_all: bool) {
    if rows.is_empty() {
        println!("{}", "No usage data found.".dimmed());
        return;
    }

    println!();
    println!("  {}", title.bold());
    println!();

    let total_cost: f64 = rows.iter().map(|r| r.total_cost).sum();
    let total_input: i64 = rows.iter().map(|r| r.total_input).sum();
    let total_output: i64 = rows.iter().map(|r| r.total_output).sum();

    // Pre-compute all period rows and formatted values to determine column widths
    let mut all_period_rows: Vec<Vec<DisplayRow>> = Vec::new();
    let mut all_dates: Vec<String> = Vec::new();

    for r in rows {
        all_period_rows.push(build_period_rows(r, show_all));
        all_dates.push(format_period(&r.date));
    }

    let total_input_str = format_tokens_comma(total_input);
    let total_output_str = format_tokens_comma(total_output);
    let total_cost_str = format_cost(total_cost);

    // Calculate dynamic column widths (content width, excluding borders and padding)
    let col_date = all_dates
        .iter()
        .map(|d| d.len())
        .max()
        .unwrap_or(4)
        .max("Date".len())
        .max("Total".len());

    let col_models = all_period_rows
        .iter()
        .flat_map(|prows| prows.iter().map(|r| r.label.len()))
        .max()
        .unwrap_or(6)
        .max("Models".len());

    let col_input = all_period_rows
        .iter()
        .flat_map(|prows| prows.iter().map(|r| r.input.len()))
        .max()
        .unwrap_or(5)
        .max("Input".len())
        .max(total_input_str.len());

    let col_output = all_period_rows
        .iter()
        .flat_map(|prows| prows.iter().map(|r| r.output.len()))
        .max()
        .unwrap_or(6)
        .max("Output".len())
        .max(total_output_str.len());

    let col_cost = all_period_rows
        .iter()
        .flat_map(|prows| prows.iter().map(|r| r.cost.len()))
        .max()
        .unwrap_or(10)
        .max("Cost (USD)".len())
        .max(total_cost_str.len());

    // Add padding (1 space each side)
    let w_date = col_date + 2;
    let w_models = col_models + 2;
    let w_input = col_input + 2;
    let w_output = col_output + 2;
    let w_cost = col_cost + 2;

    let top = format!(
        "┌{:─>w1$}┬{:─>w2$}┬{:─>w3$}┬{:─>w4$}┬{:─>w5$}┐",
        "", "", "", "", "",
        w1 = w_date, w2 = w_models, w3 = w_input, w4 = w_output, w5 = w_cost,
    );
    let sep = format!(
        "├{:─>w1$}┼{:─>w2$}┼{:─>w3$}┼{:─>w4$}┼{:─>w5$}┤",
        "", "", "", "", "",
        w1 = w_date, w2 = w_models, w3 = w_input, w4 = w_output, w5 = w_cost,
    );
    let bot = format!(
        "└{:─>w1$}┴{:─>w2$}┴{:─>w3$}┴{:─>w4$}┴{:─>w5$}┘",
        "", "", "", "", "",
        w1 = w_date, w2 = w_models, w3 = w_input, w4 = w_output, w5 = w_cost,
    );
    // Dotted separator for between models within a provider
    let dotted = format!(
        "│ {:<cw$} │ {:·>mw$} │ {:·>iw$} │ {:·>ow$} │ {:·>kw$} │",
        "", "", "", "", "",
        cw = col_date, mw = col_models, iw = col_input, ow = col_output, kw = col_cost,
    );

    println!("{}", top);

    // Header
    println!(
        "│ {:<cw$} │ {:<mw$} │ {:>iw$} │ {:>ow$} │ {:>kw$} │",
        "Date".cyan().bold(),
        "Models".cyan().bold(),
        "Input".cyan().bold(),
        "Output".cyan().bold(),
        "Cost (USD)".cyan().bold(),
        cw = col_date,
        mw = col_models,
        iw = col_input,
        ow = col_output,
        kw = col_cost,
    );
    println!("{}", sep);

    // Data rows
    for (row_idx, _) in rows.iter().enumerate() {
        // Add separator between periods (but not before the first row)
        if row_idx > 0 {
            println!("{}", sep);
        }

        let period_rows = &all_period_rows[row_idx];
        let date = &all_dates[row_idx];

        for (line_idx, drow) in period_rows.iter().enumerate() {
            if matches!(drow.kind, RowKind::DottedSep) {
                println!("{}", dotted.dimmed());
                continue;
            }

            let d = if line_idx == 0 { date.as_str() } else { "" };

            let model_padded = format!("{:<width$}", drow.label, width = col_models);
            let model_display = if matches!(drow.kind, RowKind::Provider) {
                model_padded.magenta().to_string()
            } else {
                model_padded
            };

            let input_plain = format!("{:>width$}", drow.input, width = col_input);
            let output_plain = format!("{:>width$}", drow.output, width = col_output);
            let cost_plain = format!("{:>width$}", drow.cost, width = col_cost);

            // Dim model-level values to distinguish from provider aggregates
            let (input_disp, output_disp, cost_disp) = if matches!(drow.kind, RowKind::Model) {
                (
                    input_plain.dimmed().to_string(),
                    output_plain.dimmed().to_string(),
                    cost_plain.dimmed().to_string(),
                )
            } else {
                (input_plain, output_plain, cost_plain)
            };

            println!(
                "│ {:<cw$} │ {} │ {} │ {} │ {} │",
                d,
                model_display,
                input_disp,
                output_disp,
                cost_disp,
                cw = col_date,
            );
        }
    }

    // Totals row
    println!("{}", sep);

    let total_label = format!("{:<width$}", "Total", width = col_date);
    let empty_models = format!("{:<width$}", "", width = col_models);
    let total_in = format!("{:>width$}", total_input_str, width = col_input);
    let total_out = format!("{:>width$}", total_output_str, width = col_output);
    let total_c = format!("{:>width$}", total_cost_str, width = col_cost);

    println!(
        "│ {} │ {} │ {} │ {} │ {} │",
        total_label.yellow().bold(),
        empty_models,
        total_in.yellow(),
        total_out.yellow(),
        total_c.yellow().bold(),
    );
    println!("{}", bot);
}

pub fn print_detail(rows: &[UsageRecord]) {
    if rows.is_empty() {
        println!("{}", "No usage records found.".dimmed());
        return;
    }

    let display_rows: Vec<DetailDisplay> = rows
        .iter()
        .map(|r| DetailDisplay {
            date: r.recorded_at[..10].to_string(),
            provider: r.provider.clone(),
            model: shorten_model(&r.model),
            input: format_tokens_comma(r.input_tokens),
            output: format_tokens_comma(r.output_tokens),
            cost: r
                .cost_usd
                .map(format_cost)
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    let table = Table::new(&display_rows)
        .with(Style::rounded())
        .with(Modify::new(Columns::new(3..=5)).with(Alignment::right()))
        .to_string();

    print_table_colored(&table);
    println!("  {} records", rows.len());
}

pub fn print_models(models: &[ModelPricing]) {
    let display_rows: Vec<ModelDisplay> = models
        .iter()
        .map(|m| ModelDisplay {
            provider: m.provider.clone(),
            model: m.model.clone(),
            input_price: format!("${:.2}", m.input_per_mtok),
            output_price: format!("${:.2}", m.output_per_mtok),
            cache_read: m
                .cache_read_per_mtok
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "-".to_string()),
            cache_write: m
                .cache_write_per_mtok
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    let table = Table::new(&display_rows)
        .with(Style::rounded())
        .with(Modify::new(Columns::new(2..)).with(Alignment::right()))
        .to_string();

    print_table_colored(&table);
}

pub fn to_csv(rows: &[UsageRecord]) -> anyhow::Result<String> {
    let mut out = String::from(
        "provider,model,input_tokens,output_tokens,cache_read,cache_write,cost_usd,recorded_at\n",
    );
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            r.provider,
            r.model,
            r.input_tokens,
            r.output_tokens,
            r.cache_read_tokens,
            r.cache_write_tokens,
            r.cost_usd.unwrap_or(0.0),
            r.recorded_at,
        ));
    }
    Ok(out)
}

pub fn to_json(rows: &[UsageRecord]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(rows)?)
}

/// Print a tabled table with the header row colored cyan
fn print_table_colored(table_str: &str) {
    let lines: Vec<&str> = table_str.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i == 1 {
            // Header row (line after top border)
            println!("{}", line.cyan().bold());
        } else {
            println!("{}", line);
        }
    }
}

/// Format tokens with comma separators (e.g., 1,234,567)
fn format_tokens_comma(n: i64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

fn format_cost(c: f64) -> String {
    if c >= 1.0 {
        format!("${:.2}", c)
    } else if c >= 0.01 {
        format!("${:.3}", c)
    } else if c > 0.0 {
        format!("${:.4}", c)
    } else {
        "$0.00".to_string()
    }
}

/// Abbreviate model names:
/// "claude-opus-4-6-20260205" -> "opus-4-6"
/// "claude-sonnet-4-20250514" -> "sonnet-4"
/// "claude-haiku-4-5-20251001" -> "haiku-4-5"
/// "antigravity-gemini-3-flash" -> "gemini-3-flash"
/// "antigravity-claude-opus-4-5-thinking" -> "opus-4-5-thinking"
/// "gpt-4o-mini" -> "gpt-4o-mini"
fn shorten_model(model: &str) -> String {
    // Strip antigravity- prefix first
    let m = model.strip_prefix("antigravity-").unwrap_or(model);
    let m = m.strip_prefix("claude-").unwrap_or(m);
    let m = strip_date_suffix(m);
    let m = m.strip_prefix("4-").unwrap_or(m);
    m.to_string()
}

/// Format period label for display (single line).
/// "2026-04-15" (daily) -> "2026-04-15"
/// "2026-W15" (weekly) -> "2026 W15"
/// "2026-04" (monthly) -> "2026-04"
fn format_period(s: &str) -> String {
    if s.contains("-W") {
        s.replace('-', " ")
    } else {
        s.to_string()
    }
}

/// Remove trailing date patterns like -20260205
fn strip_date_suffix(s: &str) -> &str {
    if s.len() > 9 {
        let potential = &s[s.len() - 9..];
        if potential.starts_with('-') && potential[1..].chars().all(|c| c.is_ascii_digit()) {
            return &s[..s.len() - 9];
        }
    }
    s
}

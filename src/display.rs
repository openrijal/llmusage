use crate::models::{DailyRow, ModelPricing, SummaryRow, UsageRecord};
use colored::Colorize;
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

pub fn print_daily(rows: &[DailyRow], title: &str) {
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

    // Build table manually for colored header/totals and wider columns
    let col_date = 12;
    let col_models = 24;
    let col_input = 12;
    let col_output = 12;
    let col_cost = 14;

    let sep = format!(
        "├{:─>w1$}┼{:─>w2$}┼{:─>w3$}┼{:─>w4$}┼{:─>w5$}┤",
        "", "", "", "", "",
        w1 = col_date, w2 = col_models, w3 = col_input, w4 = col_output, w5 = col_cost,
    );
    let top = format!(
        "┌{:─>w1$}┬{:─>w2$}┬{:─>w3$}┬{:─>w4$}┬{:─>w5$}┐",
        "", "", "", "", "",
        w1 = col_date, w2 = col_models, w3 = col_input, w4 = col_output, w5 = col_cost,
    );
    let bot = format!(
        "└{:─>w1$}┴{:─>w2$}┴{:─>w3$}┴{:─>w4$}┴{:─>w5$}┘",
        "", "", "", "", "",
        w1 = col_date, w2 = col_models, w3 = col_input, w4 = col_output, w5 = col_cost,
    );

    println!("{}", top);

    // Header
    println!(
        "│{:<w1$}│{:<w2$}│{:>w3$}│{:>w4$}│{:>w5$}│",
        " Date".cyan().bold(),
        " Models".cyan().bold(),
        format!("{} ", "Input").cyan().bold(),
        format!("{} ", "Output").cyan().bold(),
        format!("{} ", "Cost (USD)").cyan().bold(),
        w1 = col_date, w2 = col_models, w3 = col_input, w4 = col_output, w5 = col_cost,
    );
    println!("{}", sep);

    // Data rows
    for r in rows {
        let models: Vec<String> = r.models.iter().map(|m| shorten_model(m)).collect();
        let date = format_period(&r.date);
        let date_lines: Vec<&str> = date.split('\n').collect();
        let model_lines: Vec<String> = if models.is_empty() {
            vec![String::new()]
        } else {
            models.iter().map(|m| format!("- {}", m)).collect()
        };

        let max_lines = date_lines.len().max(model_lines.len());

        for line_idx in 0..max_lines {
            let d = date_lines.get(line_idx).unwrap_or(&"");
            let m = model_lines
                .get(line_idx)
                .map(|s| s.as_str())
                .unwrap_or("");

            if line_idx == 0 {
                println!(
                    "│ {:<w1$}│ {:<w2$}│{:>w3$} │{:>w4$} │{:>w5$} │",
                    d,
                    m,
                    format_tokens_comma(r.total_input),
                    format_tokens_comma(r.total_output),
                    format_cost(r.total_cost),
                    w1 = col_date - 2,
                    w2 = col_models - 2,
                    w3 = col_input - 2,
                    w4 = col_output - 2,
                    w5 = col_cost - 2,
                );
            } else {
                println!(
                    "│ {:<w1$}│ {:<w2$}│{:>w3$}│{:>w4$}│{:>w5$}│",
                    d, m, "", "", "",
                    w1 = col_date - 2,
                    w2 = col_models - 2,
                    w3 = col_input,
                    w4 = col_output,
                    w5 = col_cost,
                );
            }
        }
    }

    // Totals row
    println!("{}", sep);
    println!(
        "│ {:<w1$}│{:<w2$}│{:>w3$} │{:>w4$} │{:>w5$} │",
        "Total".yellow().bold(),
        "",
        format_tokens_comma(total_input).yellow(),
        format_tokens_comma(total_output).yellow(),
        format_cost(total_cost).yellow().bold(),
        w1 = col_date - 2,
        w2 = col_models,
        w3 = col_input - 2,
        w4 = col_output - 2,
        w5 = col_cost - 2,
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

/// Abbreviate model names like ccusage does:
/// "claude-opus-4-6-20260205" -> "opus-4-6"
/// "claude-sonnet-4-20250514" -> "sonnet-4"
/// "claude-haiku-4-5-20251001" -> "haiku-4-5"
/// "gpt-4o-mini" -> "gpt-4o-mini"
fn shorten_model(model: &str) -> String {
    let m = model.strip_prefix("claude-").unwrap_or(model);
    let m = strip_date_suffix(m);
    let m = m.strip_prefix("4-").unwrap_or(m);
    m.to_string()
}

/// Format period label for display.
/// "2026-04-15" (daily) -> "2026\n04-15"
/// "2026-W15" (weekly) -> "2026\nW15"
/// "2026-04" (monthly) -> "2026-04"
fn format_period(s: &str) -> String {
    if s.len() == 10 && s.chars().nth(4) == Some('-') {
        format!("{}\n{}", &s[..4], &s[5..])
    } else if s.contains("-W") {
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        if parts.len() == 2 {
            format!("{}\n{}", parts[0], parts[1])
        } else {
            s.to_string()
        }
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

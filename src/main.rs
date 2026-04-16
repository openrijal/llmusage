mod collectors;
mod config;
mod costs;
mod db;
mod display;
mod models;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "llmusage")]
#[command(about = "Track token usage and costs across AI providers")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync usage data from configured providers
    Sync {
        /// Specific provider to sync (default: all configured)
        #[arg(short, long)]
        provider: Option<String>,
    },
    /// Show usage summary
    Summary {
        /// Number of days to look back (default: 30)
        #[arg(short, long, default_value = "30")]
        days: u32,
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
        /// Filter by model
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Show daily usage breakdown
    Daily {
        /// Number of days to look back (default: 90)
        #[arg(short, long, default_value = "90")]
        days: u32,
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show all models including those with zero tokens
        #[arg(long)]
        all: bool,
    },
    /// Show weekly usage breakdown
    Weekly {
        /// Number of weeks to look back (default: 12)
        #[arg(short, long, default_value = "12")]
        weeks: u32,
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show all models including those with zero tokens
        #[arg(long)]
        all: bool,
    },
    /// Show monthly usage breakdown
    Monthly {
        /// Number of months to look back (default: 6)
        #[arg(short, long, default_value = "6")]
        months: u32,
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show all models including those with zero tokens
        #[arg(long)]
        all: bool,
    },
    /// Show detailed usage breakdown
    Detail {
        /// Filter by model
        #[arg(short, long)]
        model: Option<String>,
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
        /// Start date (YYYY-MM-DD)
        #[arg(short, long)]
        since: Option<String>,
        /// End date (YYYY-MM-DD)
        #[arg(short, long)]
        until: Option<String>,
        /// Max rows to display
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },
    /// List known models and their pricing
    Models {
        /// Filter by provider
        #[arg(short = 'P', long)]
        provider: Option<String>,
    },
    /// Manage configuration
    Config {
        /// Set a config value (KEY=VALUE)
        #[arg(short, long)]
        set: Option<String>,
        /// Show current config
        #[arg(short, long)]
        list: bool,
    },
    /// Update model pricing from LiteLLM
    UpdatePricing,
    /// Export usage data
    Export {
        /// Output format
        #[arg(short, long, default_value = "csv")]
        format: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
        /// Number of days to export
        #[arg(short, long, default_value = "30")]
        days: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load_config()?;
    let db = db::Database::open(&cfg.db_path)?;

    match cli.command {
        Commands::Sync { provider } => {
            cmd_sync(&cfg, &db, provider.as_deref()).await?;
        }
        Commands::Summary {
            days,
            provider,
            model,
        } => {
            cmd_summary(&db, days, provider.as_deref(), model.as_deref())?;
        }
        Commands::Daily {
            days,
            provider,
            json,
            all,
        } => {
            cmd_daily(&db, days, provider.as_deref(), json, all)?;
        }
        Commands::Weekly {
            weeks,
            provider,
            json,
            all,
        } => {
            cmd_weekly(&db, weeks, provider.as_deref(), json, all)?;
        }
        Commands::Monthly {
            months,
            provider,
            json,
            all,
        } => {
            cmd_monthly(&db, months, provider.as_deref(), json, all)?;
        }
        Commands::Detail {
            model,
            provider,
            since,
            until,
            limit,
        } => {
            cmd_detail(
                &db,
                model.as_deref(),
                provider.as_deref(),
                since.as_deref(),
                until.as_deref(),
                limit,
            )?;
        }
        Commands::Models { provider } => {
            cmd_models(provider.as_deref())?;
        }
        Commands::UpdatePricing => {
            cmd_update_pricing().await?;
        }
        Commands::Config { set, list } => {
            cmd_config(&cfg, set.as_deref(), list)?;
        }
        Commands::Export {
            format,
            output,
            days,
        } => {
            cmd_export(&db, &format, output.as_deref(), days)?;
        }
    }

    Ok(())
}

async fn cmd_sync(
    cfg: &config::Config,
    db: &db::Database,
    provider_filter: Option<&str>,
) -> Result<()> {
    use colored::Colorize;

    // Auto-fetch pricing on first sync if no cache exists
    let cache = dirs::cache_dir()
        .unwrap_or_default()
        .join("llmusage")
        .join("litellm_pricing.json");
    if !cache.exists() {
        print!("First run: fetching model pricing... ");
        match costs::update_pricing_cache().await {
            Ok(_) => println!("{}", "ok".green()),
            Err(e) => println!("{}: {} (using fallback)", "warn".yellow(), e),
        }
    }

    let providers = collectors::get_collectors(cfg, provider_filter)?;

    if providers.is_empty() {
        println!(
            "{}",
            "No providers configured. Run `llmusage config --set anthropic_api_key=sk-...`"
                .yellow()
        );
        return Ok(());
    }

    for collector in &providers {
        let name = collector.name();
        print!("Syncing {}... ", name.cyan());
        match collector.collect().await {
            Ok(records) => {
                let count = records.len();
                for record in records {
                    db.insert_record(&record)?;
                }
                println!("{} ({} records)", "ok".green(), count);
            }
            Err(e) => {
                println!("{}: {}", "error".red(), e);
            }
        }
    }

    Ok(())
}

fn cmd_summary(
    db: &db::Database,
    days: u32,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    let rows = db.query_summary(days, provider, model)?;
    display::print_summary(&rows);
    Ok(())
}

fn cmd_daily(db: &db::Database, days: u32, provider: Option<&str>, json: bool, show_all: bool) -> Result<()> {
    let rows = db.query_daily(days, provider)?;
    if json {
        let filtered = display::filter_daily_rows(&rows, show_all);
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else {
        display::print_daily(&rows, "Token Usage Report — Daily", show_all);
    }
    Ok(())
}

fn cmd_weekly(db: &db::Database, weeks: u32, provider: Option<&str>, json: bool, show_all: bool) -> Result<()> {
    let rows = db.query_weekly(weeks, provider)?;
    if json {
        let filtered = display::filter_daily_rows(&rows, show_all);
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else {
        display::print_daily(&rows, "Token Usage Report — Weekly", show_all);
    }
    Ok(())
}

fn cmd_monthly(db: &db::Database, months: u32, provider: Option<&str>, json: bool, show_all: bool) -> Result<()> {
    let rows = db.query_monthly(months, provider)?;
    if json {
        let filtered = display::filter_daily_rows(&rows, show_all);
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else {
        display::print_daily(&rows, "Token Usage Report — Monthly", show_all);
    }
    Ok(())
}

fn cmd_detail(
    db: &db::Database,
    model: Option<&str>,
    provider: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
    limit: usize,
) -> Result<()> {
    let rows = db.query_detail(model, provider, since, until, limit)?;
    display::print_detail(&rows);
    Ok(())
}

fn cmd_models(provider: Option<&str>) -> Result<()> {
    let models = costs::get_model_pricing(provider);
    display::print_models(&models);
    Ok(())
}

fn cmd_config(cfg: &config::Config, set: Option<&str>, _list: bool) -> Result<()> {
    if let Some(kv) = set {
        let (key, value) = kv
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Expected KEY=VALUE format"))?;
        config::set_config_value(cfg, key.trim(), value.trim())?;
        println!("Set {} = {}", key.trim(), value.trim());
    } else {
        config::print_config(cfg);
    }
    Ok(())
}

async fn cmd_update_pricing() -> Result<()> {
    use colored::Colorize;
    print!("Fetching pricing from LiteLLM... ");
    costs::update_pricing_cache().await?;
    println!("{}", "ok".green());
    let models = costs::get_model_pricing(None);
    println!("Cached pricing for {} models", models.len());
    Ok(())
}

fn cmd_export(
    db: &db::Database,
    format: &str,
    output: Option<&str>,
    days: u32,
) -> Result<()> {
    let since = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let since_str = since.format("%Y-%m-%d").to_string();
    let rows = db.query_detail(None, None, Some(&since_str), None, 100_000)?;
    let content = match format {
        "json" => display::to_json(&rows)?,
        _ => display::to_csv(&rows)?,
    };

    match output {
        Some(path) => {
            std::fs::write(path, &content)?;
            println!("Exported {} records to {}", rows.len(), path);
        }
        None => print!("{}", content),
    }

    Ok(())
}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    #[serde(default)]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub gemini_api_key: Option<String>,
    #[serde(default)]
    pub ollama_host: Option<String>,
    #[serde(default = "default_true")]
    pub claude_code_enabled: bool,
    #[serde(skip)]
    pub config_path: PathBuf,
}

fn default_db_path() -> String {
    config_dir()
        .join("llmusage.db")
        .to_string_lossy()
        .to_string()
}

fn default_true() -> bool {
    true
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("llmusage")
}

fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_config() -> Result<Config> {
    let path = config_file();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let mut cfg: Config = toml::from_str(&content)?;
        cfg.config_path = path;
        Ok(cfg)
    } else {
        Ok(Config {
            db_path: default_db_path(),
            anthropic_api_key: None,
            openai_api_key: None,
            gemini_api_key: None,
            ollama_host: None,
            claude_code_enabled: true,
            config_path: path,
        })
    }
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let dir = cfg.config_path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    let content = toml::to_string_pretty(cfg)?;
    std::fs::write(&cfg.config_path, content)?;
    Ok(())
}

pub fn set_config_value(cfg: &Config, key: &str, value: &str) -> Result<()> {
    let mut cfg = cfg.clone();
    match key {
        "anthropic_api_key" => cfg.anthropic_api_key = Some(value.to_string()),
        "openai_api_key" => cfg.openai_api_key = Some(value.to_string()),
        "gemini_api_key" => cfg.gemini_api_key = Some(value.to_string()),
        "ollama_host" => cfg.ollama_host = Some(value.to_string()),
        "claude_code_enabled" => cfg.claude_code_enabled = value.parse()?,
        "db_path" => cfg.db_path = value.to_string(),
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
    save_config(&cfg)?;
    Ok(())
}

pub fn print_config(cfg: &Config) {
    use colored::Colorize;

    println!("{}", "llmusage configuration".bold());
    println!("  config: {}", cfg.config_path.display());
    println!("  db:     {}", cfg.db_path);
    println!();
    println!("{}", "Providers:".bold());
    println!(
        "  anthropic:    {}",
        if cfg.anthropic_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  openai:       {}",
        if cfg.openai_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  gemini:       {}",
        if cfg.gemini_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  ollama:       {}",
        if cfg.ollama_host.is_some() {
            cfg.ollama_host.as_deref().unwrap().to_string().green()
        } else {
            "not set (default: http://localhost:11434)".dimmed()
        }
    );
    println!(
        "  claude_code:  {}",
        if cfg.claude_code_enabled {
            "enabled".green()
        } else {
            "disabled".dimmed()
        }
    );
}

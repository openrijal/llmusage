pub mod anthropic;
pub mod claude_code;
pub mod codex;
pub mod gemini;
pub mod gemini_cli;
pub mod ollama;
pub mod openai;
pub mod opencode;

use anyhow::Result;
use async_trait::async_trait;

use crate::config::Config;
use crate::models::UsageRecord;

#[async_trait]
pub trait Collector: Send + Sync {
    fn name(&self) -> &str;
    async fn collect(&self) -> Result<Vec<UsageRecord>>;
}

pub fn get_collectors(
    cfg: &Config,
    provider_filter: Option<&str>,
) -> Result<Vec<Box<dyn Collector>>> {
    let mut collectors: Vec<Box<dyn Collector>> = Vec::new();

    let should_include = |name: &str| provider_filter.is_none() || provider_filter == Some(name);

    // API-based collectors (require explicit config)
    if should_include("anthropic") {
        if let Some(ref key) = cfg.anthropic_api_key {
            collectors.push(Box::new(anthropic::AnthropicCollector::new(key.clone())));
        }
    }

    if should_include("openai") {
        if let Some(ref key) = cfg.openai_api_key {
            collectors.push(Box::new(openai::OpenAICollector::new(key.clone())));
        }
    }

    if should_include("gemini") {
        if let Some(ref key) = cfg.gemini_api_key {
            collectors.push(Box::new(gemini::GeminiCollector::new(key.clone())));
        }
    }

    if should_include("ollama") {
        let host = cfg
            .ollama_host
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        collectors.push(Box::new(ollama::OllamaCollector::new(host)));
    }

    // Local log-based collectors (auto-detect if installed)
    if should_include("claude_code") && cfg.claude_code_enabled {
        collectors.push(Box::new(claude_code::ClaudeCodeCollector::new()));
    }

    if should_include("codex") {
        let codex_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".codex")
            .join("archived_sessions");
        if codex_dir.exists() {
            collectors.push(Box::new(codex::CodexCollector::new()));
        }
    }

    if should_include("opencode") {
        let db_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".local/share/opencode/opencode.db");
        if db_path.exists() {
            collectors.push(Box::new(opencode::OpenCodeCollector::new()));
        }
    }

    if should_include("gemini_cli") {
        let home = dirs::home_dir().unwrap_or_default();
        let jsonl_dir = home.join(".gemini").join("tmp");
        let legacy_dir = home.join(".gemini").join("antigravity").join("conversations");
        if jsonl_dir.exists() || legacy_dir.exists() {
            collectors.push(Box::new(gemini_cli::GeminiCliCollector::new()));
        }
    }

    Ok(collectors)
}

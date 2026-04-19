pub mod anthropic;
pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod deepseek;
pub mod gemini;
pub mod gemini_cli;
pub mod ollama;
pub mod openai;
pub mod opencode;
pub mod openrouter;

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

use crate::config::Config;
use crate::models::UsageRecord;

#[async_trait]
pub trait Collector: Send + Sync {
    fn name(&self) -> &str;
    async fn collect(&self) -> Result<Vec<UsageRecord>>;
}

pub struct LocalCollectorStatus {
    pub name: &'static str,
    pub state: LocalCollectorState,
    pub path: PathBuf,
    pub note: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub enum LocalCollectorState {
    Detected,
    NotFound,
    Unsupported,
}

pub fn get_collectors(
    cfg: &Config,
    provider_filter: Option<&str>,
) -> Result<Vec<Box<dyn Collector>>> {
    let provider_filter = provider_filter.map(canonical_provider_name);
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

    if should_include("openrouter") {
        if let Some(ref key) = cfg.openrouter_api_key {
            collectors.push(Box::new(openrouter::OpenRouterCollector::new(key.clone())));
        }
    }

    if should_include("deepseek") {
        if let Some(ref key) = cfg.deepseek_api_key {
            collectors.push(Box::new(deepseek::DeepSeekCollector::new(key.clone())));
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
        let codex_dir = codex_sessions_dir();
        if codex_dir.exists() {
            collectors.push(Box::new(codex::CodexCollector::new()));
        }
    }

    if should_include("opencode") {
        let db_path = opencode_db_path();
        if db_path.exists() {
            collectors.push(Box::new(opencode::OpenCodeCollector::new()));
        }
    }

    if should_include("gemini_cli") {
        let jsonl_dir = gemini_cli_jsonl_dir();
        let legacy_dir = gemini_cli_legacy_dir();
        if jsonl_dir.exists() || legacy_dir.exists() {
            collectors.push(Box::new(gemini_cli::GeminiCliCollector::new()));
        }
    }

    if should_include("cursor") {
        let db_path = cursor::cursor_state_db_path();
        if db_path.exists() {
            collectors.push(Box::new(cursor::CursorCollector::new()));
        }
    }

    Ok(collectors)
}

pub fn local_collector_statuses() -> Vec<LocalCollectorStatus> {
    let gemini_dir = gemini_cli_jsonl_dir();
    let gemini_legacy_dir = gemini_cli_legacy_dir();
    let gemini_jsonl_detected = gemini_dir.exists();
    let gemini_legacy_detected = gemini_legacy_dir.exists();

    vec![
        LocalCollectorStatus {
            name: "codex",
            state: if codex_sessions_dir().exists() {
                LocalCollectorState::Detected
            } else {
                LocalCollectorState::NotFound
            },
            path: codex_sessions_dir(),
            note: None,
        },
        LocalCollectorStatus {
            name: "opencode",
            state: if opencode_db_path().exists() {
                LocalCollectorState::Detected
            } else {
                LocalCollectorState::NotFound
            },
            path: opencode_db_path(),
            note: None,
        },
        LocalCollectorStatus {
            name: "gemini_cli",
            state: if gemini_jsonl_detected {
                LocalCollectorState::Detected
            } else if gemini_legacy_detected {
                LocalCollectorState::Unsupported
            } else {
                LocalCollectorState::NotFound
            },
            path: if gemini_jsonl_detected {
                gemini_dir
            } else {
                gemini_legacy_dir
            },
            note: if gemini_legacy_detected && !gemini_jsonl_detected {
                Some("legacy Antigravity .pb sessions are not parseable in strict mode")
            } else {
                None
            },
        },
        LocalCollectorStatus {
            name: "cursor",
            state: if cursor::cursor_state_db_path().exists() {
                LocalCollectorState::Detected
            } else {
                LocalCollectorState::NotFound
            },
            path: cursor::cursor_state_db_path(),
            note: None,
        },
        LocalCollectorStatus {
            name: "windsurf",
            state: if windsurf_state_db_path().exists() {
                LocalCollectorState::Unsupported
            } else {
                LocalCollectorState::NotFound
            },
            path: windsurf_state_db_path(),
            note: Some("local data lacks reliable token counts"),
        },
        LocalCollectorStatus {
            name: "vscode",
            state: if vscode_state_db_path().exists() {
                LocalCollectorState::Unsupported
            } else {
                LocalCollectorState::NotFound
            },
            path: vscode_state_db_path(),
            note: Some("installed AI extensions lack local token counts"),
        },
    ]
}

pub fn canonical_provider_name(name: &str) -> &str {
    match name {
        "antigravity" | "gemini-cli" => "gemini_cli",
        "vscode-copilot-chat" => "vscode",
        _ => name,
    }
}

pub fn explain_provider_filter(cfg: &Config, provider_filter: &str) -> String {
    match canonical_provider_name(provider_filter) {
        "anthropic" => {
            if cfg.anthropic_api_key.is_some() {
                "Provider 'anthropic' is configured but returned no active collector.".to_string()
            } else {
                "Provider 'anthropic' is supported but requires `anthropic_api_key` in config."
                    .to_string()
            }
        }
        "openai" => {
            if cfg.openai_api_key.is_some() {
                "Provider 'openai' is configured but returned no active collector.".to_string()
            } else {
                "Provider 'openai' is supported but requires `openai_api_key` in config."
                    .to_string()
            }
        }
        "gemini" => {
            if cfg.gemini_api_key.is_some() {
                "Provider 'gemini' is configured, but the Gemini API collector remains unimplemented."
                    .to_string()
            } else {
                "Provider 'gemini' is supported only as a stub and also requires `gemini_api_key` in config."
                    .to_string()
            }
        }
        "openrouter" => {
            if cfg.openrouter_api_key.is_some() {
                "Provider 'openrouter' is configured but returned no active collector.".to_string()
            } else {
                "Provider 'openrouter' is supported but requires `openrouter_api_key` in config."
                    .to_string()
            }
        }
        "deepseek" => {
            if cfg.deepseek_api_key.is_some() {
                "Provider 'deepseek' is configured, but DeepSeek does not expose a historical usage API; the collector only validates the key."
                    .to_string()
            } else {
                "Provider 'deepseek' is supported but requires `deepseek_api_key` in config."
                    .to_string()
            }
        }
        "ollama" => "Provider 'ollama' is supported, but no collector could be activated.".to_string(),
        "claude_code" => {
            if cfg.claude_code_enabled {
                "Provider 'claude_code' is enabled, but no local session logs were found."
                    .to_string()
            } else {
                "Provider 'claude_code' is supported but currently disabled in config."
                    .to_string()
            }
        }
        "codex" => {
            "Provider 'codex' is supported but no `~/.codex/archived_sessions` logs were found."
                .to_string()
        }
        "opencode" => {
            "Provider 'opencode' is supported but no local OpenCode database was found."
                .to_string()
        }
        "gemini_cli" => {
            if gemini_cli_jsonl_dir().exists() {
                "Provider 'gemini_cli' is supported but no collector could be activated."
                    .to_string()
            } else if gemini_cli_legacy_dir().exists() {
                "Provider 'antigravity' legacy `.pb` sessions are present, but strict mode requires parseable token counts; only Gemini CLI JSONL sessions are supported."
                    .to_string()
            } else {
                "Provider 'gemini_cli' is supported but no local Gemini CLI session data was found."
                    .to_string()
            }
        }
        "cursor" => {
            "Provider 'cursor' is supported but no local Cursor state database was found."
                .to_string()
        }
        "windsurf" => {
            "Provider 'windsurf' is not supported in strict mode because local artifacts do not expose reliable token counts."
                .to_string()
        }
        "vscode" => {
            "Provider 'vscode' is not supported in strict mode because available extension data lacks token counts."
                .to_string()
        }
        other => format!(
            "Unknown provider '{}'. Known providers: anthropic, openai, gemini, openrouter, deepseek, ollama, claude_code, codex, opencode, gemini_cli, antigravity, cursor, windsurf, vscode",
            other
        ),
    }
}

fn codex_sessions_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".codex")
        .join("archived_sessions")
}

fn opencode_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".local/share/opencode/opencode.db")
}

fn gemini_cli_jsonl_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".gemini")
        .join("tmp")
}

fn gemini_cli_legacy_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".gemini")
        .join("antigravity")
        .join("conversations")
}

fn windsurf_state_db_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Windsurf")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

fn vscode_state_db_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Code")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb")
}

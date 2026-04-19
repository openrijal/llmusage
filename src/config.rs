use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    pub openrouter_api_key: Option<String>,
    #[serde(default)]
    pub deepseek_api_key: Option<String>,
    #[serde(default)]
    pub ollama_host: Option<String>,
    #[serde(default)]
    pub ollama_enabled: bool,
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
    let mut cfg = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let mut cfg: Config = toml::from_str(&content)?;
        cfg.config_path = path.clone();
        cfg
    } else {
        Config {
            db_path: default_db_path(),
            anthropic_api_key: None,
            openai_api_key: None,
            gemini_api_key: None,
            openrouter_api_key: None,
            deepseek_api_key: None,
            ollama_host: None,
            ollama_enabled: false,
            claude_code_enabled: true,
            config_path: path.clone(),
        }
    };

    apply_env_overrides(&mut cfg);

    if path.exists() {
        tighten_config_permissions(&path)?;
    }

    Ok(cfg)
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let dir = cfg
        .config_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config_path has no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    let content = toml::to_string_pretty(cfg)?;
    std::fs::write(&cfg.config_path, content)?;
    tighten_config_permissions(&cfg.config_path)?;
    Ok(())
}

pub fn set_config_value(cfg: &Config, key: &str, value: &str) -> Result<()> {
    let mut cfg = cfg.clone();
    match key {
        "anthropic_api_key" => cfg.anthropic_api_key = Some(value.to_string()),
        "openai_api_key" => cfg.openai_api_key = Some(value.to_string()),
        "gemini_api_key" => cfg.gemini_api_key = Some(value.to_string()),
        "openrouter_api_key" => cfg.openrouter_api_key = Some(value.to_string()),
        "deepseek_api_key" => cfg.deepseek_api_key = Some(value.to_string()),
        "ollama_host" => cfg.ollama_host = Some(value.to_string()),
        "ollama_enabled" => cfg.ollama_enabled = value.parse()?,
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
        if env_var_present("ANTHROPIC_API_KEY") {
            "configured (env)".green()
        } else if cfg.anthropic_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  openai:       {}",
        if env_var_present("OPENAI_API_KEY") {
            "configured (env)".green()
        } else if cfg.openai_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  gemini:       {}",
        if env_var_present("GEMINI_API_KEY") {
            "configured (env)".green()
        } else if cfg.gemini_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  openrouter:   {}",
        if env_var_present("OPENROUTER_API_KEY") {
            "configured (env)".green()
        } else if cfg.openrouter_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    println!(
        "  deepseek:     {}",
        if env_var_present("DEEPSEEK_API_KEY") {
            "configured (env)".green()
        } else if cfg.deepseek_api_key.is_some() {
            "configured".green()
        } else {
            "not set".dimmed()
        }
    );
    let ollama_host_display = cfg
        .ollama_host
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    println!(
        "  ollama:       {}",
        if !cfg.ollama_enabled {
            "disabled (set ollama_enabled=true to include in sync)".dimmed()
        } else if env_var_present("OLLAMA_HOST") {
            format!("enabled (env: {})", ollama_host_display).green()
        } else {
            format!("enabled ({})", ollama_host_display).green()
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

fn apply_env_overrides(cfg: &mut Config) {
    if let Some(value) = env_var_value("ANTHROPIC_API_KEY") {
        cfg.anthropic_api_key = Some(value);
    }
    if let Some(value) = env_var_value("OPENAI_API_KEY") {
        cfg.openai_api_key = Some(value);
    }
    if let Some(value) = env_var_value("GEMINI_API_KEY") {
        cfg.gemini_api_key = Some(value);
    }
    if let Some(value) = env_var_value("OPENROUTER_API_KEY") {
        cfg.openrouter_api_key = Some(value);
    }
    if let Some(value) = env_var_value("DEEPSEEK_API_KEY") {
        cfg.deepseek_api_key = Some(value);
    }
    if let Some(value) = env_var_value("OLLAMA_HOST") {
        cfg.ollama_host = Some(value);
    }
}

fn env_var_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_var_present(name: &str) -> bool {
    env_var_value(name).is_some()
}

#[cfg(unix)]
fn tighten_config_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn tighten_config_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Tests that mutate process-wide environment variables must not run in
    // parallel — cargo test runs tests in multiple threads by default, and
    // overlapping set_var calls produce nondeterministic results.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llmusage-{name}-{}-{nanos}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn env_overrides_file_values() {
        let _guard = env_lock();
        let _anthropic = EnvGuard::set("ANTHROPIC_API_KEY", "env-anthropic");
        let _openai = EnvGuard::set("OPENAI_API_KEY", "env-openai");
        let _gemini = EnvGuard::set("GEMINI_API_KEY", "env-gemini");
        let _ollama = EnvGuard::set("OLLAMA_HOST", "http://env-host");

        let mut cfg = Config {
            db_path: "db.sqlite".to_string(),
            anthropic_api_key: Some("file-anthropic".to_string()),
            openai_api_key: Some("file-openai".to_string()),
            gemini_api_key: Some("file-gemini".to_string()),
            openrouter_api_key: None,
            deepseek_api_key: None,
            ollama_host: Some("http://file-host".to_string()),
            ollama_enabled: false,
            claude_code_enabled: true,
            config_path: PathBuf::from("config.toml"),
        };

        apply_env_overrides(&mut cfg);

        assert_eq!(cfg.anthropic_api_key.as_deref(), Some("env-anthropic"));
        assert_eq!(cfg.openai_api_key.as_deref(), Some("env-openai"));
        assert_eq!(cfg.gemini_api_key.as_deref(), Some("env-gemini"));
        assert_eq!(cfg.ollama_host.as_deref(), Some("http://env-host"));
    }

    #[test]
    fn empty_env_vars_do_not_override_file_values() {
        let _guard = env_lock();
        let _openai = EnvGuard::set("OPENAI_API_KEY", "");

        let mut cfg = Config {
            db_path: "db.sqlite".to_string(),
            anthropic_api_key: None,
            openai_api_key: Some("file-openai".to_string()),
            gemini_api_key: None,
            openrouter_api_key: None,
            deepseek_api_key: None,
            ollama_host: None,
            ollama_enabled: false,
            claude_code_enabled: true,
            config_path: PathBuf::from("config.toml"),
        };

        apply_env_overrides(&mut cfg);

        assert_eq!(cfg.openai_api_key.as_deref(), Some("file-openai"));
    }

    #[test]
    #[cfg(unix)]
    fn save_config_restricts_permissions_to_owner() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("save-config");
        let cfg = Config {
            db_path: "db.sqlite".to_string(),
            anthropic_api_key: Some("secret".to_string()),
            openai_api_key: None,
            gemini_api_key: None,
            openrouter_api_key: None,
            deepseek_api_key: None,
            ollama_host: None,
            ollama_enabled: false,
            claude_code_enabled: true,
            config_path: path.clone(),
        };

        save_config(&cfg).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    #[cfg(unix)]
    fn load_config_tightens_existing_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("load-config");
        std::fs::write(
            &path,
            r#"
db_path = "db.sqlite"
openai_api_key = "secret"
"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let mut cfg: Config = toml::from_str(&content).unwrap();
        cfg.config_path = path.clone();
        apply_env_overrides(&mut cfg);
        tighten_config_permissions(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = std::fs::remove_file(path);
    }
}

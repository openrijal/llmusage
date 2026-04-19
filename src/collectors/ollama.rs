use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Collector;
use crate::models::UsageRecord;

pub struct OllamaCollector {
    host: String,
    client: reqwest::Client,
}

impl OllamaCollector {
    pub fn new(host: String) -> Self {
        Self {
            host,
            client: reqwest::Client::new(),
        }
    }
}

// Print the limitations note at most once per process so repeated `sync`
// invocations from a long-running watcher don't spam the user.
static WARNED: AtomicBool = AtomicBool::new(false);

#[async_trait]
impl Collector for OllamaCollector {
    fn name(&self) -> &str {
        "ollama"
    }

    /// Ollama does not persist per-request token usage anywhere we can read
    /// after the fact. Investigation summary (issue #17):
    ///
    ///   * `/api/ps` and `/api/tags` only describe loaded/available models.
    ///     They expose no historical token counts.
    ///   * `/api/generate` and `/api/chat` responses include `eval_count` and
    ///     `prompt_eval_count`, but only for the in-flight request. Capturing
    ///     them requires sitting in the request path (proxy or client wrapper).
    ///   * Server logs (`~/.ollama/logs/server.log`, journalctl on Linux) at
    ///     `OLLAMA_LOG_LEVEL=info` log requests but not token counts. Bumping
    ///     to `debug` is noisy and still does not expose `eval_count` in a
    ///     structured form across versions.
    ///   * Ollama has no hook/plugin system today.
    ///
    /// Conclusion: the only viable approaches are out-of-process — run a
    /// logging proxy in front of Ollama (see README) or have the calling
    /// client emit usage records directly. Until one of those is wired up,
    /// this collector confirms reachability and emits zero records rather than
    /// inserting useless 0-token heartbeat rows (issue #27).
    async fn collect(&self) -> Result<Vec<UsageRecord>> {
        let resp = self
            .client
            .get(format!("{}/api/ps", self.host))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if !WARNED.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "ollama: reachable at {} but token usage is not tracked. \
                         Ollama does not persist per-request usage; run a logging \
                         proxy in front of it to capture token counts.",
                        self.host
                    );
                }
                Ok(vec![])
            }
            Ok(r) => {
                anyhow::bail!("Ollama returned status {}", r.status());
            }
            Err(_) => {
                anyhow::bail!(
                    "Could not connect to Ollama at {}. Is it running?",
                    self.host
                );
            }
        }
    }
}

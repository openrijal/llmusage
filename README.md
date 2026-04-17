[![CI](https://github.com/openrijal/llmusage/actions/workflows/ci.yml/badge.svg)](https://github.com/openrijal/llmusage/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/llmusage)](https://crates.io/crates/llmusage)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/llmusage?label=downloads)](https://crates.io/crates/llmusage)

# llmusage

Track token usage and costs across AI providers and coding tools from a single CLI.

## What it does

llmusage collects usage data from multiple AI sources — API dashboards, local session logs — normalizes it into a unified SQLite database, and provides fast CLI queries for reporting and cost analysis.

**Supported providers:**

| Provider | Type | Config required |
|----------|------|-----------------|
| Claude Code | Local logs (`~/.claude/projects/`) | None (auto-detect) |
| Codex | Local logs (`~/.codex/archived_sessions/`) | None (auto-detect) |
| Cursor | Local SQLite (`~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`) | None (auto-detect) |
| OpenCode | Local SQLite (`~/.local/share/opencode/opencode.db`) | None (auto-detect) |
| Gemini CLI / Antigravity | Local logs (`~/.gemini/tmp/`) | None (auto-detect) |
| Anthropic API | REST API | `anthropic_api_key` |
| OpenAI API | REST API | `openai_api_key` |
| Gemini API | REST API | `gemini_api_key` (stub) |
| Ollama | REST API | None (defaults to `localhost:11434`) |

**Strict-mode limitations:**

- `windsurf` is intentionally not supported yet. The local artifacts investigated on macOS did not expose reliable token counts.
- `vscode` is intentionally not supported as a generic collector. Installed AI extensions exposed session/model metadata, but not token counts.
- `antigravity` is accepted as a provider alias for `gemini_cli`. Legacy Antigravity `.pb` sessions remain unsupported; only Gemini CLI JSONL sessions are collected.

## Installation

### Prerequisites

You need the [Rust toolchain](https://rustup.rs/) installed for source-based installation methods. No other system dependencies are required — SQLite is bundled.

### From crates.io (recommended)

```bash
cargo install llmusage
```

### From Homebrew (macOS)

```bash
brew install openrijal/tap/llmusage
```

### Pre-built binaries

Download the latest binary for your platform from [GitHub Releases](https://github.com/openrijal/llmusage/releases).

Available for: macOS (Intel & Apple Silicon), Linux (x86_64, ARM64).

### Build from source

```bash
git clone https://github.com/openrijal/llmusage.git
cd llmusage
cargo build --release
# Binary at ./target/release/llmusage
```

Or install directly from the local checkout:

```bash
cargo install --path .
```

Verify the installation:

```bash
llmusage --version
```

## Quick start

```bash
# Configure an API provider (optional — local tools are auto-detected)
llmusage config --set anthropic_api_key=sk-ant-...

# Sync usage data from all configured/detected providers
llmusage sync

# View a summary of the last 30 days
llmusage summary

# Daily breakdown for the last 90 days
llmusage daily
```

## Commands

### Sync

Pull usage data from all configured and auto-detected providers.

```bash
llmusage sync                    # all providers
llmusage sync --provider claude_code  # specific provider only
llmusage sync --provider antigravity  # alias for gemini_cli
```

### Summary

Aggregated usage by provider and model.

```bash
llmusage summary                 # last 30 days
llmusage summary --days 7        # last 7 days
llmusage summary --provider anthropic
llmusage summary --model opus
```

### Daily / Weekly / Monthly

Time-series usage breakdowns.

```bash
llmusage daily                   # last 90 days
llmusage daily --days 30 --json  # JSON output
llmusage weekly --weeks 12
llmusage monthly --months 6 --provider openai
```

### Detail

Per-record breakdown with filtering.

```bash
llmusage detail                          # last 50 records
llmusage detail --model opus --limit 100
llmusage detail --since 2025-01-01 --until 2025-01-31
llmusage detail --provider claude_code
```

### Models

List known models and their pricing (per-million tokens).

```bash
llmusage models
llmusage models --provider anthropic
```

### Export

Export usage data as CSV or JSON.

```bash
llmusage export                          # CSV to stdout
llmusage export --format json --output usage.json
llmusage export --days 7 --output week.csv
```

### Configuration

```bash
llmusage config                          # show current config
llmusage config --set anthropic_api_key=sk-ant-...
llmusage config --set openai_api_key=sk-...
llmusage config --set ollama_host=http://192.168.1.10:11434
llmusage config --set claude_code_enabled=false
```

`llmusage config --list` also shows auto-detected local collectors and strict-mode unsupported local IDE tooling such as Windsurf and VS Code.

### Update pricing

Refresh the LiteLLM pricing cache (900+ models).

```bash
llmusage update-pricing
```

Pricing is auto-fetched on first sync. The cache is stored at `~/.cache/llmusage/litellm_pricing.json`.

## Configuration file

TOML config at platform-specific location:

- **macOS**: `~/Library/Application Support/llmusage/config.toml`
- **Linux**: `~/.config/llmusage/config.toml`

| Key | Description | Default |
|-----|-------------|---------|
| `db_path` | SQLite database path | `<config_dir>/llmusage.db` |
| `anthropic_api_key` | Anthropic Admin API key | None |
| `openai_api_key` | OpenAI API key | None |
| `gemini_api_key` | Gemini API key | None |
| `ollama_host` | Ollama server URL | `http://localhost:11434` |
| `claude_code_enabled` | Parse Claude Code session logs | `true` |

## Uninstall

```bash
# If installed via cargo
cargo uninstall llmusage

# If installed via Homebrew
brew uninstall llmusage
```

To remove all data and configuration:
- **macOS**: `rm -rf ~/Library/Application\ Support/llmusage`
- **Linux**: `rm -rf ~/.config/llmusage`

## Screenshots

### Daily usage with provider grouping

![Daily usage report showing provider-grouped models with per-model token and cost breakdown](screenshots/daily_usage_all.png)

## Architecture

```
CLI (clap)
  |
SQLite DB (dedup index, WAL mode)
  |
Collectors (one per source, async)
  ├── claude_code   ~/.claude/projects/**/*.jsonl
  ├── codex         ~/.codex/archived_sessions/*.jsonl
  ├── cursor        ~/Library/Application Support/Cursor/.../state.vscdb
  ├── opencode      ~/.local/share/opencode/opencode.db
  ├── gemini_cli    ~/.gemini/tmp/**/chats/*.jsonl
  ├── anthropic     /v1/organizations/usage
  ├── openai        /v1/organization/usage
  ├── gemini        (stub)
  └── ollama        /api/ps
```

## Tech stack

| Crate | Purpose |
|-------|---------|
| clap | CLI parsing (derive macros) |
| tokio | Async runtime |
| reqwest | HTTP client |
| rusqlite | SQLite (bundled) |
| serde / serde_json | Serialization |
| chrono | Date/time |
| tabled | Table rendering |
| colored | Terminal colors |
| toml | Config parsing |
| dirs | Platform directories |

## License

MIT

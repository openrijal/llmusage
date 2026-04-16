# llmusage

Track token usage and costs across AI providers and coding tools from a single CLI.

## What it does

llmusage collects usage data from multiple AI sources — API dashboards, local session logs — normalizes it into a unified SQLite database, and provides fast CLI queries for reporting and cost analysis.

**Supported providers:**

| Provider | Type | Config required |
|----------|------|-----------------|
| Claude Code | Local logs (`~/.claude/projects/`) | None (auto-detect) |
| Codex | Local logs (`~/.codex/archived_sessions/`) | None (auto-detect) |
| OpenCode | Local SQLite (`~/.local/share/opencode/opencode.db`) | None (auto-detect) |
| Gemini CLI | Local logs (`~/.gemini/`) | None (stub) |
| Anthropic API | REST API | `anthropic_api_key` |
| OpenAI API | REST API | `openai_api_key` |
| Gemini API | REST API | `gemini_api_key` (stub) |
| Ollama | REST API | None (defaults to `localhost:11434`) |

## Installation

```bash
cargo install llmusage
```

Single static binary, no runtime dependencies (SQLite is bundled).

### Build from source

```bash
git clone https://github.com/openrijal/llmusage.git
cd llmusage
cargo build --release
# binary at ./target/release/llmusage
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

## Architecture

```
CLI (clap)
  |
SQLite DB (dedup index, WAL mode)
  |
Collectors (one per source, async)
  ├── claude_code   ~/.claude/projects/**/*.jsonl
  ├── codex         ~/.codex/archived_sessions/*.jsonl
  ├── opencode      ~/.local/share/opencode/opencode.db
  ├── gemini_cli    ~/.gemini/ (stub)
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

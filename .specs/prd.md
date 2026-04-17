# llmusage - Product Requirements Document

## Overview

**llmusage** is a Rust CLI tool that tracks token usage and costs across AI providers and coding tools. It collects usage data from multiple sources (API dashboards, local session logs), normalizes it into a unified SQLite database, and provides CLI queries for reporting and cost analysis.

## Problem Statement

Developers using multiple AI coding assistants (Claude Code, Codex, Cursor, OpenCode, Gemini CLI) and AI APIs (Anthropic, OpenAI, Gemini) have no unified way to track their token consumption and costs. Each tool stores usage data in different formats and locations, making it difficult to answer basic questions like:

- How much am I spending per day/week/month across all tools?
- Which models consume the most tokens?
- What's my usage trend over time?

## Target Users

- Individual developers using AI coding assistants
- Teams tracking AI tool costs across projects
- Power users managing multiple AI provider accounts

## Product Goals

1. **Unified collection**: Pull usage data from all major AI coding tools and APIs into one place
2. **Zero-config for local tools**: Auto-detect installed local tools (Claude Code, Codex, Cursor, OpenCode, Gemini CLI JSONL) without requiring API keys
3. **Accurate pricing**: Use LiteLLM's maintained pricing database (900+ models) with hardcoded fallback
4. **Fast queries**: SQLite-backed storage with indexed queries for instant reporting
5. **Simple distribution**: Single binary via `cargo install`, no runtime dependencies

## Architecture

```
┌──────────────────────┐
│   CLI (clap)         │  ← query/display layer
├──────────────────────┤
│   SQLite DB (WAL)    │  ← unified schema with dedup index
├──────────────────────┤
│   Collectors         │  ← one per source, runs on-demand
│   ├─ claude_code     │  ← ~/.claude/projects/**/*.jsonl
│   ├─ codex           │  ← ~/.codex/archived_sessions/*.jsonl
│   ├─ cursor          │  ← ~/Library/Application Support/Cursor/.../state.vscdb
│   ├─ opencode        │  ← ~/.local/share/opencode/opencode.db
│   ├─ gemini_cli      │  ← ~/.gemini/tmp/**/chats/*.jsonl
│   ├─ anthropic       │  ← API: /v1/organizations/usage
│   ├─ openai          │  ← API: /v1/organization/usage
│   ├─ gemini          │  ← API: stub (no clean usage API)
│   └─ ollama          │  ← API: /api/ps (requires config)
└──────────────────────┘
```

## Data Model

### Usage Record Schema

| Field | Type | Description |
|-------|------|-------------|
| id | INTEGER | Auto-increment primary key |
| provider | TEXT | Source identifier (claude_code, codex, opencode, anthropic, openai, etc.) |
| model | TEXT | Model name (claude-opus-4-6, gpt-4o, etc.) |
| input_tokens | INTEGER | Input/prompt tokens |
| output_tokens | INTEGER | Output/completion tokens |
| cache_read_tokens | INTEGER | Cached input tokens read |
| cache_write_tokens | INTEGER | Cache creation tokens |
| cost_usd | REAL | Calculated cost in USD |
| session_id | TEXT | Optional session grouping |
| recorded_at | TEXT | ISO8601 timestamp of the usage event |
| collected_at | TEXT | When the record was synced |
| metadata | TEXT | JSON blob for provider-specific extras |

### Deduplication

A UNIQUE index on `(provider, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, recorded_at, session_id)` ensures idempotent syncs via `INSERT OR IGNORE`.

## CLI Commands

| Command | Description |
|---------|-------------|
| `llmusage sync [--provider P]` | Pull usage from all configured/detected providers |
| `llmusage daily [--days N] [--provider P] [--json]` | Daily usage breakdown |
| `llmusage weekly [--weeks N] [--provider P] [--json]` | Weekly usage breakdown |
| `llmusage monthly [--months N] [--provider P] [--json]` | Monthly usage breakdown |
| `llmusage summary [--days N] [--provider P] [--model M]` | Aggregated by provider+model |
| `llmusage detail [--model M] [--since DATE] [--limit N]` | Per-record breakdown |
| `llmusage models [--provider P]` | List known models and pricing |
| `llmusage update-pricing` | Refresh LiteLLM pricing cache |
| `llmusage config [--set KEY=VALUE]` | Manage API keys and settings |
| `llmusage export [--format csv\|json] [--output FILE]` | Export data |

## Collector Tiers

### Tier 1 - Local log parsing (zero-config)

- **Claude Code**: Parses JSONL session logs from `~/.claude/projects/`. Extracts `message.usage` from assistant messages.
- **Codex**: Parses JSONL from `~/.codex/archived_sessions/`. Extracts `token_count` events with `last_token_usage` deltas.
- **Cursor**: Reads local SQLite state from Cursor's `state.vscdb` and extracts per-bubble token counts from persisted Cursor AI metadata.
- **OpenCode**: Reads directly from SQLite at `~/.local/share/opencode/opencode.db`. Message data contains `tokens` and `cost` fields.
- **Gemini CLI**: Parses JSONL chat session files under `~/.gemini/tmp/**/chats/`.

### Tier 2 - API-based (requires API keys)

- **Anthropic**: Admin API at `/v1/organizations/usage`, grouped by model
- **OpenAI**: Usage API at `/v1/organization/usage`
- **Ollama**: Requires explicit `ollama_host` config

### Tier 3 - Strict-mode unsupported

- **Legacy Antigravity protobuf sessions**: Uses protobuf (`.pb`) for conversations - no parseable usage data
- **Windsurf**: Current local artifacts do not expose reliable token counts
- **VS Code AI tooling**: Installed extensions expose session/model metadata, but not token counts
- **Gemini API**: No clean programmatic usage API from Google AI Studio

## Pricing

Model pricing is sourced from LiteLLM's `model_prices_and_context_window.json` (GitHub raw URL), cached locally at `~/.cache/llmusage/litellm_pricing.json`. Auto-fetched on first sync. Hardcoded fallback covers common Anthropic, OpenAI, and Gemini models.

Pricing is stored as cost-per-token in LiteLLM format and converted to per-million-token for display.

## Configuration

TOML config at `~/.config/llmusage/config.toml` (macOS: `~/Library/Application Support/llmusage/config.toml`).

| Key | Description |
|-----|-------------|
| `db_path` | SQLite database location |
| `anthropic_api_key` | Anthropic Admin API key |
| `openai_api_key` | OpenAI API key |
| `gemini_api_key` | Gemini API key |
| `ollama_host` | Ollama server URL (enables Ollama collector) |
| `claude_code_enabled` | Enable/disable Claude Code log parsing (default: true) |

## Display

- Tables rendered with `tabled` crate, box-drawing characters
- Headers in cyan, totals in yellow (via `colored` crate)
- Model names abbreviated: `claude-opus-4-6-20260205` → `opus-4-6`
- Token counts with comma separators: `1,234,567`
- Costs formatted contextually: `$42.10`, `$0.271`, `$0.0034`

## Tech Stack

| Dependency | Purpose |
|-----------|---------|
| clap | CLI argument parsing with derive macros |
| tokio | Async runtime for API collectors |
| reqwest | HTTP client for API calls and LiteLLM fetch |
| rusqlite | SQLite with bundled build (no system dependency) |
| serde / serde_json | JSON serialization/deserialization |
| chrono | Date/time handling |
| tabled | Table rendering |
| colored | Terminal colors |
| toml | Config file parsing |
| dirs | Platform-specific directory resolution |
| async-trait | Async trait support for Collector interface |
| anyhow | Error handling |

## Distribution

- `cargo install llmusage` from crates.io
- Single static binary, no runtime dependencies (SQLite bundled)
- Works on macOS and Linux

## Non-Goals (v0.1)

- No daemon/background process - use cron for periodic sync
- No web UI or TUI dashboard
- No team/multi-user features
- No real-time streaming collection
- No proxy-based interception for arbitrary apps
- No metadata-only collectors in strict mode

## Future Considerations

- Session-level breakdown (group by session_id)
- Interactive TUI with ratatui
- GitHub Actions integration for CI cost tracking
- Gemini CLI protobuf reverse-engineering
- Windsurf usage support if a token-bearing local or API source becomes available
- VS Code extension support if token-bearing local artifacts become available
- Cost alerts and budgets
- Historical cost trend charts

# AGENTS.md

## Project Overview

**llmusage** is a Rust CLI tool that tracks token usage and costs across AI providers and coding tools. It collects usage data from local session logs and provider APIs, normalizes it into SQLite, and provides time-series reports (daily/weekly/monthly).

## Architecture

```
src/main.rs          → CLI entry (clap derive), command dispatch
src/models.rs        → UsageRecord, SummaryRow, DailyRow, ModelPricing
src/config.rs        → TOML config (~/.config/llmusage/config.toml)
src/costs.rs         → LiteLLM pricing engine + fallback
src/display.rs       → Table rendering (tabled + manual box-drawing), colors
src/db/              → SQLite layer (WAL mode, dedup via UNIQUE index)
src/collectors/      → One module per source, all implement Collector trait
```

### Collector System

All collectors implement `async_trait Collector { fn name(); async fn collect() -> Vec<UsageRecord> }`.

**Local log parsers (zero-config, auto-detected):**
- `claude_code.rs` — parses `~/.claude/projects/**/*.jsonl`, extracts `message.usage` from assistant entries
- `codex.rs` — parses `~/.codex/archived_sessions/*.jsonl`, extracts `token_count` events with `last_token_usage`
- `opencode.rs` — reads `~/.local/share/opencode/opencode.db` (SQLite), parses JSON `data` column from `message` table
- `gemini_cli.rs` — stub (conversations are protobuf `.pb` files)

**API-based (require config):**
- `anthropic.rs` — `/v1/organizations/usage` (needs `anthropic_api_key`)
- `openai.rs` — `/v1/organization/usage` (needs `openai_api_key`)
- `gemini.rs` — stub (no clean usage API)
- `ollama.rs` — `/api/ps` (needs explicit `ollama_host`)

### Data Flow

```
llmusage sync → collectors auto-detect installed tools → fetch records
             → normalize to UsageRecord → INSERT OR IGNORE into SQLite
             → dedup via UNIQUE index on (provider, model, tokens, timestamp, session)

llmusage daily → query_grouped(DATE(recorded_at)) → BTreeMap aggregation → colored table
```

### Pricing

Model pricing sourced from LiteLLM's `model_prices_and_context_window.json` (900+ models). Cached at `~/.cache/llmusage/litellm_pricing.json`. Auto-fetched on first sync. Hardcoded fallback for ~11 common models. Prices stored per-token in LiteLLM, converted to per-million-token for display.

## Code Conventions

- **Error handling**: `anyhow::Result` everywhere, `bail!` for early returns with context
- **SQL**: Dynamic query building with parameterized `?N` placeholders, `Vec<Box<dyn ToSql>>`
- **Async**: Required only for collectors (API calls). DB and display are sync.
- **No unwrap**: All fallible operations use `?` or explicit error handling
- **Clippy clean**: Zero warnings required — run `cargo clippy` before committing

## Build and Test

```bash
cargo build                    # dev build
cargo clippy                   # lint (must pass clean)
cargo run -- sync              # collect from all detected providers
cargo run -- daily             # daily usage report
cargo run -- models            # show pricing table
cargo install --path .         # install to ~/.cargo/bin/llmusage
```

## Key Design Decisions

1. **INSERT OR IGNORE for dedup** — syncs are idempotent; running sync multiple times is safe
2. **Auto-detect local tools** — check for directory/file existence, no config needed for Claude Code/Codex/OpenCode
3. **API collectors require explicit keys** — never auto-connect to remote APIs
4. **Ollama opt-in only** — requires `ollama_host` config to avoid connection errors on machines without Ollama
5. **LiteLLM for pricing** — maintained externally, covers 900+ models, with hardcoded fallback
6. **Manual table rendering for daily/weekly/monthly** — enables colored header/total rows that tabled crate doesn't support per-row
7. **Model name abbreviation** — strip `claude-` prefix and `-YYYYMMDD` date suffixes for readability
8. **No daemon** — use cron (`0 * * * * llmusage sync`) for periodic collection

## Adding a New Collector

1. Create `src/collectors/<name>.rs` implementing the `Collector` trait
2. Add `pub mod <name>;` to `src/collectors/mod.rs`
3. Add auto-detection logic in `get_collectors()` — check for files/config before adding
4. Map any new provider names in `costs.rs` `normalize_provider()` if using LiteLLM pricing
5. Run `cargo clippy` and `cargo build`

## File Layout

```
.spec/                    → Product spec, task tracking, implementation docs
  prd.md                  → Product requirements document
  tasks/                  → Task lists per branch
  implementation/         → Implementation details per branch
  validation/             → Test/verification records per branch
src/                      → Rust source (3,751 lines across 18 files)
Cargo.toml                → 13 dependencies, edition 2021
```

## Dependencies (rationale)

| Crate | Why |
|-------|-----|
| `rusqlite` (bundled) | Zero system deps — compiles SQLite from source |
| `tokio` (full) | Async runtime for API collectors |
| `reqwest` (json) | HTTP for APIs and LiteLLM fetch |
| `clap` (derive) | CLI with derive macros for minimal boilerplate |
| `async-trait` | Async methods on Collector trait |
| `tabled` | Table rendering for summary/detail/models |
| `colored` | Terminal colors for headers and totals |
| `dirs` | Platform-specific config/cache/data paths |

## Common Pitfalls

- **OpenCode path**: Uses `~/.local/share/` (XDG) even on macOS, not `dirs::data_dir()`
- **Claude Code log structure**: JSONL files are directly in project dirs, not in `sessions/` subdirs
- **Codex token_count**: Use `last_token_usage` (per-turn delta), not `total_token_usage` (cumulative)
- **LiteLLM prices are per-token**: Multiply by 1,000,000 for per-MTok display
- **Cache tokens**: Anthropic's `input_tokens` field is often near-zero; real input is in `cache_read_input_tokens`

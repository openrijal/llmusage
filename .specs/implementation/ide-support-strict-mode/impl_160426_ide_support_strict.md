# Implementation Details - IDE Support Strict Mode

## Summary

This change set adds strict-mode IDE support where a real token-bearing local source exists, and codifies explicit non-support where it does not.

## Cursor

- Added `src/collectors/cursor.rs`.
- Source: `<config_dir>/Cursor/User/globalStorage/state.vscdb` where `<config_dir>` is `~/Library/Application Support` on macOS and `~/.config` on Linux.
- Implementation copies the live SQLite DB to `/tmp` before querying to avoid lock conflicts with a running Cursor instance.
- Reads `cursorDiskKV` rows:
  - `composerData:*` for model configuration
  - `bubbleId:*` for per-bubble token counts and timing metadata
- Produces `UsageRecord` rows with:
  - `provider = "cursor"`
  - `session_id = <composer_id>`
  - `input_tokens` / `output_tokens` from persisted Cursor token metadata
  - timestamp inferred from Cursor client timing fields
  - metadata including `bubble_id`, `usage_uuid`, and `server_bubble_id`

## Provider Registry Changes

- Registered `cursor` in `src/collectors/mod.rs`.
- Added provider aliasing:
  - `antigravity` -> `gemini_cli`
  - `gemini-cli` -> `gemini_cli`
  - `vscode-copilot-chat` -> `vscode`
- Added explicit provider explanations for unsupported or missing providers so `llmusage sync --provider windsurf` and `--provider vscode` fail with actionable strict-mode messages.

## Config / Status Output

- Extended `config --list` output to show:
  - detected local collectors
  - unsupported local IDE tooling discovered on disk
  - strict-mode notes for legacy Antigravity protobuf sessions, Windsurf, and VS Code

## Strict-Mode Outcomes

### Implemented

- `cursor`
- `gemini_cli` reuse via `antigravity` alias

### Not Implemented

- `windsurf`
  - local data investigated, but no reliable token-bearing session source found
- `vscode`
  - installed AI extensions investigated, but no token-bearing local source found
- legacy Antigravity protobuf sessions
  - still not parseable for strict token accounting

## Documentation Updates

- Updated `README.md` supported provider table and architecture section
- Updated `.specs/prd.md` to reflect Cursor support, Gemini CLI JSONL support, and strict-mode unsupported tooling

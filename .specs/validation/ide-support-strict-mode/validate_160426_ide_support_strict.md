# Validation Record - IDE Support Strict Mode

## Build / Lint

### cargo build
- **Status**: PASS

### cargo clippy --all-targets --all-features -- -D warnings
- **Status**: PASS

## Targeted Tests

### cargo test cursor
- **Status**: PASS
- **Coverage**:
  - parses Cursor bubble rows into `UsageRecord`
  - reads a copied temp Cursor SQLite DB and extracts one record

## CLI Smoke Tests

### cargo run -- config --list
- **Status**: PASS
- **Verified**:
  - `cursor` is shown as detected when local Cursor state exists
  - `windsurf` is shown as unsupported with a strict-mode note when local Windsurf state exists
  - `vscode` is shown as unsupported with a strict-mode note when local VS Code state exists
  - `gemini_cli` / legacy Antigravity status is surfaced accurately

### Expected strict-mode provider behavior

- `llmusage sync --provider cursor`
  - should activate the Cursor collector when local Cursor state exists
- `llmusage sync --provider antigravity`
  - should behave the same as `--provider gemini_cli`
- `llmusage sync --provider windsurf`
  - should report that Windsurf is unsupported in strict mode because local artifacts do not expose reliable token counts
- `llmusage sync --provider vscode`
  - should report that VS Code is unsupported in strict mode because available extension data lacks token counts

## Validation Notes

- Windsurf and VS Code remain intentionally unimplemented under strict mode.
- The absence of collectors for those providers is expected behavior, not a gap in verification.

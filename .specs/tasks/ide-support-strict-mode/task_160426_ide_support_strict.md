# Task Record - IDE Support Strict Mode

## Objective

Advance issues `#64`-`#67` using a strict collection bar:

- implement local collectors only when real token counts are available
- avoid metadata-only collectors for unsupported IDE tooling
- document strict-mode limitations clearly in code, CLI output, README, and specs

## Scope

- [x] Investigate Cursor local artifacts for token-bearing usage data
- [x] Investigate Windsurf local artifacts for token-bearing usage data
- [x] Investigate VS Code AI-tooling local artifacts for token-bearing usage data
- [x] Reuse existing `gemini_cli` support rather than re-implementing it
- [x] Add Cursor collector when strict-mode token data is available
- [x] Add provider alias support for `antigravity -> gemini_cli`
- [x] Make unsupported providers fail honestly (`windsurf`, `vscode`)
- [x] Update `config --list` to expose supported vs unsupported local tooling
- [x] Update README and `.specs/prd.md`
- [x] Add implementation and validation records for this work

## Decisions

- Cursor is supported because persisted local state contains queryable token counts.
- Windsurf is not implemented because current local artifacts lack reliable token counts.
- VS Code is not implemented because installed AI extensions expose session/model metadata but not token counts.
- Antigravity remains an alias to `gemini_cli`; legacy `.pb` sessions are still unsupported in strict mode.

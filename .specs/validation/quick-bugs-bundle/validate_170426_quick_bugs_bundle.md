# Validation Record - Quick-Bug Bundle

## Automated Checks

- `cargo fmt --all --check` — PASS
- `cargo build` — PASS
- `cargo clippy --all-targets -- -D warnings` — PASS
- `cargo test` — PASS (20 pre-existing + 3 new = 23 tests in the lib/bin target)

### New Tests (`src/display.rs::format_tests`)

- `positive_numbers_get_commas`: `0`, `123`, `1,234`, `1,234,567`.
- `negative_numbers_get_commas_and_sign`: `-1`, `-1,234`, `-1,234,567`.
- `i64_min_does_not_panic`: smoke test for the `i128` branch.

## Manual Smoke — Recommended Post-Merge

- **#33**: construct a `Config` with empty `config_path` (via test or custom deserializer) and call `save_config`. Expect a clean `anyhow::Error`, not a panic.
- **#34**: insert records on a week-boundary date (e.g., 2025-12-29 through 2026-01-04) and run `llmusage weekly`. Confirm both weeks render as `2025 W53` / `2026 W01`, not `2025 W52` / `2026 W00`.
- **#36**: `llmusage export --format xml` should exit non-zero with the message `Unknown export format: 'xml'. Supported: csv, json`. `--format csv` and `--format json` should continue to work.

## Regression Risk

- **#34 SQLite version.** If a user is on an older embedded SQLite (only relevant if they swap to `rusqlite` with `features = ["sqlcipher"]` + system `libsqlite3` < 3.44), `%V` silently returns NULL. The bundled build we ship avoids this. No runtime version check added.
- **#30 existing call sites.** All callers pass token counts from DB records (`i64`); none pass computed negatives today. Behavior for positive inputs is unchanged by inspection and by the new `positive_numbers_get_commas` test.
- **#36 scripted users.** Anyone with a shell script that passes a wrong format and relied on CSV output will now see a non-zero exit. This is the intended behavior and is called out in the PR description.

## Rollback

Each fix is a localized diff (1–20 lines). Reverting any one of them is straightforward via `git revert <commit> -- <file>`.

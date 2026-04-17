# Task Record - Quick-Bug Bundle (save_config, token formatting, ISO weeks, export validation)

## Objective

Close four small, independent correctness bugs in one PR. Each is low-complexity and unrelated to the others in mechanism, but bundling them avoids four separate review rounds for ~40 lines of net change.

## Scope

- [x] **#33** — `config.rs::save_config` replaces `cfg.config_path.parent().unwrap()` with a guarded `ok_or_else` that returns an `anyhow::Error` instead of panicking on an empty `PathBuf`.
- [x] **#30** — `display.rs::format_tokens_comma` strips the sign before computing comma positions, so negative values format as `-1,234` instead of `-,123,4`. `i64::MIN` is handled safely via an `i128` magnitude.
- [x] **#34** — `db/queries.rs::query_weekly` groups by `strftime('%G-W%V', recorded_at)` (ISO 8601 year + week) instead of `%Y-W%W`. Eliminates the `W00` label and the Dec-31-is-next-year's-W01 rollover confusion.
- [x] **#36** — `main.rs::cmd_export` rejects unknown `--format` values with `anyhow::bail!` instead of silently emitting CSV. `csv` and `json` are the only valid values.

## Decisions

- **Bundle instead of split.** Four separate PRs for four one-to-five line fixes would burn more reviewer time than it saves. All four fixes are local, touch independent files, and carry their own tests — so regressions in one are not correlated with the others.
- **`i128` for `i64::MIN`.** `-i64::MIN` overflows in `i64`. `(n as i128).unsigned_abs()` is a one-line fix that makes the function total for all `i64` inputs rather than adding a special-case branch.
- **ISO week via SQLite `%V`, not Rust post-processing.** `rusqlite` bundles SQLite ≥ 3.45, which supports `%V`/`%G`. Computing ISO weeks in Rust after the query would require a second grouping pass and lose the streaming advantage of `query_grouped`.
- **Reject unknown export formats loudly.** The prior catch-all `_ => to_csv(...)` is a footgun: a typo silently produces the wrong output. Bailing makes the contract explicit and the failure mode obvious.

## Out of Scope

- No refactor of `format_tokens_comma` into a crate (`num-format` etc.) — the inline fix is small enough that adding a dependency is not justified.
- No migration of existing saved configs — this is a purely defensive fix.
- No change to monthly grouping (`%Y-%m`) — unaffected by the weekly issue.

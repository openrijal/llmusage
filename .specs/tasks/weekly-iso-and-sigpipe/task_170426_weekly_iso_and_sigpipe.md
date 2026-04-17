# Task Record - Fix Weekly Crash + Broken-Pipe Panic

## Objective

Fix two runtime regressions surfaced during local smoke-testing of v0.1.3 + the bug-bundle merges:

1. **`llmusage weekly` crashes** with `Error: Invalid column type Null at index: 0, name: period`. The prior CHANGELOG-era code used `strftime('%Y-W%W', ...)`; PR #78 changed it to `strftime('%G-W%V', ...)`. `%V` and `%G` were added in **SQLite 3.46** (May 2024); `rusqlite = 0.31` bundles **SQLite 3.45**, which silently returns `NULL` for unknown modifiers. Row decoding then fails on the NULL period column.

2. **Broken-pipe panic** when piping output into `head`, e.g. `llmusage export --format csv | head`. Rust's stdlib sets `SIGPIPE` to `SIG_IGN` at startup, so writing to a closed pipe yields `EPIPE`, and `println!` unwraps the error and panics. The expected CLI behavior is silent termination (matching `cat`, `grep`, etc.).

## Scope

- [x] Replace the SQL-side ISO-week grouping with an in-process rebucketing using `chrono::Datelike::iso_week`. Portable on every bundled SQLite version.
- [x] Add `libc` as a `cfg(unix)` dependency and reset `SIGPIPE` to `SIG_DFL` at the top of `main`.
- [x] Unit tests for ISO-week bucketing: same-week aggregation, Monday boundary split, year-boundary (`2027-01-01 → 2026-W53`), cross-day model merge.
- [x] Manual smoke: `llmusage weekly` renders, `llmusage export --format csv | head` exits 0.

## Decisions

- **Compute ISO weeks in Rust, not SQL.** Bumping `rusqlite` to a version that bundles SQLite 3.46+ would work but carries API churn risk across a minor-version jump during a release-prep window. The Rust computation is ~60 lines, uses an already-present `chrono` dep, is test-friendly, and eliminates any future "bundled SQLite is too old" surprise. The small cost is that weekly aggregation now runs in-process instead of server-side — irrelevant at the row counts involved.
- **Reset `SIGPIPE` to `SIG_DFL` globally at `main` entry.** Alternative: wrap every `println!` in a pattern that handles `EPIPE` cleanly. Rejected — invasive and easy to regress. The signal reset is one line, POSIX-standard behavior, and matches user expectations for a Unix pipeline tool. Windows gets a no-op stub.
- **Add `libc = "0.2"` under `[target.'cfg(unix)'.dependencies]`.** Avoiding a direct FFI `extern "C"` block keeps the unsafe surface minimal and well-audited.
- **Earlier spec claim was wrong.** `.specs/tasks/quick-bugs-bundle/task_170426_quick_bugs_bundle.md` asserted SQLite ≥ 3.44 sufficed; that was incorrect (the modifiers land in 3.46). Not amending the historical spec — this task records the correction instead.

## Out of Scope

- No data migration or schema change.
- Not upgrading `rusqlite`. Considered and deferred.
- Monthly grouping (`%Y-%m`) is unaffected — `%Y` and `%m` have been in SQLite since forever.

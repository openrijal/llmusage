# Schema Migration System

## Goal

Add migration and version tracking for SQLite schema upgrades without requiring users to delete their existing database or lose historical data.

## Non-Goals

- No new user-facing CLI or command surface
- No unrelated schema redesign
- No bundling of richer token fields or new usage columns into this change

## Current State

- [`initialize()`](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/db/schema.rs:4) uses `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS`.
- Existing databases are opened through [Database::open()](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/db/mod.rs:13) and never receive explicit schema upgrades.
- Any future table or index evolution will not be applied automatically to already-initialized databases.

## Design

- Use `PRAGMA user_version` as the schema version store.
- Define `LATEST_SCHEMA_VERSION = 2`.
- Treat pre-version databases with `usage_records` present as legacy schema version `1`.
- Fresh databases initialize directly to version `2`.

## Required Functions

- `initialize(conn)`
- `current_schema_version(conn) -> Result<i64>`
- `set_schema_version(conn, version) -> Result<()>`
- `create_schema_v1(conn) -> Result<()>`
- `migrate(conn, from, to) -> Result<()>`
- `migrate_v1_to_v2(conn) -> Result<()>`

## Migration Rules

- Run all migration work inside a transaction.
- Only bump the schema version after the migration completes successfully.
- Fail closed on unknown future versions.
- Keep startup reopen-safe and idempotent.

## Version Definitions

- `1`: Legacy current table and index shape, with no explicit version metadata in existing installs
- `2`: Migration-managed schema baseline

## v1 -> v2 Scope

- Keep the current `usage_records` column set unchanged.
- Recreate or normalize indexes under explicit migration control if needed.
- Do not delete user data.

## Call Flow

- `Database::open()` opens the SQLite database.
- `Database::open()` enables WAL and foreign key pragmas.
- `Database::open()` calls a migration-aware `schema::initialize()`.
- `schema::initialize()` ensures the database is at `LATEST_SCHEMA_VERSION` before returning.

## Risks

- Legacy database detection could misclassify an unusual local database state.
- A partially applied migration would leave the database in an inconsistent state if not fully transactional.
- Future schema edits may still drift if version updates are not added alongside DB changes.

## Follow-On Work Enabled

- `#50` Dedup index changes
- `#57` OpenAI collector token shape expansion
- `#58` Database purge and info commands

## Assumptions

- `PRAGMA user_version` is sufficient and preferred over a dedicated schema history table for this project size.
- The first migration-capable release should establish the managed baseline as schema version `2`.
- This issue should stay internal to the DB layer and not widen scope into adjacent feature work.

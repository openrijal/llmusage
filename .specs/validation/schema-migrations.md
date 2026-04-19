# Validation Plan

## Acceptance Criteria

- A fresh database gets the latest schema version.
- A legacy database upgrades without data loss.
- Reopening an upgraded database is a no-op.
- A migration failure rolls back cleanly.
- Existing commands still work after upgrade.

## Test Scenarios

1. Fresh temporary database created through the normal open path
2. Legacy database seeded with the old `usage_records` schema and no version marker
3. Already-upgraded database reopened with no additional migration work
4. Simulated migration failure to confirm rollback behavior
5. Read and query smoke test after migration

## Commands To Run

- `cargo build`
- `cargo clippy`
- `cargo test`

## Evidence To Record

- Schema version before and after initialization
- Presence of expected tables and indexes
- Row counts preserved across migration
- Any migration log or error text captured during testing

## Out Of Scope

- Performance benchmarking
- Release automation
- Non-SQLite backends

## Assumptions

- Validation will be done against temporary or disposable local databases.
- Existing reporting commands are sufficient for post-migration smoke testing.
- No PRD changes are required for this spec-only task.

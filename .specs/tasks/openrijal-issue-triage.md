# Issue Triage

## Branch

`openrijal/issue-triage`

## Recommendation

Recommend GitHub issue `#56`.

Link: https://github.com/openrijal/llmusage/issues/56

## Why This Next

- The current DB boot path has no migration or schema version tracking.
- Future schema changes will break existing installs or silently fail to apply.
- This unblocks safer work on dedup/index changes, richer token fields, and DB cleanup or management.

## Alternatives Considered

- `#53` Add environment variable support for API key configuration
- `#35` Codex collector uses hardcoded model name instead of actual model
- `#58` Add llmusage reset/purge command for database management

## Decision

Priority order:

1. `#56`
2. `#50`
3. `#57`
4. `#58`

## Current Code References

- [src/db/schema.rs](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/db/schema.rs:4)
- [src/db/mod.rs](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/db/mod.rs:13)

Supporting references for related or partially stale issues:

- [src/collectors/openai.rs](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/collectors/openai.rs:45)
- [src/collectors/codex.rs](/Users/openrijal/conductor/workspaces/llmusage/bandung/src/collectors/codex.rs:87)

## Assumptions

- `.specs` is the authoritative spec root for this workspace.
- The next recommended issue remains `#56`.
- The product PRD does not need changes for this triage decision.

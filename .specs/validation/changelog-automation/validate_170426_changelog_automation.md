# Validation Record - Changelog Automation

## Local Verification

### git-cliff --config cliff.toml --output CHANGELOG.md
- **Status**: PASS
- Generates a Keep-a-Changelog style file with sections for `Unreleased`, `0.1.2`, `0.1.1`, `0.1.0`.
- Each release is grouped into `Added`, `Changed`, `Fixed`, `Removed` as appropriate for that release.
- Housekeeping commits (`Bump version …`, `Apply cargo fmt`, merge commits) are excluded.

### git-cliff --config cliff.toml --latest --strip all
- **Status**: PASS
- Produces only the most recent release section with no header, suitable for use as the GitHub release body.

## Workflow Steps — To Verify on Next Tag

The release workflow has not yet been exercised end-to-end because that requires cutting a new tag. The following should be validated on the next release (`v0.1.3` or later):

- `Create Release` job uses `RELEASE_NOTES.md` (produced by git-cliff) as the release body, not the default auto-generated GitHub notes.
- `Commit CHANGELOG.md back to main` step either commits a new `Update CHANGELOG for vX.Y.Z` commit or exits cleanly with "CHANGELOG.md unchanged".
- The new commit on `main` is attributed to `github-actions[bot]` and does not break branch-protection rules.
- No recursion: the `Update CHANGELOG for …` commit itself is skipped by `cliff.toml` and therefore does not appear in the next release's changelog.

## Backfill Correctness

- `v0.1.0` section captures the initial CLI, collectors, and release tooling.
- `v0.1.1` section captures the homepage/email/badge updates.
- `v0.1.2` section captures the Gemini CLI JSONL collector, Cursor strict-mode support, the four collector bugfixes, and the release-pipeline lockfile fix.
- `Unreleased` currently captures the install script and workflow-permission changes, consistent with the commit history on `main`.

## Validation Notes

- `generate_release_notes: true` has been removed from the release action to avoid duplicate release notes; git-cliff is now the single source of truth.
- The stash + checkout dance in the commit step exists because the workflow checks out the tag, not `main`. Switching branches while keeping the regenerated file in the working tree is the simplest way to land the commit on `main`.

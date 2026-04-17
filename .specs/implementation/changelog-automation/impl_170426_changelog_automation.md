# Implementation Details - Changelog Automation

## Summary

Adds `cliff.toml`, seeds `CHANGELOG.md` with the full history for `v0.1.0` through `v0.1.2`, and extends `.github/workflows/release.yml` so that every tag push (matching `v*`) regenerates both the GitHub release body and `CHANGELOG.md` at the repo root.

## File Layout

- `cliff.toml` (new) — git-cliff configuration.
- `CHANGELOG.md` (new) — Keep a Changelog format.
- `.github/workflows/release.yml` — release job extended to invoke git-cliff and commit the changelog.
- `.specs/prd.md` — distribution section unchanged; release process notes updated informally via this spec.

## cliff.toml

- `conventional_commits = false` — commit messages are not required to match the Conventional Commits spec.
- Regex-based `commit_parsers` matching the existing history:
  - `^(Add|Introduce|Implement|Support) ` → **Added**
  - `^Fix` → **Fixed**
  - `^(Remove|Drop|Delete) ` → **Removed**
  - `^Deprecate ` → **Deprecated**
  - Catch-all `.*` → **Changed**
- `skip = true` rules for version bumps, `cargo fmt` commits, merge commits, previous `Update CHANGELOG` commits, and `chore(release)` style tags so the file only contains user-facing lines.
- `tag_pattern = "v[0-9]+\\.[0-9]+\\.[0-9]+$"` — changelog sections are driven exclusively by semver tags.
- Keep-a-Changelog style header + per-release body template.

## Release Workflow

`release.yml::release` job additions:

1. `actions/checkout@v4` with `fetch-depth: 0` so git-cliff sees the full tag history.
2. `orhun/git-cliff-action@v4` call #1 (`--latest --strip all`) → writes per-release notes to `RELEASE_NOTES.md`.
3. `orhun/git-cliff-action@v4` call #2 (`--output CHANGELOG.md`) → regenerates the full changelog.
4. A bash step that:
   - Early-exits if `CHANGELOG.md` is unchanged against the tag's tree.
   - Stashes the regenerated `CHANGELOG.md`, switches onto `main`, pops the stash, and commits `"Update CHANGELOG for <tag>"` attributed to `github-actions[bot]`.
   - Pushes the commit to `origin/main`.
5. `softprops/action-gh-release@v2` now uses `body_path: RELEASE_NOTES.md` instead of `generate_release_notes: true`, so the release description is the curated git-cliff output rather than GitHub's auto-generated list of PRs.

The job grants `permissions.contents: write` explicitly so the `main`-branch push succeeds without relying on inherited defaults.

## Housekeeping

- The generated `Update CHANGELOG for v…` commits are themselves `skip = true` in `cliff.toml` so they don't recurse into future changelog sections.
- `cargo fmt` and `Bump version` commits are likewise skipped to keep the changelog focused on user-visible changes.

## Out of Scope

- Automating the `Cargo.toml` version bump (release-please territory).
- Backporting commit messages to conform to Conventional Commits.
- Emitting a PR for changelog updates instead of a direct push (can be revisited if `main` becomes protected).

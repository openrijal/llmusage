# Task Record - Changelog Automation

## Objective

Ship a maintained `CHANGELOG.md` at the repo root, and automate both its upkeep and the GitHub release notes so every tagged release produces a consistent, user-facing changelog without manual writing.

## Scope

- [x] Add `cliff.toml` configured to the project's existing commit style
- [x] Backfill `CHANGELOG.md` covering `v0.1.0`, `v0.1.1`, `v0.1.2`
- [x] Hook `git-cliff` into `.github/workflows/release.yml` to:
  - generate the per-release notes used as the GitHub release body
  - regenerate the full `CHANGELOG.md` and commit it back to `main`
- [x] Replace `generate_release_notes: true` on the release action with `body_path: RELEASE_NOTES.md`
- [x] Grant the release job `contents: write` so it can commit to `main`
- [x] Document the changelog in the README installation/contribution area (optional — skipped for v1)

## Decisions

- **git-cliff over release-please.** release-please is more automated but requires the project to adopt strict Conventional Commit prefixes (`feat:`, `fix:`, …). The project's history uses imperative verbs (`Add …`, `Fix …`, `Bump …`), and switching style is a bigger lift than this feature warrants. git-cliff supports regex parsers keyed to the existing style.
- **Keep a Changelog style** for the CHANGELOG.md format — familiar, widely recognized, simple to read.
- **Auto-commit CHANGELOG.md back to `main`** after the tag push. The alternative is requiring the maintainer to run `git-cliff` locally before tagging; that is error-prone and creates a "forgot to update the changelog" failure mode.
- **Housekeeping commits are skipped** (version bumps, `cargo fmt` only, merge commits, previous `Update CHANGELOG` commits) so the changelog shows only user-facing changes.
- **Unreleased section stays** at the top of CHANGELOG.md between releases so in-flight changes are visible on `main` without waiting for a tag.

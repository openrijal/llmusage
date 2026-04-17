# Validation Record - Release Workflow PR-Based CHANGELOG

## Static Checks

### YAML syntax
- **Status**: PASS
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` parses cleanly.

### Action versions
- `peter-evans/create-pull-request@v7` — current major as of April 2026.
- `orhun/git-cliff-action@v4`, `actions/checkout@v4`, `actions/download-artifact@v4`, `softprops/action-gh-release@v2` unchanged.

## End-to-End — To Verify on Next Tag

Real verification requires a tag push. On the next release (`v0.1.4` or later), confirm:

- The release job checks out `main` (not the tag).
- `peter-evans/create-pull-request` opens a PR titled `Update CHANGELOG for vX.Y.Z` on branch `release/update-changelog` against `main`.
- The PR contains only the `CHANGELOG.md` diff (no other working-tree changes).
- The GitHub Release still gets created with git-cliff-generated notes.
- crates.io publish and Homebrew updates still succeed (no regression from the permission changes).
- After the PR is merged, the branch is auto-deleted (`delete-branch: true`).
- A subsequent release that also modifies `CHANGELOG.md` reuses the same branch and updates the existing PR rather than opening a new one.

## Regression Risk

- **Permissions scope creep.** The job now has `pull-requests: write`. This is a standard scope for release automation and doesn't grant admin access to branch protection rules.
- **Race between two tag pushes.** If two tags are pushed in quick succession, both runs try to write to the same `release/update-changelog` branch. peter-evans handles this by updating the existing branch rather than failing; the net PR reflects the later tag's content. Acceptable for this repo's release cadence.
- **Workflow no longer fails silently.** Removing `continue-on-error` means any real failure (network issue, permission misconfiguration) will block the release. This is a positive trade — the CHANGELOG update is now part of the release's success criteria rather than a best-effort afterthought.

## Rollback

If PR creation misbehaves on the next release, reverting this commit restores the previous direct-push-with-continue-on-error behavior. Manual CHANGELOG updates via admin-merged PR (as done for v0.1.3) remain a viable fallback.

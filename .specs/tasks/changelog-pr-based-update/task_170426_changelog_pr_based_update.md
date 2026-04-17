# Task Record - Release Workflow Opens a PR for CHANGELOG Updates

## Objective

Make the release workflow's CHANGELOG update actually land on `main`. Previously the workflow tried to push directly from `github-actions[bot]`, which branch protection on `main` rejects with `GH013 — Changes must be made through a pull request`.

The step was made `continue-on-error` in the emergency v0.1.3 fix so the rest of the release could ship, but that lost the CHANGELOG auto-update. This task replaces the direct push with a PR-creation flow.

## Scope

- [x] Replace the direct-push step with `peter-evans/create-pull-request@v7`
- [x] Grant the release job `pull-requests: write` (on top of its existing `contents: write`)
- [x] Check out `main` instead of the tag so git-cliff sees the tag graph and the PR branches off `main` cleanly
- [x] Delete the PR branch automatically on merge
- [x] Remove the `continue-on-error` stopgap (PR creation should succeed cleanly; real failures should fail loudly)
- [x] Document the behavior in specs
- [x] Verify YAML syntax

## Decisions

- **PR-based over PAT-with-bypass.** A PAT with "bypass branch protection" would give the bot elevated privileges and quietly skip review. The PR flow is transparent: every CHANGELOG update is reviewable, the PR shows up in the usual queue, and the admin can merge it with one click.
- **Single recycled branch (`release/update-changelog`) rather than one branch per release.** `peter-evans/create-pull-request` updates the existing PR when the branch is reused. If two releases ship close together and the prior PR hasn't been merged, the second release's CHANGELOG supersedes the first on the same PR — no PR clutter. `delete-branch: true` cleans up once merged.
- **`add-paths: CHANGELOG.md`** ensures the PR only carries the changelog update, never accidentally sweeps in other working-tree drift.
- **Check out `main`, not the tag.** git-cliff only needs the full tag graph (`fetch-depth: 0`); the tag itself still drives the release (via `github.ref_name`), and the softprops release action attaches assets to the tag regardless of what's checked out.

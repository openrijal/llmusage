# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.3] - 2026-04-17


### Added

- Add specs for install script feature (d7b0916)
- Add curl | sh installer script (4881596)

### Changed

- Make release CHANGELOG auto-commit non-fatal (59525dc)
- Cursor: aggregate per-composer and emit sentinel records (d98eac8)
- Automate CHANGELOG.md and release notes with git-cliff (dcc8dac)
- gemini fix specs (66fb0c7)
- Allow manual trigger for Security Audit workflow (e016bcb)
- Grant checks:write to Security Audit workflow (67550e0)
## [0.1.2] - 2026-04-17


### Added

- Add strict-mode IDE support for Cursor (89c8172)
- Add Gemini CLI JSONL log support (8fab86f)

### Changed

- Clarify Cursor Linux path support (44409a0)
- Expand .gitignore with common artifacts (78ebd13)

### Fixed

- Fix release publish step to tolerate cargo package's lockfile regen (d18d21a)
- Fix four collector and export bugs (5af89d0)
## [0.1.1] - 2026-04-16


### Changed

- Point homepage to niteshrijal.com, keep repository as GitHub URL (ad4e3b3)
- Update contact email to namaste@niteshrijal.com (b42880e)
- Change downloads badge label to distinguish from version badge (23d7442)
## [0.1.0] - 2026-04-16


### Added

- Add weekly security audit workflow with cargo-audit (2693471)
- Add Homebrew formula auto-update workflow on release (c615611)
- Add automated crates.io publish workflow on release (96fe1c1)
- Add release workflow to build cross-platform binaries on tag push (2d24f4c)
- Add CI workflow for build, lint, and test checks on PRs (7ff10a6)
- Add CONTRIBUTING.md with development and contribution guide (7a2f05a)
- Add authors, readme, and homepage to Cargo.toml for crates.io publishing (d7d6a7f)
- Add MIT LICENSE file (1a9027e)
- Add screenshots directory for README usage examples (ab3dde5)
- Add provider-grouped table display with per-model breakdown (00ab012)
- Add comprehensive README with usage, commands, and configuration (e003ca6)
- Add AGENTS.md with project overview, conventions, and pitfalls (f63947b)
- Add project spec: PRD, task list, implementation details, validation (ad18eaa)
- Add llmusage CLI - token usage tracker across AI providers (74d4015)

### Changed

- Consolidate publish and homebrew workflows into release pipeline (469b38f)
- Switch reqwest from OpenSSL to rustls-tls for portable cross-compilation (1cd1513)
- Improve README with badges, expanded install methods, and uninstall section (79444cb)
- Update README with screenshots section and daily usage screenshot (403d4ba)
- Rename .spec/ to .specs/ for consistency (75a4fd4)
- Move spec files into branch-named subfolders (4700554)
- first commit (a347298)
- first commit (5462af1)

### Fixed

- Fix formatting to pass cargo fmt --check in CI (cfc1cf6)
- Fix table alignment, zero-token filter, and --all JSON consistency (7e03a93)
- Fix dedup NULL session_id, --until date filtering, and Ollama default host (d101308)


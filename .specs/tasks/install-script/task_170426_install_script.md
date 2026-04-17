# Task Record - curl | sh Installer Script

## Objective

Provide a zero-dependency install path that does not require a Rust or C toolchain.

A developer reported that `cargo install llmusage` failed on Linux while compiling `rusqlite`'s bundled SQLite with a `cc` error. The crate's dependency set has not changed between `v0.1.1` and `v0.1.2` — only the version number — but `cargo install` re-resolves transitive deps on every run and always needs a C toolchain for any crate with native code. Users without `build-essential` / `base-devel` / Xcode Command Line Tools will hit this.

## Scope

- [x] Ship `install.sh` at the repo root that downloads the prebuilt release tarball for the user's platform and installs the binary
- [x] Detect OS (Linux / macOS) and arch (`x86_64`, `aarch64` / `arm64`)
- [x] Detect Linux libc (glibc vs musl) and pick the right release asset
- [x] Verify the downloaded tarball against `sha256sums.txt` before installing
- [x] Default install location: `/usr/local/bin` when root, `$HOME/.local/bin` otherwise
- [x] Support overrides via `LLMUSAGE_VERSION` and `LLMUSAGE_INSTALL_DIR` environment variables
- [x] Work with either `curl` or `wget`, and either `sha256sum` or `shasum`
- [x] Pass `shellcheck` with no warnings
- [x] Update `README.md` to lead with the install script, call out the C toolchain requirement for `cargo install`, and note the new uninstall path
- [x] Update `.specs/prd.md` distribution section

## Decisions

- The script is hosted from `raw.githubusercontent.com/openrijal/llmusage/main/install.sh` rather than attached to each release, so a single file stays authoritative and improvements reach users without re-cutting a release.
- sha256 verification is mandatory, not optional — a `curl | sh` script without checksum verification is an unacceptable supply-chain footgun.
- Install dir defaults to `$HOME/.local/bin` for non-root runs rather than prompting for `sudo`, because the common `curl | sh` flow is non-interactive.
- Scope is explicitly limited to **Linux and macOS**. Windows users are directed to `cargo install` or the prebuilt Windows binary (future work once a Windows target is added to the release pipeline).
- The script does not attempt to add the install dir to `PATH`; it prints a warning with the exact line to append to the user's shell rc file. Modifying shell rc files from an installer invites edge cases (multiple shells, existing entries, sourcing order) that are not worth the complexity for a binary drop-in.

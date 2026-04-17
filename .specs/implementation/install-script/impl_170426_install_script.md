# Implementation Details - curl | sh Installer Script

## Summary

Adds `install.sh` at the repo root. The script downloads the correct prebuilt release tarball for the host's OS/arch/libc, verifies it against the release's `sha256sums.txt`, and drops the `llmusage` binary into a bin directory. It replaces `cargo install llmusage` as the recommended install path for users who do not have a Rust or C toolchain.

## File Layout

- `install.sh` (new) — POSIX `sh` script, executable bit set, shellcheck-clean.
- `README.md` — install section rewritten to lead with the installer and document C toolchain requirements for the remaining install paths.
- `.specs/prd.md` — distribution section updated.

## Script Behavior

### Platform detection

- `uname -s` → `Linux` or `Darwin`
- `uname -m` → `x86_64`, `amd64`, `aarch64`, or `arm64`
- On Linux, libc is detected via `ldd --version 2>&1 | grep -qi musl` and an `/etc/alpine-release` fallback to choose between:
  - `x86_64-unknown-linux-gnu`
  - `x86_64-unknown-linux-musl`
  - `aarch64-unknown-linux-gnu`
- On macOS, arm64 and x86_64 both map to the corresponding `-apple-darwin` tarball.
- Any other OS exits with a message pointing the user at `cargo install llmusage`.

### Tooling fallbacks

- HTTP fetch: `curl -fsSL` preferred, `wget -qO-` fallback.
- Checksum: `sha256sum` preferred (Linux default), `shasum -a 256` fallback (macOS default).
- The script errors out immediately if none of `uname`, `tar`, `mkdir`, a downloader, or a hasher is available.

### Version resolution

- Default: call `https://api.github.com/repos/openrijal/llmusage/releases/latest` and parse `tag_name` via `sed`.
- Override: `LLMUSAGE_VERSION=vX.Y.Z` env var skips the API call.

### Install location

| Condition | Directory |
|---|---|
| `LLMUSAGE_INSTALL_DIR` is set | that path |
| Running as root (`id -u == 0`) | `/usr/local/bin` |
| Otherwise | `$HOME/.local/bin` |

### Checksum verification

- Downloads `sha256sums.txt` from the same release.
- Extracts the expected hash with `awk -v f="$TARBALL" '$2 == f {print $1}'`.
- Compares against the actual hash of the downloaded tarball.
- Mismatch exits with a clear error showing both values.

### Installation

- Extracts the tarball into a `mktemp -d` working directory cleaned up via `trap`.
- Verifies the extracted `llmusage` binary exists.
- `mv -f` into the install dir; if the move fails (permissions), exits with a hint to run `sudo sh` or set `LLMUSAGE_INSTALL_DIR`.
- `chmod +x` on the destination.

### PATH awareness

- Detects whether the install dir is already on `PATH` by scanning `:$PATH:`.
- If not, prints the exact shell rc line the user should add. Does not modify any shell rc files.

## README Changes

Install section now leads with the installer:

```bash
curl -LsSf https://raw.githubusercontent.com/openrijal/llmusage/main/install.sh | sh
```

The `cargo install` section is retained but explicitly documents the C toolchain requirement with distro-specific commands for Debian/Ubuntu, Arch, Alpine, and macOS.

The uninstall section documents the new install-script removal path:

```bash
rm "$HOME/.local/bin/llmusage"   # or /usr/local/bin/llmusage when installed as root
```

## PRD Changes

- Distribution section lists the install script as the primary path, with `cargo install` and Homebrew as secondary.
- Notes the C toolchain caveat for `cargo install`.

## Out of Scope

- Windows support — the release pipeline does not currently produce Windows binaries.
- Auto-updating `PATH` in shell rc files.
- Hosting the installer at a custom domain or a shortened URL.

# Validation Record - curl | sh Installer Script

## Static Analysis

### shellcheck install.sh
- **Status**: PASS
- No warnings or notes.

### bash -n install.sh
- **Status**: PASS
- POSIX syntax is clean.

## End-to-End Install

### macOS (aarch64-apple-darwin)
- **Status**: PASS
- Command: `LLMUSAGE_INSTALL_DIR=/tmp/llmusage-install-test sh install.sh`
- Verified:
  - Latest release tag resolved via GitHub API
  - Correct tarball selected: `llmusage-v0.1.2-aarch64-apple-darwin.tar.gz`
  - `sha256sums.txt` downloaded, expected hash located by filename
  - Actual hash matched expected
  - Binary extracted to install dir
  - `/tmp/llmusage-install-test/llmusage --version` returned `llmusage 0.1.2`
  - PATH warning was emitted because the temp dir was not in `PATH`

## Platform Coverage — To Verify Manually

The following platforms are supported by the script but have not yet been exercised end-to-end. The mapping logic and tarball URLs are covered by the code, but a physical or container install run is still recommended before treating these as verified.

- `x86_64-unknown-linux-gnu` (glibc: Debian/Ubuntu, Fedora)
- `x86_64-unknown-linux-musl` (Alpine)
- `aarch64-unknown-linux-gnu` (ARM64 Linux: Raspberry Pi OS 64-bit, AWS Graviton)
- `x86_64-apple-darwin` (Intel macOS)

## Failure Modes — Expected Behavior

### Missing downloader
- If neither `curl` nor `wget` is available, the script exits with a clear error.

### Missing hasher
- If neither `sha256sum` nor `shasum` is available, the script exits with a clear error.

### Unsupported OS
- Any `uname -s` outside `Linux` or `Darwin` exits with a message recommending `cargo install llmusage` after installing a C toolchain.

### Unsupported arch
- Any `uname -m` outside `x86_64`, `amd64`, `aarch64`, or `arm64` exits with an "unsupported architecture" error.

### Checksum mismatch
- Exits with an error printing both the expected and actual hashes. The binary is not installed.

### Permission denied on install
- Exits with a hint to rerun with `sudo sh` or set `LLMUSAGE_INSTALL_DIR` to a user-writable directory.

## Validation Notes

- The installer does not touch the user's `PATH`. It prints a warning with the exact shell rc line to append. This is intentional per the task decision.
- The installer is served from `raw.githubusercontent.com/openrijal/llmusage/main/install.sh`, so changes go live on merge to `main` without needing a release cut.

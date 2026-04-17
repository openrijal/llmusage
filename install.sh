#!/bin/sh
# llmusage installer — downloads the prebuilt release binary for your
# platform and drops it into a bin directory. Use this instead of
# `cargo install llmusage` when you don't have a C toolchain (gcc,
# build-essential) installed on your machine.
#
# Usage:
#   curl -LsSf https://raw.githubusercontent.com/openrijal/llmusage/main/install.sh | sh
#
# Environment variables:
#   LLMUSAGE_VERSION       Specific version to install (e.g. v0.1.2). Default: latest.
#   LLMUSAGE_INSTALL_DIR   Install directory. Default: /usr/local/bin if root, else $HOME/.local/bin.

set -eu

REPO="openrijal/llmusage"
BIN_NAME="llmusage"

err() { printf 'error: %s\n' "$*" >&2; exit 1; }
info() { printf '%s\n' "$*"; }

command -v uname >/dev/null 2>&1 || err "missing required command: uname"
command -v tar >/dev/null 2>&1 || err "missing required command: tar"
command -v mkdir >/dev/null 2>&1 || err "missing required command: mkdir"

if command -v curl >/dev/null 2>&1; then
    http_get() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
    http_get() { wget -qO- "$1"; }
else
    err "need either curl or wget to download files"
fi

if command -v sha256sum >/dev/null 2>&1; then
    sha_cmd() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
    sha_cmd() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    err "need either sha256sum or shasum to verify downloads"
fi

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64|amd64)
                if (ldd --version 2>&1 | grep -qi musl) || [ -f /etc/alpine-release ]; then
                    TARGET="x86_64-unknown-linux-musl"
                else
                    TARGET="x86_64-unknown-linux-gnu"
                fi
                ;;
            aarch64|arm64)
                TARGET="aarch64-unknown-linux-gnu"
                ;;
            *)
                err "unsupported Linux architecture: $ARCH"
                ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64)
                TARGET="x86_64-apple-darwin"
                ;;
            arm64|aarch64)
                TARGET="aarch64-apple-darwin"
                ;;
            *)
                err "unsupported macOS architecture: $ARCH"
                ;;
        esac
        ;;
    *)
        err "unsupported OS: $OS (install a C toolchain and try 'cargo install llmusage' instead)"
        ;;
esac

VERSION="${LLMUSAGE_VERSION:-}"
if [ -z "$VERSION" ]; then
    info "Resolving latest release..."
    VERSION=$(http_get "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
        | sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' | head -n1)
    [ -n "$VERSION" ] || err "could not resolve latest release (set LLMUSAGE_VERSION=vX.Y.Z)"
fi

if [ -n "${LLMUSAGE_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="$LLMUSAGE_INSTALL_DIR"
elif [ "$(id -u)" = "0" ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi

TARBALL="${BIN_NAME}-${VERSION}-${TARGET}.tar.gz"
BASE_URL="https://github.com/$REPO/releases/download/${VERSION}"
TMP=$(mktemp -d 2>/dev/null || mktemp -d -t llmusage-install)
trap 'rm -rf "$TMP"' EXIT INT TERM

info "Downloading $TARBALL..."
http_get "$BASE_URL/$TARBALL" > "$TMP/$TARBALL" \
    || err "failed to download $BASE_URL/$TARBALL"
[ -s "$TMP/$TARBALL" ] || err "downloaded tarball is empty ($BASE_URL/$TARBALL)"

info "Verifying checksum..."
http_get "$BASE_URL/sha256sums.txt" > "$TMP/sha256sums.txt" \
    || err "failed to download checksums"
EXPECTED=$(awk -v f="$TARBALL" '$2 == f {print $1}' "$TMP/sha256sums.txt")
[ -n "$EXPECTED" ] || err "no checksum entry for $TARBALL in sha256sums.txt"
ACTUAL=$(sha_cmd "$TMP/$TARBALL")
[ "$EXPECTED" = "$ACTUAL" ] || err "checksum mismatch for $TARBALL
  expected: $EXPECTED
  actual:   $ACTUAL"

info "Extracting..."
tar -xzf "$TMP/$TARBALL" -C "$TMP"
[ -f "$TMP/$BIN_NAME" ] || err "binary '$BIN_NAME' not found inside tarball"

mkdir -p "$INSTALL_DIR" 2>/dev/null || err "cannot create $INSTALL_DIR"
DEST="$INSTALL_DIR/$BIN_NAME"
if ! mv -f "$TMP/$BIN_NAME" "$DEST" 2>/dev/null; then
    err "cannot write to $DEST (try 'sudo sh' or set LLMUSAGE_INSTALL_DIR=\$HOME/bin)"
fi
chmod +x "$DEST"

info ""
info "Installed $BIN_NAME $VERSION to $DEST"

case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        info "Run '$BIN_NAME --help' to get started."
        ;;
    *)
        info ""
        info "Warning: $INSTALL_DIR is not in your PATH."
        info "Add it by appending this line to your shell rc (~/.bashrc, ~/.zshrc, etc.):"
        info ""
        info "    export PATH=\"\$PATH:$INSTALL_DIR\""
        info ""
        info "Or invoke it with the full path: $DEST"
        ;;
esac

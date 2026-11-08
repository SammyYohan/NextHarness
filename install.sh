#!/bin/sh
# Install next-harness for the current user on Linux or macOS. No root: the
# binary lands in ~/.local/bin, which this script adds to your PATH.
#
# Either pipe it straight from GitHub:
#
#   curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
#
# or run it from a checkout, in which case your own build is preferred:
#
#   ./install.sh

set -eu

REPO="SammyYohan/NextHarness"
LATEST="https://github.com/$REPO/releases/latest/download"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_BIN="$INSTALL_DIR/next-harness"

# Directory holding this script, empty when the script arrives through a pipe.
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" 2>/dev/null && pwd)" || SCRIPT_DIR=""

fetch() { # fetch <url> <file>
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --output "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        echo "next-harness installer: curl or wget is required to download the release." >&2
        exit 1
    fi
}

sha256_of() { # sha256_of <file>
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ -n "$SCRIPT_DIR" ] && [ -x "$SCRIPT_DIR/target/release/next-harness" ]; then
    # A checkout with a release build: prefer it, no download needed.
    BINARY="$SCRIPT_DIR/target/release/next-harness"
    echo "Using your local build at $BINARY."
else
    # One prebuilt binary per platform, matching the release workflow.
    case "$(uname -s)/$(uname -m)" in
        Darwin/arm64 | Darwin/aarch64) TARGET="aarch64-apple-darwin" ;;
        Darwin/x86_64 | Darwin/amd64) TARGET="x86_64-apple-darwin" ;;
        Linux/x86_64 | Linux/amd64) TARGET="x86_64-unknown-linux-gnu" ;;
        *)
            echo "next-harness installer: no prebuilt binary for $(uname -s)/$(uname -m)." >&2
            echo "Build from source: cargo install --git https://github.com/$REPO --bin next-harness" >&2
            exit 1
            ;;
    esac

    echo "Downloading the latest release for $TARGET..."
    ARCHIVE="$TMP_DIR/next-harness-$TARGET.tar.gz"
    if ! fetch "$LATEST/next-harness-$TARGET.tar.gz" "$ARCHIVE" ||
        ! fetch "$LATEST/next-harness-$TARGET.tar.gz.sha256" "$ARCHIVE.sha256"; then
        echo "next-harness installer: download failed. Check that a release exists at" >&2
        echo "https://github.com/$REPO/releases" >&2
        exit 1
    fi

    EXPECTED="$(cut -d' ' -f1 "$ARCHIVE.sha256")"
    ACTUAL="$(sha256_of "$ARCHIVE")"
    if [ "$ACTUAL" != "$EXPECTED" ]; then
        echo "next-harness installer: SHA-256 mismatch (expected $EXPECTED, got $ACTUAL)." >&2
        exit 1
    fi

    tar -xzf "$ARCHIVE" -C "$TMP_DIR"
    BINARY="$TMP_DIR/next-harness-$TARGET/next-harness"
    if [ ! -f "$BINARY" ]; then
        echo "next-harness installer: the archive did not contain the binary." >&2
        exit 1
    fi
fi

mkdir -p "$INSTALL_DIR"
cp -f "$BINARY" "$INSTALL_BIN"
chmod +x "$INSTALL_BIN"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        SHELL_RC=""
        if [ -f "$HOME/.bashrc" ]; then SHELL_RC="$HOME/.bashrc"
        elif [ -f "$HOME/.zshrc" ]; then SHELL_RC="$HOME/.zshrc"
        elif [ -f "$HOME/.profile" ]; then SHELL_RC="$HOME/.profile"
        fi

        if [ -n "$SHELL_RC" ] && ! grep -qF "$INSTALL_DIR" "$SHELL_RC"; then
            echo "" >> "$SHELL_RC"
            echo "# next-harness" >> "$SHELL_RC"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$SHELL_RC"
            echo "Added $INSTALL_DIR to your PATH in $SHELL_RC."
        fi
        ;;
esac

case "$TARGET" in
    *linux-gnu)
        echo "Note: on a minimal distro, install the GTK 3 runtime so the pay prompt works"
        echo "(Debian/Ubuntu: sudo apt-get install libgtk-3-0 libxdo3)."
        ;;
esac

VERSION="$("$INSTALL_BIN" --version 2>/dev/null || true)"
echo "Installed: $INSTALL_BIN ${VERSION:-(no --version)}"
echo "Open a new terminal, then run: next-harness --help"

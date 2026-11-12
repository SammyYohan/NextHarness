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
API_RELEASES="https://api.github.com/repos/$REPO/releases?per_page=20"
LATEST="https://github.com/$REPO/releases/latest/download"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_BIN="$INSTALL_DIR/next-harness"
TARGET=""

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

fetch_stdout() { # fetch_stdout <url>
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$1"
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

    ARCHIVE_NAME="next-harness-$TARGET.tar.gz"
    ASSET_URL="$LATEST/$ARCHIVE_NAME"
    CHECK_URL="$LATEST/$ARCHIVE_NAME.sha256"

    # `releases/latest` skips prereleases, and this project ships betas: ask the
    # API for the newest release (prereleases included) that carries both the
    # archive and its checksum, so both files come from the same release.
    RELEASES="$(fetch_stdout "$API_RELEASES" 2>/dev/null || true)"
    if [ -n "$RELEASES" ]; then
        TAGS="$(printf '%s\n' "$RELEASES" | grep -o '"tag_name":"[^"]*"' | cut -d'"' -f4)"
        for tag in $TAGS; do
            A="$(printf '%s\n' "$RELEASES" | grep -o "\"browser_download_url\":\"[^\"]*/download/$tag/$ARCHIVE_NAME\"" | cut -d'"' -f4 | head -n 1)"
            C="$(printf '%s\n' "$RELEASES" | grep -o "\"browser_download_url\":\"[^\"]*/download/$tag/$ARCHIVE_NAME.sha256\"" | cut -d'"' -f4 | head -n 1)"
            if [ -n "$A" ] && [ -n "$C" ]; then
                ASSET_URL="$A"
                CHECK_URL="$C"
                break
            fi
        done
    fi

    echo "Downloading the latest release for $TARGET..."
    ARCHIVE="$TMP_DIR/$ARCHIVE_NAME"
    if ! fetch "$ASSET_URL" "$ARCHIVE" ||
        ! fetch "$CHECK_URL" "$ARCHIVE.sha256"; then
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
        echo "Note: the pay prompt is drawn by zenity or kdialog when one of them is"
        echo "installed (Debian/Ubuntu: sudo apt-get install zenity). Without either,"
        echo "NextHarness asks at the terminal or waits: it never spends by itself."
        ;;
esac

VERSION="$("$INSTALL_BIN" --version 2>/dev/null || true)"
echo "Installed: $INSTALL_BIN ${VERSION:-(no --version)}"
echo "Open a new terminal, then run: next-harness --help"

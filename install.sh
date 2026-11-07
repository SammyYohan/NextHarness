#!/usr/bin/env bash
set -euo pipefail

# Installe next-harness pour l'utilisateur actuel sur Linux/macOS.
# Copie le binaire release dans ~/.local/bin et ajoute ce dossier au PATH.
# Aucun droit root requis.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_BIN="$SCRIPT_DIR/target/release/next-harness"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_BIN="$INSTALL_DIR/next-harness"

if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "Binaire introuvable : $RELEASE_BIN (generez-le avec 'cargo build --release')" >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
cp -f "$RELEASE_BIN" "$INSTALL_BIN"
chmod +x "$INSTALL_BIN"

SHELL_RC=""
if [[ -f "$HOME/.bashrc" ]]; then SHELL_RC="$HOME/.bashrc"
elif [[ -f "$HOME/.zshrc" ]]; then SHELL_RC="$HOME/.zshrc"
elif [[ -f "$HOME/.profile" ]]; then SHELL_RC="$HOME/.profile"
fi

if [[ -n "$SHELL_RC" ]] && ! grep -q "$INSTALL_DIR" "$SHELL_RC"; then
    echo "" >> "$SHELL_RC"
    echo "# next-harness" >> "$SHELL_RC"
    echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$SHELL_RC"
fi

echo "Installation terminee : $INSTALL_BIN"
echo "Redemarrez votre terminal, puis tapez : next-harness --help"

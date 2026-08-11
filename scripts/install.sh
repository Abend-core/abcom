#!/usr/bin/env bash
# install.sh — Installe Abcom et configure le démarrage automatique
set -euo pipefail

BINARY_DIR="$HOME/.local/bin"

echo "╔══════════════════════════════════╗"
echo "║   Installation de Abcom          ║"
echo "╚══════════════════════════════════╝"

# ── 1. Rust ──────────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
  fi
fi

if ! command -v cargo &>/dev/null; then
  echo "→ Rust non trouvé. Installation via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
else
  echo "✓ Rust trouvé : $(rustc --version)"
fi

# ── 2. Compilation ────────────────────────────────────────────────────────────
echo "→ Compilation en mode release..."
cargo build --release

# ── 3. Installation du binaire ────────────────────────────────────────────────
mkdir -p "$BINARY_DIR"
cp target/release/abcom "$BINARY_DIR/abcom"
chmod +x "$BINARY_DIR/abcom"
echo "✓ Binaire installé dans $BINARY_DIR/abcom"

# Ajouter ~/.local/bin au PATH si absent
if [[ ":$PATH:" != *":$BINARY_DIR:"* ]]; then
  echo "→ Ajout de $BINARY_DIR au PATH dans ~/.bashrc"
  echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
fi

# ── 4. Lancement automatique ─────────────────────────────────────────────────
# Aucun service systemd n'est installé : l'application gère elle-même son
# démarrage à l'ouverture de session (entrée XDG dans ~/.config/autostart),
# activable et désactivable depuis Paramètres → Général. Installer les deux
# lançait deux instances qui se disputaient les mêmes ports.

echo ""
echo "✓ Abcom démarrera à l'ouverture de session (réglable dans Paramètres → Général)."
echo "  journalctl --user -u abcom -f    → voir les logs"

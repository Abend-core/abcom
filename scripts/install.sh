#!/usr/bin/env bash
# install.sh — Installe Abcom et configure le démarrage automatique
set -euo pipefail

BINARY_DIR="$HOME/.local/bin"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_NAME="abcom.service"

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

# ── 4. Service systemd utilisateur ───────────────────────────────────────────
mkdir -p "$SERVICE_DIR"
cp contrib/abcom.service "$SERVICE_DIR/$SERVICE_NAME"

# Activer le service de linger pour que les services user tournent sans session
loginctl enable-linger "$(whoami)" 2>/dev/null || true

systemctl --user daemon-reload
systemctl --user enable --now "$SERVICE_NAME"

echo ""
echo "✓ Service systemd activé : $SERVICE_NAME"
echo "✓ Abcom démarrera automatiquement à la connexion graphique."
echo ""
echo "Commandes utiles :"
echo "  systemctl --user status abcom    → état du service"
echo "  systemctl --user restart abcom   → redémarrer"
echo "  systemctl --user stop abcom      → arrêter"
echo "  journalctl --user -u abcom -f    → voir les logs"

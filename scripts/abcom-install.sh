#!/usr/bin/env bash
# abcom-install.sh — Installation rapide d'Abcom (binaire pré-compilé)
set -euo pipefail

BINARY_SOURCE="${1:-.}"
BINARY_DIR="$HOME/.local/bin"
DATA_DIR="$HOME/.local/share/abcom"
SERVICE_DIR="$HOME/.config/systemd/user"
APPS_DIR="$HOME/.local/share/applications"

echo "╔════════════════════════════════════╗"
echo "║  Installation d'Abcom (v2)         ║"
echo "║  Déploiement rapide - binaire      ║"
echo "╚════════════════════════════════════╝"
echo ""

# ── Cherche le binaire ────────────────────────────────────────────────────────
if [ -f "$BINARY_SOURCE/abcom" ]; then
  BINARY_FILE="$BINARY_SOURCE/abcom"
elif [ -f "$BINARY_SOURCE" ]; then
  BINARY_FILE="$BINARY_SOURCE"
elif [ -f "target/release/abcom" ]; then
  BINARY_FILE="target/release/abcom"
else
  echo "❌ Binaire abcom non trouvé !"
  echo "   Usage: $0 [chemin/vers/abcom]"
  echo "   Exemples:"
  echo "     $0 .                    # Dans le répertoire abcom"
  echo "     $0 ~/Downloads/abcom    # Depuis Downloads"
  exit 1
fi

echo "✓ Binaire trouvé: $BINARY_FILE"
echo ""

# ── Installation ──────────────────────────────────────────────────────────────
echo "→ Installation..."

# Binaire
mkdir -p "$BINARY_DIR"
cp "$BINARY_FILE" "$BINARY_DIR/abcom"
chmod +x "$BINARY_DIR/abcom"
echo "  ✓ Binaire → $BINARY_DIR/abcom"

# Dossier de données
mkdir -p "$DATA_DIR"
echo "  ✓ Dossier de données → $DATA_DIR"

# Service systemd (optionnel, mais pratique)
mkdir -p "$SERVICE_DIR"
cat > "$SERVICE_DIR/abcom.service" << 'EOF'
[Unit]
Description=Abcom - LAN Chat
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=%h/.local/bin/abcom %u
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
echo "  ✓ Service systemd installé"

# Activer le service
systemctl --user daemon-reload 2>/dev/null || true
loginctl enable-linger "$(whoami)" 2>/dev/null || true
echo "  ✓ Service prêt pour démarrage auto"

# ── Raccourci Menu (Desktop file) ─────────────────────────────────────────────
mkdir -p "$APPS_DIR"
cat > "$APPS_DIR/abcom.desktop" << 'EOF'
[Desktop Entry]
Type=Application
Name=Abcom
Comment=Chat LAN P2P et découverte automatique des pairs
Exec=bash -c 'read -p "Ton pseudonyme: " username; %h/.local/bin/abcom "$username"'
Icon=chat
Terminal=true
Categories=Network;Chat;
Keywords=chat;lan;p2p;distributed;
EOF
echo "  ✓ Raccourci menu créé"

# ── Ajouter au PATH (si absent) ───────────────────────────────────────────────
if [[ ":$PATH:" != *":$BINARY_DIR:"* ]]; then
  if ! grep -q "export PATH.*$BINARY_DIR" "$HOME/.bashrc" 2>/dev/null; then
    echo "" >> "$HOME/.bashrc"
    echo "# Abcom" >> "$HOME/.bashrc"
    echo "export PATH=\"$HOME/.local/bin:\$PATH\"" >> "$HOME/.bashrc"
    echo "  ✓ PATH mis à jour (redémarre le terminal pour l'appliquer)"
  fi
fi

echo ""
echo "╔════════════════════════════════════╗"
echo "║  ✅ Installation terminée !        ║"
echo "╚════════════════════════════════════╝"
echo ""
echo "🚀 Pour lancer Abcom:"
echo ""
echo "   Méthode 1 (Terminal):"
echo "     $ abcom Alice"
echo ""
echo "   Méthode 2 (Menu/Raccourci):"
echo "     → Applications → Abcom"
echo "     (ou cherche 'Abcom' dans ton app launcher)"
echo ""
echo "   Méthode 3 (Service auto au démarrage - optionnel):"
echo "     $ systemctl --user enable abcom.service"
echo "     $ systemctl --user start abcom.service"
echo ""
echo "📝 Configuration:"
echo "   • Historique: $DATA_DIR/messages.json"
echo "   • Ports: 9000/tcp (P2P), 9001/udp (découverte)"
echo ""
echo "💡 Prochaines étapes:"
echo "   1. Lance l'app sur chaque machine (avec un pseudo unique)"
echo "   2. La liste des 'Pairs LAN' devrait s'auto-remplir"
echo "   3. Envoie un message → devrait arriver aux autres machines !"
echo ""

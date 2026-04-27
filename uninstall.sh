#!/usr/bin/env bash
# uninstall.sh — Supprime Abcom et son service systemd
set -euo pipefail

echo "→ Arrêt et désactivation du service..."
systemctl --user stop abcom.service 2>/dev/null || true
systemctl --user disable abcom.service 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/abcom.service"
systemctl --user daemon-reload

echo "→ Suppression du binaire..."
rm -f "$HOME/.local/bin/abcom"

echo "✓ Abcom désinstallé."

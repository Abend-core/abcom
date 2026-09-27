#!/usr/bin/env bash
# uninstall.sh — Supprime Abcom (binaire, raccourci, démarrage automatique)
set -euo pipefail

# Reste d'une installation antérieure : les versions passées installaient un
# service systemd, remplacé depuis par le démarrage XDG géré par l'application.
echo "→ Nettoyage d'un éventuel service systemd..."
systemctl --user disable --now abcom.service 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/abcom.service"
systemctl --user daemon-reload 2>/dev/null || true

echo "→ Suppression du démarrage automatique et du raccourci..."
rm -f "$HOME/.config/autostart/Abcom.desktop"
rm -f "$HOME/.local/share/applications/abcom.desktop"

echo "→ Suppression du binaire..."
rm -f "$HOME/.local/bin/abcom"

echo "✓ Abcom désinstallé."

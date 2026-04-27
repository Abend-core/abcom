#!/usr/bin/env bash
# deploy.sh - Prépare Abcom pour test multi-machine

set -e

BINARY_PATH="${1:-target/release/abcom}"
DEST_DIR="${2:-$HOME/.local/bin}"

echo "╔════════════════════════════════════╗"
echo "║  Abcom Multi-Machine Deployment   ║"
echo "╚════════════════════════════════════╝"
echo ""

# Vérifier le binaire
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Binaire non trouvé: $BINARY_PATH"
    echo "   Exécute d'abord: cargo build --release"
    exit 1
fi

echo "✓ Binaire trouvé: $BINARY_PATH"
echo "  Taille: $(du -h "$BINARY_PATH" | cut -f1)"

# Créer le répertoire
mkdir -p "$DEST_DIR"

# Copier le binaire
cp "$BINARY_PATH" "$DEST_DIR/abcom"
chmod +x "$DEST_DIR/abcom"

echo "✓ Binaire copié vers: $DEST_DIR/abcom"

# Créer les data directories pour chaque user
mkdir -p ~/.local/share/abcom
mkdir -p ~/.config/abcom

echo "✓ Répertoires de données créés"

# Afficher les instructions
echo ""
echo "╔════════════════════════════════════╗"
echo "║     Instructions pour 3 machines   ║"
echo "╚════════════════════════════════════╝"
echo ""
echo "Machine 1 (Alice):"
echo "  ~/.local/bin/abcom Alice"
echo ""
echo "Machine 2 (Bob):"
echo "  ~/.local/bin/abcom Bob"
echo ""
echo "Machine 3 (Charlie):"
echo "  ~/.local/bin/abcom Charlie"
echo ""
echo "📖 Pour plus d'infos: cat DEPLOYMENT.md"
echo ""
echo "✅ Déploiement prêt!"

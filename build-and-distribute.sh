#!/usr/bin/env bash
# build-and-distribute.sh — Prépare Abcom pour la distribution
set -euo pipefail

SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$SOURCE_DIR/dist"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
DIST_NAME="abcom_${TIMESTAMP}"

echo "╔════════════════════════════════════════╗"
echo "║  Préparation de Distribution Abcom     ║"
echo "╚════════════════════════════════════════╝"
echo ""

# ── Compilation ───────────────────────────────────────────────────────────────
echo "→ Compilation en mode release..."
cd "$SOURCE_DIR"
cargo build --release
echo "✓ Binaire compilé"
echo ""

# ── Préparation du dossier de distribution ──────────────────────────────────────
echo "→ Préparation du dossier de distribution..."
rm -rf "$DIST_DIR/$DIST_NAME"
mkdir -p "$DIST_DIR/$DIST_NAME"

# Copie les fichiers essentiels
cp target/release/abcom "$DIST_DIR/$DIST_NAME/"
cp abcom-install.sh "$DIST_DIR/$DIST_NAME/"
cp INSTALL_FRIEND.md "$DIST_DIR/$DIST_NAME/README.md"
cp QUICK_DEPLOY.md "$DIST_DIR/$DIST_NAME/DEPLOY.md"
cp contrib/abcom.desktop "$DIST_DIR/$DIST_NAME/"

# Rend les scripts exécutables
chmod +x "$DIST_DIR/$DIST_NAME/abcom"
chmod +x "$DIST_DIR/$DIST_NAME/abcom-install.sh"

echo "✓ Dossier créé: $DIST_DIR/$DIST_NAME"
echo ""

# ── Archive ZIP ────────────────────────────────────────────────────────────────
echo "→ Création d'une archive ZIP..."
cd "$DIST_DIR"
zip -r -q "${DIST_NAME}.zip" "$DIST_NAME/"
echo "✓ Archive: $DIST_DIR/${DIST_NAME}.zip"
echo ""

# ── Résumé ─────────────────────────────────────────────────────────────────────
echo "╔════════════════════════════════════════╗"
echo "║  ✅ Distribution prête !               ║"
echo "╚════════════════════════════════════════╝"
echo ""
echo "📦 Dossier: $DIST_DIR/$DIST_NAME"
echo "   Contient:"
echo "     • abcom (binaire)"
echo "     • abcom-install.sh (script d'installation)"
echo "     • README.md (guide pour les copains)"
echo "     • DEPLOY.md (doc de déploiement)"
echo ""
echo "📲 Archive ZIP: $DIST_DIR/${DIST_NAME}.zip (~5 MB)"
echo ""
echo "🚀 Pour partager:"
echo "   # Option 1: Dossier"
echo "   scp -r $DIST_DIR/$DIST_NAME alice@192.168.1.100:~/"
echo ""
echo "   # Option 2: ZIP"
echo "   cp $DIST_DIR/${DIST_NAME}.zip ~/Desktop/"
echo "   # Puis partage via USB, email, cloud, etc."
echo ""
echo "💻 Sur la machine du copain:"
echo "   unzip ${DIST_NAME}.zip"
echo "   cd $DIST_NAME"
echo "   bash abcom-install.sh ./abcom"
echo ""

#!/usr/bin/env bash
#
# Lance plusieurs instances locales d'Abcom pour tester la connexion P2P
# (chat 1-à-1, groupes, transferts) sur une seule machine.
#
# Chaque instance utilise un identifiant ABCOM_INSTANCE distinct → ports TCP et
# répertoire de données séparés, mais elles partagent le port UDP de découverte
# et se voient donc mutuellement.
#
#   Usage : scripts/run-multi.sh [N]      (N = nombre de fenêtres, défaut 2)
#
set -euo pipefail

N="${1:-2}"
cd "$(dirname "$0")/.."

# Noms d'utilisateur distincts (obligatoire : une instance ignore les broadcasts
# portant son propre nom, donc deux instances homonymes ne se verraient pas).
NAMES=(alice bob carol dave eve frank)

echo "🔨 Compilation (release)…"
cargo build --release
BIN="target/release/abcom"

PIDS=()
cleanup() {
    echo ""
    echo "🛑 Arrêt des instances…"
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
}
trap cleanup INT TERM EXIT

echo ""
for i in $(seq 1 "$N"); do
    name="${NAMES[$((i-1))]:-user$i}"
    chat_port=$((9000 + i * 10))
    echo "🚀 Instance $i : '$name'  (ABCOM_INSTANCE=$i, chat TCP $chat_port, données abcom-$i)"
    ABCOM_INSTANCE="$i" "$BIN" "$name" &
    PIDS+=("$!")
    sleep 0.4
done

echo ""
echo "✅ $N fenêtre(s) lancée(s). Elles devraient se découvrir en ~3 s."
echo "   Ctrl-C dans ce terminal pour tout fermer."
echo ""
wait

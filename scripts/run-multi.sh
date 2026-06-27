#!/usr/bin/env bash
#
# Lance plusieurs instances locales d'Abcom pour tester la connexion P2P.
# Les processus sont détachés du terminal (setsid) → ils apparaissent comme
# fenêtres indépendantes dans la barre des tâches du bureau.
#
#   Usage : scripts/run-multi.sh [N]      (N = nombre de fenêtres, défaut 2)
#
set -euo pipefail

N="${1:-2}"
cd "$(dirname "$0")/.."

NAMES=(alice bob carol dave eve frank)

echo "Compilation (release)..."
cargo build --release --quiet
BIN="$(pwd)/target/release/abcom"

echo ""
PIDS=()
for i in $(seq 1 "$N"); do
    name="${NAMES[$((i-1))]:-user$i}"
    log="/tmp/abcom-${name}.log"
    ABCOM_INSTANCE="$i" setsid "$BIN" "$name" </dev/null >"$log" 2>&1 &
    PIDS+=("$!")
    echo "Instance '$name' lancée  (PID ${PIDS[-1]}, log: $log)"
    sleep 0.3
done

echo ""
echo "$N fenêtre(s) détachée(s) du terminal — elles apparaissent dans la barre des tâches."
echo "Pour tout arrêter : kill ${PIDS[*]}"

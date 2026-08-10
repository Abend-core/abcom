#!/bin/bash
# integration_test.sh — Test d'intégration headless : deux pairs établissent
# une session Noise et échangent un message authentifié sur une vraie socket
# TCP. Exécuté par la CI de `main`.
# Usage : bash scripts/integration_test.sh  (aucun argument)

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

pass() { printf "[PASS] %s\n" "$1"; }

cargo test --locked --test p2p_e2e
pass "échange P2P authentifié"

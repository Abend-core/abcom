#!/bin/bash
# Script d'intégration — vérifie compilation, tests et binaire final.
# Conçu pour tourner en CI (headless) comme en local.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

BINARY="$REPO_DIR/target/release/abcom"

pass() { printf "[PASS] %s\n" "$1"; }
fail() { printf "[FAIL] %s\n" "$1" >&2; exit 1; }
step() { printf "\n--- %s ---\n" "$1"; }

step "1/3  Vérification de la compilation"
cargo check --quiet
pass "cargo check"

step "2/3  Tests unitaires"
cargo test --quiet 2>&1 | tail -3
cargo test --quiet 2>&1 | grep -q "test result: ok" || fail "Des tests ont échoué"
pass "cargo test"

step "3/3  Build release"
cargo build --release --quiet
[ -f "$BINARY" ] || fail "Binaire absent : $BINARY"
[ -x "$BINARY" ] || fail "Binaire non exécutable : $BINARY"
BINARY_SIZE=$(du -sh "$BINARY" | cut -f1)
pass "Binaire produit ($BINARY_SIZE)"

printf "\nOK — tous les contrôles passent.\n"

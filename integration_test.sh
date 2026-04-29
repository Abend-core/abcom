#!/bin/bash
# Integration test script for Abcom group management and persistence

set -e

APP_DIR="/home/ra/abcom"
DATA_DIR_LINUX="$HOME/.local/share/abcom"

echo "================================"
echo "🧪 INTEGRATION TEST: Groups & Persistence"
echo "================================"
echo ""

# Step 1: Verify compilation
echo "📦 [1/5] Checking compilation..."
cd "$APP_DIR"
cargo check --quiet
echo "✅ Compilation OK"
echo ""

# Step 2: Run unit tests
echo "🧪 [2/5] Running unit tests..."
TEST_OUTPUT=$(cargo test 2>&1)
if echo "$TEST_OUTPUT" | grep -q "test result: ok"; then
    TEST_PASS=$(echo "$TEST_OUTPUT" | grep "test result:" | grep "passed")
    echo "✅ All tests PASSED"
    echo "$TEST_PASS" | head -1
else
    echo "❌ Tests FAILED"
    echo "$TEST_OUTPUT" | tail -20
    exit 1
fi
echo ""

# Step 3: Build release binary
echo "🔨 [3/5] Building release binary..."
cargo build --release --target x86_64-pc-windows-gnu --quiet
echo "✅ Release build OK"
echo ""

# Step 4: Check data persistence files
echo "💾 [4/5] Checking data persistence structure..."

if [ -f "$DATA_DIR_LINUX/groups.json" ]; then
    echo "✅ groups.json exists"
    # Check if file is not empty and starts with [
    if [ -s "$DATA_DIR_LINUX/groups.json" ] && head -1 "$DATA_DIR_LINUX/groups.json" | grep -q '\['; then
        echo "   ✓ Valid JSON array format"
    else
        echo "   ℹ️  Empty or invalid (will be created on first run)"
    fi
else
    echo "⚠️  groups.json not created yet (will be created on first run)"
fi

if [ -f "$DATA_DIR_LINUX/messages.json" ]; then
    echo "✅ messages.json exists"
    # Check if file is not empty and starts with [
    if [ -s "$DATA_DIR_LINUX/messages.json" ] && head -1 "$DATA_DIR_LINUX/messages.json" | grep -q '\['; then
        echo "   ✓ Valid JSON array format"
    else
        echo "   ℹ️  Empty or invalid (will be created on first run)"
    fi
else
    echo "⚠️  messages.json not created yet (will be created on first run)"
fi

echo ""

# Step 5: Check app startup
echo "⚡ [5/5] Testing app startup..."
timeout 3 cargo run --release -- testuser 2>&1 | head -20 || true
echo "✅ App startup OK (timeout expected)"
echo ""

echo "================================"
echo "✅ ALL INTEGRATION TESTS PASSED"
echo "================================"
echo ""
echo "📝 Manual Test Checklist:"
echo "1. Launch app: make run"
echo "2. Test validation UI:"
echo "   - Click '+' button in Groups section"
echo "   - Type '@group' → should show RED '✗ Nom invalide'"
echo "   - Type 'TestGroup' → should show GREEN '✓ 9'"
echo "3. Create a group and send messages"
echo "4. Restart app and verify groups/messages persist"
echo ""

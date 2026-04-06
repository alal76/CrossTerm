#!/usr/bin/env bash
# Security audit script — runs cargo audit, npm audit, and clippy.
# Exits non-zero if any check fails.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PASS=0
FAIL=0
RESULTS=()

run_check() {
    local name="$1"
    shift
    echo "━━━ Running: $name ━━━"
    if "$@"; then
        RESULTS+=("✓ PASS: $name")
        ((PASS++))
    else
        RESULTS+=("✗ FAIL: $name")
        ((FAIL++))
    fi
    echo ""
}

# 1. cargo audit — check for RUSTSEC advisories
run_check "cargo audit" \
    cargo audit --file "$PROJECT_ROOT/src-tauri/Cargo.lock"

# 2. npm audit — check for high/critical npm advisories
run_check "npm audit" \
    npm audit --audit-level=high --prefix "$PROJECT_ROOT"

# 3. cargo clippy — deny all warnings
run_check "cargo clippy" \
    cargo clippy --manifest-path "$PROJECT_ROOT/src-tauri/Cargo.toml" -- -D warnings

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Security Audit Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo ""
echo "Passed: $PASS  Failed: $FAIL"

if [ "$FAIL" -gt 0 ]; then
    echo "SECURITY AUDIT FAILED"
    exit 1
fi

echo "ALL CHECKS PASSED"

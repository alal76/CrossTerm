#!/usr/bin/env bash
# SEC-T-10: Check for plaintext credential leaks after vault operations.
#
# Scans temp files and the app data directory for plaintext credential
# patterns that should never appear on disk unencrypted.
# Reports any matches as potential leaks.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Directories to scan
SCAN_DIRS=()

# Temp directories
if [ -d "/tmp" ]; then
    SCAN_DIRS+=("/tmp")
fi
if [ -n "${TMPDIR:-}" ] && [ -d "$TMPDIR" ]; then
    SCAN_DIRS+=("$TMPDIR")
fi

# App data directories (platform-specific)
case "$(uname -s)" in
    Darwin)
        APP_DATA="$HOME/Library/Application Support/com.crossterm"
        ;;
    Linux)
        APP_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/com.crossterm"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        APP_DATA="${APPDATA:-$HOME/AppData/Roaming}/com.crossterm"
        ;;
esac

if [ -n "${APP_DATA:-}" ] && [ -d "$APP_DATA" ]; then
    SCAN_DIRS+=("$APP_DATA")
fi

# Patterns that indicate plaintext credential leaks.
# These are test/sentinel values that should never appear unencrypted.
PATTERNS=(
    "test_master_password"
    "test_vault_password"
    "SuperSecret123"
    "my_api_token_value"
    "BEGIN RSA PRIVATE KEY"
    "BEGIN OPENSSH PRIVATE KEY"
    "BEGIN EC PRIVATE KEY"
    "BEGIN DSA PRIVATE KEY"
    "AKIA[0-9A-Z]{16}"         # AWS access key pattern
    "password.*=.*['\"][^'\"]{8,}" # password assignments in config
)

FOUND=0

echo "Scanning for plaintext credential leaks..."
echo "Directories: ${SCAN_DIRS[*]:-none}"
echo ""

for dir in "${SCAN_DIRS[@]}"; do
    if [ ! -d "$dir" ]; then
        continue
    fi
    for pattern in "${PATTERNS[@]}"; do
        # Use grep with timeout and limit depth to avoid scanning the entire filesystem
        matches=$(find "$dir" -maxdepth 3 -type f -size -10M \
            -not -path "*/node_modules/*" \
            -not -path "*/.git/*" \
            -not -name "*.log" \
            -print0 2>/dev/null | \
            xargs -0 grep -l -E "$pattern" 2>/dev/null || true)

        if [ -n "$matches" ]; then
            while IFS= read -r file; do
                echo "⚠ POTENTIAL LEAK: pattern '$pattern' found in: $file"
                ((FOUND++))
            done <<< "$matches"
        fi
    done
done

echo ""
if [ "$FOUND" -gt 0 ]; then
    echo "FAIL: Found $FOUND potential plaintext leak(s)"
    exit 1
fi

echo "PASS: No plaintext credential leaks detected"

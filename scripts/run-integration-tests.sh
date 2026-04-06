#!/usr/bin/env bash
# ── CrossTerm Integration Test Runner ─────────────────────────────────
# Starts Docker containers, generates test SSH keys, runs integration
# tests, and tears everything down on exit.
#
# Usage: ./scripts/run-integration-tests.sh [-- extra cargo test args]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_DIR="$REPO_ROOT/tests"
COMPOSE_FILE="$COMPOSE_DIR/docker-compose.yml"
SSH_KEY_DIR="$COMPOSE_DIR/fixtures/ssh-keys"
SSH_KEY_PATH="$SSH_KEY_DIR/test_ed25519"

# ── Cleanup on exit ──────────────────────────────────────────────────
cleanup() {
    echo ""
    echo "==> Tearing down containers..."
    docker compose -f "$COMPOSE_FILE" down --volumes --remove-orphans 2>/dev/null || true
    echo "==> Done."
}
trap cleanup EXIT INT TERM

# ── Generate test SSH keys if missing ────────────────────────────────
generate_keys() {
    if [[ ! -f "$SSH_KEY_PATH" ]]; then
        echo "==> Generating test Ed25519 keypair..."
        mkdir -p "$SSH_KEY_DIR"
        ssh-keygen -t ed25519 -f "$SSH_KEY_PATH" -N "" -C "crossterm-test" -q
        echo "    Created $SSH_KEY_PATH"
    else
        echo "==> Test SSH key already exists."
    fi
}

# ── Wait for a TCP port to become available ──────────────────────────
wait_for_port() {
    local host="$1" port="$2" name="$3" retries="${4:-30}"
    echo -n "    Waiting for $name ($host:$port)..."
    for ((i = 1; i <= retries; i++)); do
        if nc -z "$host" "$port" 2>/dev/null; then
            echo " ready (${i}s)"
            return 0
        fi
        sleep 1
    done
    echo " TIMEOUT after ${retries}s"
    return 1
}

# ── Main ─────────────────────────────────────────────────────────────
main() {
    echo "╔══════════════════════════════════════════════╗"
    echo "║   CrossTerm Integration Tests                ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""

    # 1. Generate keys
    generate_keys

    # 2. Start containers
    echo "==> Starting Docker containers..."
    docker compose -f "$COMPOSE_FILE" up -d --wait --build
    echo ""

    # 3. Wait for ports
    echo "==> Checking service readiness..."
    wait_for_port 127.0.0.1 2222 "openssh-server"
    wait_for_port 127.0.0.1 2223 "openssh-jump"
    echo ""

    # 4. Run integration tests
    echo "==> Running integration tests..."
    echo ""

    local extra_args=()
    if [[ "${1:-}" == "--" ]]; then
        shift
        extra_args=("$@")
    fi

    cd "$REPO_ROOT/src-tauri"
    cargo test --features integration -- --ignored --test-threads=1 "${extra_args[@]}" 2>&1
    local exit_code=$?

    echo ""
    if [[ $exit_code -eq 0 ]]; then
        echo "✅ All integration tests passed."
    else
        echo "❌ Some integration tests failed (exit code $exit_code)."
    fi

    return $exit_code
}

main "$@"

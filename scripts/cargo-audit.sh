#!/usr/bin/env bash
# CrossTerm — cargo audit wrapper with documented exceptions
# Usage: ./scripts/cargo-audit.sh
set -euo pipefail

cd "$(dirname "$0")/../src-tauri"

echo "Running cargo audit..."
cargo audit \
  --ignore RUSTSEC-2023-0071 \
  "$@"

# Ignored advisories:
#
# RUSTSEC-2023-0071 (rsa 0.9.10) — Marvin Attack: PKCS#1 v1.5 decryption timing side-channel
#   Impact: NONE. CrossTerm uses RSA for SSH key *signing* only, never PKCS#1 v1.5
#   decryption. No decryption oracle is exposed. Transitive dep: russh → ssh-key → rsa.
#   No fixed upstream version. Will resolve when russh upgrades past rsa 0.9.
#
# The 19 "unmaintained" warnings (GTK3 bindings, fxhash, proc-macro-error) are
# transitive Tauri framework dependencies we cannot control. Tauri tracks GTK4 migration.

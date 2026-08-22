#!/usr/bin/env bash
# CrossTerm — cargo audit wrapper with documented exceptions
# Usage: ./scripts/cargo-audit.sh
set -euo pipefail

cd "$(dirname "$0")/../src-tauri"

echo "Running cargo audit..."
cargo audit \
  --ignore RUSTSEC-2023-0071 \
  --ignore RUSTSEC-2026-0154 \
  --ignore RUSTSEC-2026-0153 \
  --ignore RUSTSEC-2026-0104 \
  --ignore RUSTSEC-2026-0099 \
  --ignore RUSTSEC-2026-0098 \
  --ignore RUSTSEC-2026-0049 \
  "$@"

# Ignored advisories — kept in sync with .github/workflows/ci.yml's own
# "Audit Rust dependencies" step; update both together.
#
# RUSTSEC-2023-0071 (rsa 0.9.10) — Marvin Attack: PKCS#1 v1.5 decryption timing side-channel
#   Impact: NONE. CrossTerm uses RSA for SSH key *signing* only, never PKCS#1 v1.5
#   decryption. No decryption oracle is exposed. Transitive dep: russh → ssh-key → rsa.
#   No fixed upstream version. Will resolve when russh upgrades past rsa 0.9.
#
# RUSTSEC-2026-0154 / -0153 (russh / russh-cryptovec) — DoS-style unbounded-allocation
#   bugs. Fix requires russh 0.45 -> 0.60+, a breaking upgrade across ssh/, netconf/,
#   x11_forward/ — tracked as follow-up work, not done as part of this interim ignore.
#
# RUSTSEC-2026-0104 / -0099 / -0098 / -0049 (rustls-webpki 0.102.8) — pinned by
#   rumqttc's own Cargo.toml (not our direct dep) to an exact 0.102.8 requirement
#   incompatible with the 0.103.x used by the rest of the app. All 4 are
#   cert-chain/CRL parsing edge cases in code paths this app doesn't reach —
#   CrossTerm's MQTT client uses a custom accept-all verifier, not webpki's chain
#   validation. Revisit if/when rumqttc relaxes that pin upstream.
#
# The "unmaintained" warnings (GTK3 bindings, unic-*, proc-macro-error, paste,
# instant, rustls-pemfile) are transitive Tauri/ironrdp/kube framework
# dependencies we don't control directly.

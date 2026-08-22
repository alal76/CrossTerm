#!/usr/bin/env bash
# CrossTerm build script — macOS and Linux.
#
# Builds a native desktop bundle for whichever OS this script runs on
# (Tauri bundles OS-specific WebView/signing/installer tooling, so a
# Windows .msi/.exe or a real macOS .app/.dmg can't be produced from a
# Linux host, or vice versa — see .github/workflows/release.yml's build
# matrix, which builds on ubuntu-latest/macos-latest/windows-latest for
# exactly this reason). Android is the one exception: it cross-compiles
# from any host with the Android SDK/NDK installed, so this same script
# can also produce an Android APK/AAB regardless of host OS.
#
# Usage:
#   ./build-scripts/build.sh                 # desktop bundle for this OS
#   ./build-scripts/build.sh android         # Android APK
#   ./build-scripts/build.sh android --aab   # Android App Bundle (Play Store)
#
# On Windows, use build-scripts/build.ps1 instead.

set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="${1:-desktop}"

command -v npm >/dev/null || { echo "npm not found — install Node.js first" >&2; exit 1; }
command -v cargo >/dev/null || { echo "cargo not found — install Rust first (https://rustup.rs)" >&2; exit 1; }

case "$(uname -s)" in
  Darwin) HOST_OS="macos" ;;
  Linux)  HOST_OS="linux" ;;
  *)
    echo "Unsupported host OS for this script: $(uname -s)." >&2
    echo "Use build-scripts/build.ps1 on Windows." >&2
    exit 1
    ;;
esac

echo "==> Installing npm dependencies..."
npm ci

case "$TARGET" in
  desktop)
    if [ "$HOST_OS" = "linux" ]; then
      # Same system packages the Linux leg of .github/workflows/ci.yml and
      # release.yml install before building — flagged here, not installed
      # automatically, since that needs sudo and varies by distro/package manager.
      for pkg in libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf; do
        if command -v dpkg >/dev/null 2>&1 && ! dpkg -s "$pkg" >/dev/null 2>&1; then
          echo "WARN: $pkg not detected — 'sudo apt-get install $pkg' if the build fails below." >&2
        fi
      done
    fi
    echo "==> Building CrossTerm desktop bundle for $HOST_OS..."
    npx tauri build
    echo "==> Done. Bundles: src-tauri/target/release/bundle/"
    ;;

  android)
    shift || true
    command -v java >/dev/null || { echo "java not found — Android builds need a JDK" >&2; exit 1; }
    [ -n "${ANDROID_HOME:-}" ] || echo "WARN: \$ANDROID_HOME not set — the Android SDK may not be found." >&2
    [ -n "${NDK_HOME:-}" ] || echo "WARN: \$NDK_HOME not set — the Android NDK may not be found." >&2

    if [ ! -d src-tauri/gen/android ]; then
      echo "==> Initializing Android project (first run only)..."
      npx tauri android init
    fi

    if [ "${1:-}" = "--aab" ]; then
      echo "==> Building CrossTerm Android App Bundle (.aab)..."
      npx tauri android build --aab
      echo "==> Done. Output: src-tauri/gen/android/app/build/outputs/bundle/"
    else
      echo "==> Building CrossTerm Android APK..."
      npx tauri android build --apk
      echo "==> Done. Output: src-tauri/gen/android/app/build/outputs/apk/"
      echo "==> Check size with: ./build-scripts/apk-size-check.sh"
    fi
    ;;

  *)
    echo "Unknown target: $TARGET (expected 'desktop' or 'android')" >&2
    exit 1
    ;;
esac

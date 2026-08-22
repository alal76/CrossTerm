#!/bin/bash
# Usage: ./build-scripts/apk-size-check.sh [path-to-apk]
# Defaults to the release APK's standard output path; pass a path explicitly
# for a debug build (e.g. .../apk/universal/debug/app-universal-debug.apk).
set -euo pipefail
APK="${1:-src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk}"
if [ -f "$APK" ]; then
  SIZE=$(stat -f%z "$APK" 2>/dev/null || stat --printf="%s" "$APK")
  SIZE_MB=$((SIZE / 1048576))
  echo "APK size: ${SIZE_MB}MB"
  [ "$SIZE_MB" -lt 50 ] && echo "PASS: Under 50MB target" || echo "WARN: Over 50MB target"
else
  echo "APK not found at $APK"
  exit 1
fi

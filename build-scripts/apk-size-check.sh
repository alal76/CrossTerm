#!/bin/bash
set -euo pipefail
APK="src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"
if [ -f "$APK" ]; then
  SIZE=$(stat -f%z "$APK" 2>/dev/null || stat --printf="%s" "$APK")
  SIZE_MB=$((SIZE / 1048576))
  echo "APK size: ${SIZE_MB}MB"
  [ "$SIZE_MB" -lt 50 ] && echo "PASS: Under 50MB target" || echo "WARN: Over 50MB target"
else
  echo "APK not found at $APK"
  exit 1
fi

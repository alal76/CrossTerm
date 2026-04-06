#!/bin/bash
# Android build pipeline for CrossTerm
set -euo pipefail
# Check prerequisites
command -v cargo >/dev/null || { echo "cargo not found"; exit 1; }
command -v npx >/dev/null || { echo "npx not found"; exit 1; }
# Build APK
echo "Building CrossTerm Android APK..."
cd "$(dirname "$0")/.."
npm ci
npx tauri android build --apk
echo "APK built successfully"
echo "Output: src-tauri/gen/android/app/build/outputs/apk/"

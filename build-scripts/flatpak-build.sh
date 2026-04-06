#!/bin/bash
set -euo pipefail
echo "Building CrossTerm Flatpak..."
# Requires flatpak-builder
flatpak-builder --force-clean build-dir build-scripts/com.crossterm.CrossTerm.yml
echo "Flatpak built successfully"

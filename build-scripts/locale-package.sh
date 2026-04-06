#!/bin/bash
# Package locale files for distribution
set -euo pipefail
LOCALE_DIR="src/i18n"
OUT_DIR="dist/locales"
mkdir -p "$OUT_DIR"
for f in "$LOCALE_DIR"/*.json; do
  locale=$(basename "$f" .json)
  echo "Packaging locale: $locale"
  cp "$f" "$OUT_DIR/$locale.json"
done
echo "Locale packages created in $OUT_DIR"

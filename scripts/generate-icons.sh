#!/usr/bin/env bash
# generate-icons.sh — Generate all Tauri icon sizes from icons/icon.svg
# Requires: ImageMagick (convert / magick) and optionally icotool (icoutils)
#
# Usage: ./scripts/generate-icons.sh
#
# Output goes to src-tauri/icons/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_SVG="$PROJECT_ROOT/icons/icon.svg"
OUT_DIR="$PROJECT_ROOT/src-tauri/icons"

if [ ! -f "$SOURCE_SVG" ]; then
  echo "Error: Source SVG not found at $SOURCE_SVG"
  exit 1
fi

mkdir -p "$OUT_DIR"

# Detect ImageMagick command (v7: magick, v6: convert)
if command -v magick &>/dev/null; then
  CONVERT="magick"
elif command -v convert &>/dev/null; then
  CONVERT="convert"
else
  echo "Error: ImageMagick is required. Install with: brew install imagemagick"
  exit 1
fi

echo "Using ImageMagick command: $CONVERT"
echo "Source: $SOURCE_SVG"
echo "Output: $OUT_DIR"
echo ""

# PNG sizes needed by Tauri
declare -A SIZES=(
  ["32x32.png"]=32
  ["128x128.png"]=128
  ["128x128@2x.png"]=256
  ["icon.png"]=1024
)

for filename in "${!SIZES[@]}"; do
  size="${SIZES[$filename]}"
  echo "  Generating ${filename} (${size}x${size})..."
  $CONVERT -background none -density 300 "$SOURCE_SVG" -resize "${size}x${size}" "$OUT_DIR/$filename"
done

# macOS .icns
echo "  Generating icon.icns..."
# Create a temporary iconset directory
ICONSET_DIR=$(mktemp -d)/CrossTerm.iconset
mkdir -p "$ICONSET_DIR"

ICNS_SIZES=(16 32 64 128 256 512 1024)
for s in "${ICNS_SIZES[@]}"; do
  $CONVERT -background none -density 300 "$SOURCE_SVG" -resize "${s}x${s}" "$ICONSET_DIR/icon_${s}x${s}.png"
done

# Create icon pairs for iconutil (name format: icon_NxN.png and icon_NxN@2x.png)
cp "$ICONSET_DIR/icon_32x32.png" "$ICONSET_DIR/icon_16x16@2x.png"
cp "$ICONSET_DIR/icon_64x64.png" "$ICONSET_DIR/icon_32x32@2x.png"
cp "$ICONSET_DIR/icon_256x256.png" "$ICONSET_DIR/icon_128x128@2x.png"
cp "$ICONSET_DIR/icon_512x512.png" "$ICONSET_DIR/icon_256x256@2x.png"
cp "$ICONSET_DIR/icon_1024x1024.png" "$ICONSET_DIR/icon_512x512@2x.png"
rm -f "$ICONSET_DIR/icon_64x64.png" "$ICONSET_DIR/icon_1024x1024.png"

if command -v iconutil &>/dev/null; then
  iconutil -c icns "$ICONSET_DIR" -o "$OUT_DIR/icon.icns"
else
  echo "  Warning: iconutil not found (macOS only). Skipping .icns generation."
fi
rm -rf "$(dirname "$ICONSET_DIR")"

# Windows .ico (multi-size)
echo "  Generating icon.ico..."
$CONVERT -background none -density 300 "$SOURCE_SVG" \
  \( -clone 0 -resize 16x16 \) \
  \( -clone 0 -resize 32x32 \) \
  \( -clone 0 -resize 48x48 \) \
  \( -clone 0 -resize 64x64 \) \
  \( -clone 0 -resize 128x128 \) \
  \( -clone 0 -resize 256x256 \) \
  -delete 0 "$OUT_DIR/icon.ico"

# Windows Store logos (Square sizes)
STORE_SIZES=(
  "Square30x30Logo.png:30"
  "Square44x44Logo.png:44"
  "Square71x71Logo.png:71"
  "Square89x89Logo.png:89"
  "Square107x107Logo.png:107"
  "Square142x142Logo.png:142"
  "Square150x150Logo.png:150"
  "Square284x284Logo.png:284"
  "Square310x310Logo.png:310"
  "StoreLogo.png:50"
)

for entry in "${STORE_SIZES[@]}"; do
  filename="${entry%%:*}"
  size="${entry##*:}"
  echo "  Generating ${filename} (${size}x${size})..."
  $CONVERT -background none -density 300 "$SOURCE_SVG" -resize "${size}x${size}" "$OUT_DIR/$filename"
done

echo ""
echo "Done! All icons generated in $OUT_DIR"

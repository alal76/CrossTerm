#!/usr/bin/env bash
# build-helpbook.sh — Build macOS Help Book bundle from docs/help/*.md
#
# Generates a CrossTerm.help bundle under src-tauri/resources/ with HTML
# converted from markdown, an index page, and (on macOS) a .helpindex for
# Spotlight integration via Apple Help Book framework.
#
# Usage:
#   ./scripts/build-helpbook.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

HELP_SRC="$PROJECT_ROOT/docs/help"
BUNDLE_DIR="$PROJECT_ROOT/src-tauri/resources/CrossTerm.help"
CONTENTS="$BUNDLE_DIR/Contents"
RESOURCES="$CONTENTS/Resources"
EN_LPROJ="$RESOURCES/en.lproj"

# Book metadata
BOOK_TITLE="CrossTerm Help"
BOOK_ID="com.crossterm.app.help"

# ── Clean previous build ────────────────────────────────────────────────
rm -rf "$BUNDLE_DIR"
mkdir -p "$EN_LPROJ"

# ── Generate Help Book Info.plist ────────────────────────────────────────
cat > "$CONTENTS/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleIdentifier</key>
    <string>${BOOK_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${BOOK_TITLE}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleSignature</key>
    <string>hbwr</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>HPDBookAccessPath</key>
    <string>index.html</string>
    <key>HPDBookTitle</key>
    <string>${BOOK_TITLE}</string>
    <key>HPDBookType</key>
    <string>3</string>
</dict>
</plist>
PLIST

# ── Markdown → HTML conversion ──────────────────────────────────────────
convert_md() {
    local md_file="$1"
    local out_dir="$2"
    local basename
    basename="$(basename "$md_file" .md)"
    local html_file="$out_dir/${basename}.html"
    local title

    # Extract title from YAML frontmatter or first H1
    title=$(grep '^title:' "$md_file" | head -1 | sed 's/^title:[[:space:]]*"*//;s/"*$//')
    if [ -z "$title" ]; then
        title=$(head -20 "$md_file" | grep '^# ' | head -1 | sed 's/^# //')
    fi
    if [ -z "$title" ]; then
        title="$basename"
    fi

    if command -v pandoc &>/dev/null; then
        pandoc -f markdown -t html5 --standalone \
            --metadata "title=$title" \
            -o "$html_file" "$md_file"
    else
        # Fallback: wrap content in basic HTML structure
        {
            echo '<!DOCTYPE html>'
            echo '<html lang="en"><head><meta charset="utf-8">'
            printf '<title>%s</title>\n' "$title"
            echo '<style>body{font-family:-apple-system,BlinkMacSystemFont,sans-serif;padding:20px;max-width:800px;margin:0 auto;line-height:1.6;}code{background:#f4f4f4;padding:2px 6px;border-radius:3px;}pre{background:#f4f4f4;padding:12px;border-radius:6px;overflow-x:auto;}</style>'
            echo '</head><body>'
            sed -e 's/^### \(.*\)/<h3>\1<\/h3>/' \
                -e 's/^## \(.*\)/<h2>\1<\/h2>/' \
                -e 's/^# \(.*\)/<h1>\1<\/h1>/' \
                -e 's/\*\*\([^*]*\)\*\*/<strong>\1<\/strong>/g' \
                -e 's/`\([^`]*\)`/<code>\1<\/code>/g' \
                -e 's/^- \(.*\)/<li>\1<\/li>/' \
                -e '/^$/s/^$/<br\/>/' \
                "$md_file"
            echo '</body></html>'
        } > "$html_file"
    fi
}

# ── Process all markdown files ───────────────────────────────────────────
echo "Building CrossTerm Help Book..."
for md in "$HELP_SRC"/*.md; do
    [ -f "$md" ] || continue
    echo "  Converting: $(basename "$md")"
    convert_md "$md" "$EN_LPROJ"
done

# ── Create index.html landing page ──────────────────────────────────────
cat > "$EN_LPROJ/index.html" << 'INDEXHEAD'
<!DOCTYPE html>
<html lang="en"><head>
    <meta charset="utf-8">
    <title>CrossTerm Help</title>
    <meta name="robots" content="anchors">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; max-width: 800px; margin: 0 auto; }
        h1 { font-size: 24px; }
        ul { list-style: none; padding: 0; }
        li { margin: 8px 0; }
        a { color: #0070c9; text-decoration: none; }
        a:hover { text-decoration: underline; }
    </style>
</head><body>
    <h1>CrossTerm Help</h1>
    <ul>
INDEXHEAD

for md in "$HELP_SRC"/*.md; do
    [ -f "$md" ] || continue
    basename_no_ext="$(basename "$md" .md)"
    title=$(grep '^title:' "$md" | head -1 | sed 's/^title:[[:space:]]*"*//;s/"*$//')
    if [ -z "$title" ]; then
        title=$(head -20 "$md" | grep '^# ' | head -1 | sed 's/^# //')
    fi
    [ -z "$title" ] && title="$basename_no_ext"
    printf '        <li><a href="%s.html">%s</a></li>\n' "$basename_no_ext" "$title" >> "$EN_LPROJ/index.html"
done

cat >> "$EN_LPROJ/index.html" << 'INDEXFOOT'
    </ul>
</body>
</html>
INDEXFOOT

# ── Generate .helpindex (macOS only) ────────────────────────────────────
if command -v hiutil &>/dev/null; then
    echo "Generating help index with hiutil..."
    hiutil -Caf "$EN_LPROJ/CrossTerm.helpindex" "$EN_LPROJ"
    echo "Help index generated."
else
    echo "hiutil not found (non-macOS platform) — skipping help index generation."
fi

echo "Help Book built at: $BUNDLE_DIR"

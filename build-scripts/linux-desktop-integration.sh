#!/bin/bash
set -euo pipefail
# Install Linux file manager context menu integration
DESKTOP_FILE="$HOME/.local/share/applications/crossterm.desktop"
cat > "$DESKTOP_FILE" << 'EOF'
[Desktop Entry]
Type=Application
Name=CrossTerm
Comment=Cross-platform terminal emulator
Exec=crossterm --working-dir %f
Icon=crossterm
Terminal=false
Categories=System;TerminalEmulator;
Actions=new-window;
[Desktop Action new-window]
Name=New Window
Exec=crossterm
EOF

# Nautilus script
NAUTILUS_DIR="$HOME/.local/share/nautilus/scripts"
mkdir -p "$NAUTILUS_DIR"
cat > "$NAUTILUS_DIR/Open in CrossTerm" << 'EOF'
#!/bin/bash
crossterm --working-dir "$NAUTILUS_SCRIPT_CURRENT_URI"
EOF
chmod +x "$NAUTILUS_DIR/Open in CrossTerm"
echo "Desktop integration installed"

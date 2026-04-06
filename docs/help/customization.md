---
title: "Customization"
slug: "customization"
category: "reference"
order: 7
schema_version: 1
keywords: ["theme", "custom", "keybinding", "font", "color", "appearance", "terminal", "style", "import", "shortcut"]
---

# Customization

CrossTerm is highly customizable. You can change themes, fonts, keybindings, and terminal appearance to match your workflow.

## Theming

### Built-in Themes

CrossTerm ships with eight built-in themes:

- **Dark** — Default dark theme with blue accents
- **Light** — Clean light theme for bright environments
- **Solarized Dark** — Ethan Schoonover's warm dark palette
- **Solarized Light** — Ethan Schoonover's warm light palette
- **Dracula** — Popular purple-accented dark theme
- **Nord** — Arctic, north-bluish color palette
- **Monokai Pro** — Rich, vibrant dark theme
- **High Contrast** — Maximum readability for accessibility

Switch themes from:
1. **Title Bar** — Click the theme toggle button (sun/moon icon) to cycle through Dark → Light → System.
2. **Settings** — Open Settings (**Ctrl+,** / **⌘,**) → Appearance → Theme.
3. **Command Palette** — Press **Ctrl+Shift+P** / **⌘⇧P** and type "Switch to Light/Dark Theme".

### System Theme

Select **System** to follow your operating system's light/dark mode preference. CrossTerm listens for `prefers-color-scheme` changes and updates automatically.

### Importing Custom Themes

You can import custom themes from JSON files:

1. Open **Settings** → **Appearance** → **Import Theme**.
2. Select a `.json` theme file from your filesystem.
3. The theme is loaded and applied immediately.

Custom theme files must follow the CrossTerm design token schema. See `src/themes/tokens.json` for the full token reference. A valid theme file includes:

```json
{
  "name": "My Theme",
  "variant": "dark",
  "tokens": {
    "surface-primary": "#1a1b26",
    "surface-secondary": "#24283b",
    "text-primary": "#c0caf5",
    "accent-primary": "#7aa2f7"
  }
}
```

### Design Token System

CrossTerm uses a three-layer token indirection system:

1. **CSS Custom Properties** — Raw values defined at the `:root` level.
2. **Tailwind Config** — Maps tokens to Tailwind utility classes.
3. **Utility Classes** — Used in components (e.g., `bg-surface-primary`, `text-accent-primary`).

This ensures theme changes propagate consistently across the entire UI.

## Terminal Appearance

### Font Settings

Configure terminal fonts from **Settings** → **Terminal**:

- **Font Family** — Default: `JetBrains Mono`. Any monospace font installed on your system works.
- **Font Size** — Default: 14px. Range: 8–32px.
- **Line Height** — Default: 1.2. Adjust for tighter or looser line spacing.
- **Letter Spacing** — Default: 0. Fine-tune character spacing.
- **Font Ligatures** — Enable/disable programming ligatures (e.g., `=>`, `!=`).

### Cursor

- **Cursor Style** — Block, Underline, or Bar.
- **Cursor Blink** — Enable or disable cursor blinking.

### Opacity

Adjust terminal background transparency from **Settings** → **Terminal** → **Opacity**. Values range from 0 (fully transparent) to 1 (fully opaque).

### Bell Mode

Choose how the terminal bell character is rendered:

- **None** — Silent.
- **Visual** — Brief flash on the terminal pane.
- **Audio** — System beep sound.

## Keybindings

### Default Shortcuts

CrossTerm provides a comprehensive set of keyboard shortcuts. View them all by pressing **Ctrl+/** / **⌘/**, or open the Command Palette and search "Keyboard Shortcuts".

### Custom Keybindings

To customize a keyboard shortcut:

1. Open the **Keyboard Shortcuts** overlay (**Ctrl+/** / **⌘/**).
2. Shortcuts marked with **(custom)** have been modified from defaults.
3. Custom keybindings are stored per-profile and sync across sessions.

### Exporting Shortcuts

Click the **Print / Export** button in the Keyboard Shortcuts overlay to download a text file with all your current shortcuts for reference or sharing.

## Tab Title Format

Customize how tab titles are displayed from **Settings** → **Terminal** → **Tab Title Format**. Use template variables:

- `{name}` — Session name
- `{host}` — Remote hostname
- `{user}` — Username
- `{shell}` — Shell name

Example: `{user}@{host}` displays as `root@server1`.

## Scrollback Buffer

Configure the number of lines kept in the terminal scrollback buffer from **Settings** → **Terminal** → **Scrollback Lines**. Default is 10,000 lines. Higher values use more memory.

## GPU Acceleration

Enable or disable hardware-accelerated rendering from **Settings** → **Terminal** → **GPU Acceleration**. Enabled by default for smooth performance. Disable if you experience rendering issues on older hardware.

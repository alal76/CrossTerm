---
title: "Settings Reference"
slug: "settings"
category: "reference"
order: 7
schema_version: 1
keywords: ["settings", "preferences", "configure", "theme", "font", "ssh", "keyboard", "notifications", "security", "advanced", "appearance"]
---

# Settings Reference

Open Settings with **⌘,** (macOS) or **Ctrl+,** (Windows/Linux), or via **Settings → Preferences…** in the menu bar.

The Settings panel is divided into ten tabs. All changes take effect immediately and are persisted to your active profile.

---

## General

| Setting | Description | Default |
|---------|-------------|---------|
| Default Profile | Which profile to activate on launch | Default |
| Startup Behavior | What to do when the app starts: `Restore last session`, `Open new tab`, or `Show session browser` | Restore last session |
| Confirm Close Tab | Ask for confirmation before closing a tab | Off |
| Show Status Bar | Display the status bar at the bottom of the window | On |
| Window Always on Top | Keep the CrossTerm window above all other windows | Off |
| Auto Update | Automatically download and install updates in the background | On |
| GPU Acceleration | Use WebGL hardware acceleration for the terminal renderer (requires restart) | On |

### Profile Sync

Export all settings, sessions, themes, and snippets to a `.ctbundle` archive, or import a bundle to restore a configuration. Useful for moving your setup between machines.

- **Export** — saves a bundle to a location you choose.
- **Import** — restores from a bundle file; existing settings are merged.

---

## Appearance

### Color Theme

| Built-in Themes | |
|---|---|
| Dark | Deep dark background, balanced contrast |
| Light | Clean white background |
| System | Follows macOS/Windows dark mode setting |
| Dracula | Purple-based dark theme |
| Nord | Arctic, north-bluish palette |
| Monokai Pro | Rich colours on dark background |
| Solarized Dark | Ethan Schoonover's solarized, dark variant |
| Solarized Light | Ethan Schoonover's solarized, light variant |
| One Dark | Atom One Dark inspired |

You can also create a custom theme by editing the CSS token values directly.

### Font

| Setting | Description | Default |
|---------|-------------|---------|
| Font Family | Terminal font. Choices include JetBrains Mono, Fira Code, Cascadia Code, Hack, Source Code Pro, Menlo, Consolas, and System Mono | JetBrains Mono |
| Font Size | Point size, 8–32 | 14 |
| Line Height | Vertical line spacing multiplier, 1.0–2.0 | 1.2 |
| Letter Spacing | Extra horizontal space between characters, −2–10 px | 0 |
| Font Ligatures | Enable programming ligatures (e.g. `=>`, `!=`) for supported fonts | On |

### Terminal Display

| Setting | Description | Default |
|---------|-------------|---------|
| Cursor Style | `block`, `underline`, or `bar` | block |
| Cursor Blink | Animate the cursor | On |
| Terminal Opacity | Background transparency, 0.1–1.0 | 1.0 |
| Scrollbar Visible | Show the scrollbar in the terminal | On |

---

## Terminal

| Setting | Description | Default |
|---------|-------------|---------|
| Scrollback Lines | Number of lines to keep in the scrollback buffer | 10 000 |
| Scroll on Output | Jump to the bottom when new output arrives | On |
| Scroll on Keystroke | Jump to the bottom when you type | On |
| Bell Style | How to handle the terminal bell: `None`, `Visual` (screen flash), or `Sound` | Visual |
| Terminal Encoding | Character encoding for the terminal: `UTF-8`, `ISO-8859-1`, `GBK`, or `Shift-JIS` | UTF-8 |
| Word Separators | Characters that delimit words for double-click selection | ` ()[]{}'"` |
| Default Shell | Override the system default shell for new local tabs. Leave blank to use `$SHELL` | (system default) |
| Tab Title Format | Template for tab titles. Tokens: `{host}`, `{user}`, `{cwd}`, `{cmd}` | `{host}` |

---

## SSH

| Setting | Description | Default |
|---------|-------------|---------|
| Default SSH Port | Port used when no port is specified in Quick Connect or session settings | 22 |
| Keepalive Interval | Seconds between SSH keepalive packets. `0` disables keepalives | 60 |
| Strict Host Checking | Reject connections if the host key has changed | On |
| SSH Compression | Enable zlib compression for SSH data streams | Off |
| Agent Forwarding | Forward your local SSH agent to remote sessions by default | Off |
| X11 Forwarding | Forward X11 display connections through the SSH tunnel by default | Off |

!!! note
    Agent Forwarding and X11 Forwarding can also be toggled per-session in the session editor. Global settings here act as the default for new sessions.

---

## Connections

| Setting | Description | Default |
|---------|-------------|---------|
| Connection Timeout | Seconds to wait before giving up on a new connection | 30 |
| Reconnect on Disconnect | Automatically attempt to reconnect if a session drops | On |
| Reconnect Delay | Seconds to wait before the first reconnect attempt | 5 |
| Max Reconnect Attempts | Stop retrying after this many consecutive failures. `0` = unlimited | 5 |
| Max Concurrent Connections | Limit the total number of simultaneously active sessions | 20 |

---

## File Transfer

| Setting | Description | Default |
|---------|-------------|---------|
| Default Remote Directory | Starting directory when opening the SFTP browser | `/home/{user}` |
| SFTP Encoding | Character encoding for remote file names: `UTF-8`, `ISO-8859-1`, `GBK` | UTF-8 |
| Confirm Overwrite | Ask before overwriting existing files during transfers | On |
| Preserve Timestamps | Keep the source file's modification time on the destination | On |
| Concurrent Transfer Jobs | Number of files to upload or download simultaneously | 4 |

---

## Keyboard

| Setting | Description | Default |
|---------|-------------|---------|
| Backspace Sends Delete | Send `DEL` (0x7F) instead of `BS` (0x08) when Backspace is pressed | Off |
| Alt is Meta | Treat the Alt/Option key as the terminal Meta key (sends ESC prefix) | On |
| Ctrl+H is Backspace | Interpret Ctrl+H as Backspace (legacy terminal compatibility) | Off |
| Home/End Scroll Buffer | Home and End keys scroll the terminal buffer instead of sending cursor keys | Off |
| Right-Click Action | What happens when you right-click the terminal: `Context Menu`, `Paste`, or `Select Word` | Context Menu |

---

## Notifications

| Setting | Description | Default |
|---------|-------------|---------|
| Notify on Disconnect | Show a system notification when a session disconnects unexpectedly | On |
| Notify on Bell | Trigger a system notification when the terminal bell fires (useful for long-running commands) | Off |
| Notify on Long Process | Notify when a command has been running longer than the threshold | On |
| Long Process Threshold | Seconds a command must run before a "long process" notification fires | 30 |
| Flash Tab on Bell | Highlight the tab title when the bell fires in a background tab | On |

---

## Security

### Clipboard

| Setting | Description | Default |
|---------|-------------|---------|
| Copy on Select | Automatically copy selected text to the clipboard | Off |
| Paste Warning Threshold | Warn before pasting more than this many lines (prevents accidental multi-line pastes) | 5 |
| Clipboard History Size | Number of clipboard entries to retain in the clipboard history | 20 |

### Vault Lock

| Setting | Description | Default |
|---------|-------------|---------|
| Idle Lock Timeout | Automatically lock the credential vault after this many seconds of inactivity. `0` disables auto-lock | 0 (disabled) |

### Security Panel

The Security tab also embeds the **Security Settings** panel for managing vault passwords, viewing audit log entries, and enabling/disabling vault encryption.

---

## Advanced

| Setting | Description | Default |
|---------|-------------|---------|
| Log Level | Backend log verbosity: `Error`, `Warn`, `Info`, `Debug`, `Trace` | Warn |
| Telemetry | Send anonymous crash and usage statistics to help improve CrossTerm | Off |

### Debug Info

The bottom of the Advanced tab shows read-only diagnostic information:

| Field | Value |
|-------|-------|
| Version | Current CrossTerm version |
| Platform | Detected operating system |
| Renderer | `GPU (WebGL)` or `CPU (Canvas)` depending on GPU Acceleration setting |

A button to **Export Audit Log** is also available here, which writes all audit events to a CSV file.

---

## Settings Persistence

Settings are stored per-profile in `~/Library/Application Support/com.crossterm.app/` (macOS), `%APPDATA%\com.crossterm.app\` (Windows), or `~/.config/com.crossterm.app/` (Linux).

To back up or migrate your settings, use **Profile Sync → Export** in the General tab.

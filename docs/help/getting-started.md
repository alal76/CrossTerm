---
title: "Getting Started"
slug: "getting-started"
category: "basics"
order: 1
schema_version: 1
keywords: ["start", "setup", "first", "connect", "session", "terminal", "new", "quick"]
---

# Getting Started with CrossTerm

Welcome to CrossTerm — a cross-platform terminal emulator and remote access suite. This guide will walk you through your first steps.

## Creating Your First Session

1. Press **Ctrl+T** (or **⌘T** on macOS) to open a new local shell tab.
2. A terminal session will open in the main canvas area.
3. You can type commands just like any other terminal.

## Connecting to a Remote Host

To connect to a remote server via SSH:

1. Press **Ctrl+Shift+N** (or **⌘⇧N** on macOS) to open Quick Connect.
2. Enter the hostname, port, username, and authentication details.
3. Click **Connect** or press Enter.
4. A new SSH tab will appear showing your remote session.

Alternatively, you can create a saved session:

1. Click the **+** button next to the tab bar.
2. Select **New SSH Session** from the dropdown.
3. Fill in the connection details.
4. The session is saved for quick access later.

## Navigating the Interface

CrossTerm uses a six-region layout:

- **Title Bar** (top): App branding, theme toggle, profile switcher.
- **Tab Bar**: Manage open sessions. Right-click tabs for context menu options.
- **Sidebar** (left): Browse saved sessions, snippets, and tunnel configurations.
- **Session Canvas** (center): Your active terminal sessions.
- **Bottom Panel**: SFTP browser, snippet manager, audit log, and search.
- **Status Bar** (bottom): Connection status, encoding, and terminal dimensions.

## Using the Command Palette

Press **Ctrl+Shift+P** (or **⌘⇧P** on macOS) to open the Command Palette. From here you can:

- Open new sessions
- Toggle panels
- Change settings
- Switch themes
- Lock the credential vault

## Managing Tabs

- **New tab**: Ctrl+T / ⌘T
- **Close tab**: Ctrl+W / ⌘W
- **Next tab**: Ctrl+Tab
- **Previous tab**: Ctrl+Shift+Tab
- **Go to tab 1–9**: Ctrl+1 through Ctrl+9

Right-click a tab to access options like duplicate, split, rename, and close others.

## Split Panes

You can split your terminal workspace:

1. Right-click any tab.
2. Select **Split Right** or **Split Down**.
3. Both panes remain active and can be used independently.

## Theme Selection

CrossTerm ships with several built-in themes including Dark, Light, Dracula, Nord, Monokai Pro, and Solarized variants. Change themes via:

- The sun/moon icon in the title bar (cycles Dark → Light → System)
- Settings panel (**Ctrl+,** / **⌘,**)

## Next Steps

- Learn about [SSH Connections](ssh-connections) for remote access.
- Set up the [Credential Vault](credential-vault) to store passwords and keys securely.
- Explore [Keyboard Shortcuts](keyboard-shortcuts) to work faster.

import type { HelpArticle } from "@/types";

// ── Raw help content bundled at build time ──
//
// Locale fallback pattern:
// Help articles can be loaded from locale-specific paths under docs/help/{locale}/
// (e.g. docs/help/en/, docs/help/de/). When a locale-specific article is not found,
// the system falls back to the default English content defined below.
// To add a new locale, create docs/help/{locale}/ with matching markdown files.

const rawArticles: Array<{
  slug: string;
  title: string;
  category: string;
  order: number;
  keywords: string[];
  body: string;
}> = [
  {
    slug: "getting-started",
    title: "Getting Started",
    category: "basics",
    order: 1,
    keywords: ["start", "setup", "first", "connect", "session", "terminal", "new", "quick"],
    body: `# Getting Started with CrossTerm

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

## Navigating the Interface

CrossTerm uses a six-region layout:

- **Title Bar** (top): App branding, theme toggle, profile switcher.
- **Tab Bar**: Manage open sessions with right-click context menus.
- **Sidebar** (left): Browse saved sessions, snippets, and tunnels.
- **Session Canvas** (center): Your active terminal sessions.
- **Bottom Panel**: SFTP browser, snippet manager, audit log, and search.
- **Status Bar** (bottom): Connection status, encoding, and dimensions.

## Using the Command Palette

Press **Ctrl+Shift+P** (or **⌘⇧P** on macOS) to open the Command Palette. From here you can open sessions, toggle panels, change settings, switch themes, and lock the vault.

## Managing Tabs

- **New tab**: Ctrl+T / ⌘T
- **Close tab**: Ctrl+W / ⌘W
- **Next tab**: Ctrl+Tab
- **Previous tab**: Ctrl+Shift+Tab

## Split Panes

Right-click any tab and select **Split Right** or **Split Down** to create a side-by-side terminal layout.`,
  },
  {
    slug: "ssh-connections",
    title: "SSH Connections",
    category: "connections",
    order: 2,
    keywords: ["ssh", "connect", "remote", "key", "password", "jump", "proxy", "port", "forward", "agent", "host"],
    body: `# SSH Connections

CrossTerm provides a full-featured SSH client with support for modern authentication methods, jump hosts, port forwarding, and agent forwarding.

## Authentication Methods

### Password Authentication

Enter your username and password when connecting. Passwords can be stored in the Credential Vault.

### SSH Key Authentication

More secure than passwords. CrossTerm supports OpenSSH, PEM, and PKCS#8 key formats.

**Supported key types:** RSA (2048-bit+), Ed25519 (recommended), ECDSA (P-256, P-384, P-521).

### Certificate Authentication

For organizations using SSH certificates. Supports PEM and PKCS#12 formats.

## Jump Hosts (ProxyJump)

Connect through an intermediate server when direct access is not possible. You can chain multiple jump hosts for complex network topologies.

## Port Forwarding

### Local Forwarding
Forward a port from your local machine to a remote destination through the SSH tunnel.

### Remote Forwarding
Expose a local service to the remote host.

### Dynamic Forwarding (SOCKS Proxy)
Create a SOCKS5 proxy through the SSH tunnel.

## Agent Forwarding

Allow the remote server to use your local SSH keys for onward connections. Only enable agent forwarding to trusted servers.

## Keep-Alive Settings

Sends a packet every N seconds (default: 60) to prevent idle disconnections. Configure per-session or globally in Settings → SSH.

## Host Key Verification

On first connection, CrossTerm displays the server's host key fingerprint. If a previously saved host key changes, CrossTerm shows a warning.`,
  },
  {
    slug: "sftp-file-transfer",
    title: "SFTP File Transfer",
    category: "connections",
    order: 3,
    keywords: ["sftp", "file", "transfer", "upload", "download", "browse", "drag", "drop", "queue", "remote"],
    body: `# SFTP File Transfer

CrossTerm includes a built-in SFTP browser for transferring files between your local machine and remote servers over SSH.

## Opening the SFTP Browser

- **New tab**: Click + → New SFTP Browser.
- **Bottom panel**: Open with Ctrl+J / ⌘J and select SFTP tab.
- **From SSH session**: Uses the same connection credentials.

## Browsing Remote Files

Dual-pane interface with local files on the left and remote files on the right. Navigate by double-clicking folders.

## Uploading Files

### Drag and Drop
Drag files from your desktop and drop them onto the remote pane.

### Upload Button
Click Upload Files in the toolbar, select files, and they upload to the current remote directory.

## Downloading Files

Select files in the remote pane and click Download, or drag them to the local pane.

## Transfer Queue

View active and pending transfers in the bottom panel. Each transfer shows progress, and you can pause, resume, or cancel individual transfers.

## File Operations

Right-click for rename, delete, change permissions, and create directory operations.

## Security

All SFTP transfers are encrypted through the SSH tunnel. File permissions and ownership are preserved when possible.`,
  },
  {
    slug: "credential-vault",
    title: "Credential Vault",
    category: "security",
    order: 4,
    keywords: ["vault", "credential", "password", "key", "security", "encrypt", "lock", "master", "store", "secret"],
    body: `# Credential Vault

The Credential Vault is CrossTerm's encrypted storage for passwords, SSH keys, API tokens, and other sensitive credentials. All data is encrypted with AES-256-GCM.

## Setting Up the Vault

On first launch, create a master password (minimum 8 characters). There is no password recovery mechanism.

## Unlocking the Vault

Click the lock icon and enter your master password. The vault unlocks and credentials become available.

## Storing Credentials

- **Passwords**: Username/password pairs for SSH and other services.
- **SSH Keys**: Private keys with optional passphrases.
- **API Tokens**: Tokens for cloud services and APIs.
- **Cloud Credentials**: AWS, Azure, or GCP access keys.

## Using Credentials

When creating a session, click the key icon to select a saved credential. Credentials are referenced by ID — updates apply to all sessions using them.

## Auto-Lock

The vault automatically locks after 15 minutes of inactivity (configurable in Settings → Security). Set to 0 to disable.

## Clipboard Security

Copied passwords are automatically cleared from the clipboard after 30 seconds (configurable in Settings → Security).

## Security Details

- **Encryption**: AES-256-GCM with PBKDF2 key derivation.
- **Key material**: Held in memory with zeroize-on-drop protection.
- **Audit logging**: All vault access events are recorded.`,
  },
  {
    slug: "troubleshooting",
    title: "Troubleshooting",
    category: "support",
    order: 5,
    keywords: ["error", "problem", "fix", "debug", "fail", "connect", "timeout", "refuse", "key", "auth", "firewall"],
    body: `# Troubleshooting

Common issues and their solutions.

## Authentication Failed

- Check your password (case-sensitive).
- Verify the username.
- Ensure your SSH public key is in the server's authorized_keys.
- Check key format compatibility (OpenSSH, PEM, PKCS#8).
- Provide passphrase if your key requires one.

## Connection Timed Out

- Verify hostname or IP address.
- Check the port number (SSH default: 22).
- Check firewall rules (local and remote).
- Verify network connectivity and DNS resolution.

## Connection Refused

- Ensure SSH daemon is running on the remote host.
- Verify the correct port.
- Check firewall and host-based access restrictions.

## Host Key Verification Failed

The server's host key changed. This could indicate server reinstallation or a security issue. Verify with your administrator before accepting the new key.

## Terminal Not Rendering

- Try resizing the terminal window.
- Check font settings for a valid monospace font.
- Toggle GPU Acceleration in Settings.
- Restart the terminal session.

## Copy/Paste Not Working

- Copy: Ctrl+Shift+C / ⌘C. Paste: Ctrl+Shift+V / ⌘V.
- Enable "Copy on Select" in Settings if preferred.
- Check paste confirmation settings in Settings → Security.

## Forgot Master Password

The master password cannot be recovered. Delete the vault data file and create a new vault.`,
  },
  {
    slug: "keyboard-shortcuts",
    title: "Keyboard Shortcuts",
    category: "reference",
    order: 6,
    keywords: ["keyboard", "shortcut", "hotkey", "key", "binding", "accelerator", "command"],
    body: `# Keyboard Shortcuts

Complete reference of all keyboard shortcuts. On macOS, Ctrl is replaced with ⌘ (Command).

## General

| Action | Windows/Linux | macOS |
|--------|--------------|-------|
| Command Palette | Ctrl+Shift+P | ⌘⇧P |
| Help Panel | F1 | F1 |
| Keyboard Shortcuts | Ctrl+/ | ⌘/ |
| Settings | Ctrl+, | ⌘, |
| Quick Connect | Ctrl+Shift+N | ⌘⇧N |
| Toggle Sidebar | Ctrl+B | ⌘B |
| Toggle Bottom Panel | Ctrl+J | ⌘J |

## Tabs

| Action | Windows/Linux | macOS |
|--------|--------------|-------|
| New Local Shell | Ctrl+T | ⌘T |
| Close Tab | Ctrl+W | ⌘W |
| Next Tab | Ctrl+Tab | ⌃Tab |
| Previous Tab | Ctrl+Shift+Tab | ⌃⇧Tab |
| Go to Tab 1–9 | Ctrl+1 – Ctrl+9 | ⌘1 – ⌘9 |

## Terminal

| Action | Windows/Linux | macOS |
|--------|--------------|-------|
| Copy | Ctrl+Shift+C | ⌘C |
| Paste | Ctrl+Shift+V | ⌘V |
| Clear Terminal | Ctrl+Shift+K | ⌘K |
| Search in Terminal | Ctrl+Shift+F | ⌘F |

## Split Panes

| Action | Windows/Linux | macOS |
|--------|--------------|-------|
| Split Right | Ctrl+Shift+D | ⌘⇧D |
| Split Down | Ctrl+Shift+E | ⌘⇧E |
| Focus Left Pane | Alt+Left | ⌥Left |
| Focus Right Pane | Alt+Right | ⌥Right |`,
  },
];

export const helpArticles: HelpArticle[] = [...rawArticles].sort((a, b) => a.order - b.order);

export function searchArticles(query: string): HelpArticle[] {
  if (!query.trim()) return helpArticles;
  const lower = query.toLowerCase();
  return helpArticles.filter(
    (a) =>
      a.title.toLowerCase().includes(lower) ||
      a.body.toLowerCase().includes(lower) ||
      a.keywords.some((k) => k.includes(lower)),
  );
}

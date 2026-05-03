import type { HelpArticle } from "@/types";

/**
 * helpContent.ts — in-app help articles bundled at build time.
 *
 * SOURCE OF TRUTH: docs/help/*.md
 * This file is kept in sync with the MkDocs site source so the same prose
 * appears both in the deployed documentation and in the in-app Help panel
 * (F1 / ⌘? / sidebar help icon). When updating an article, edit the markdown
 * file in docs/help/ first, then mirror the change here.
 *
 * MkDocs-specific syntax (admonitions: !!! tip, !!! note) is converted to
 * plain markdown blockquotes or bold text because the in-app MarkdownRenderer
 * uses a basic remark pipeline without the MkDocs extensions.
 *
 * LOCALE FALLBACK:
 * Articles can be loaded from locale-specific paths under docs/help/{locale}/
 * (e.g. docs/help/de/). When a locale-specific article is not found the system
 * falls back to the English content defined here.
 * To add a new locale, create docs/help/{locale}/ with matching markdown files.
 *
 * ARTICLE CATEGORIES (rendered as sections in the Help panel):
 *   basics            — Installation & Getting Started
 *   connections       — SSH, SFTP
 *   security          — Credential Vault, Security Guide
 *   tools             — Network Explorer
 *   reference         — Keyboard Shortcuts, Settings, Customization
 *   support           — Troubleshooting
 *   protocol-reference — SSH / RDP / VNC / Serial deep-dive
 *   developers        — Plugin API
 *
 * The `order` field controls sort order within the Help panel article list.
 * Category display order is determined by the order of first appearance.
 */

const rawArticles: Array<{
  slug: string;
  title: string;
  category: string;
  order: number;
  keywords: string[];
  body: string;
}> = [
  {
    slug: "installation",
    title: "Installation & Upgrade",
    category: "basics",
    order: 0,
    keywords: ["install", "download", "homebrew", "brew", "deb", "rpm", "appimage", "msi", "exe", "upgrade", "uninstall", "windows", "macos", "linux"],
    body: `# Installation & Upgrade

CrossTerm ships as a native binary for macOS, Windows, and Linux. No runtime dependencies are required — the installer bundles everything.

---

## macOS

### Homebrew (recommended)

Homebrew manages installation, upgrades, and uninstall automatically.

\`\`\`bash
brew tap alal76/crossterm
brew install --cask crossterm
\`\`\`

CrossTerm.app is placed in \`/Applications\`.

**Upgrade:**
\`\`\`bash
brew upgrade --cask crossterm
\`\`\`

**Check for updates:**
\`\`\`bash
brew outdated --cask
\`\`\`

**Uninstall:**
\`\`\`bash
brew uninstall --cask crossterm
brew untap alal76/crossterm   # optional: remove the tap
\`\`\`

### Direct DMG

1. Download \`CrossTerm_x.y.z_aarch64.dmg\` from the releases page.
2. Open the downloaded \`.dmg\` file.
3. Drag **CrossTerm.app** into your **Applications** folder.
4. Eject the disk image.

On first launch macOS may show a Gatekeeper dialog — open **System Settings → Privacy & Security** and click **Open Anyway** if prompted.

**Upgrade:** download and install the new DMG; it replaces the existing app.

**Uninstall:** drag \`CrossTerm.app\` from \`/Applications\` to the Trash. To remove user data:
\`\`\`bash
rm -rf ~/Library/Application\\ Support/com.crossterm.app
rm -rf ~/Library/Preferences/com.crossterm.app.plist
rm -rf ~/Library/Caches/com.crossterm.app
\`\`\`

### Verify checksum (optional)

Every release includes a \`.sha256\` file. To verify:
\`\`\`bash
shasum -a 256 -c CrossTerm_0.2.4_aarch64.dmg.sha256
\`\`\`

---

## Windows

### Installer (.exe)

1. Download \`CrossTerm_x.y.z_x64-setup.exe\` from the releases page.
2. Run the installer and follow the wizard.
3. CrossTerm is added to the Start menu and optionally to the Desktop.

**Upgrade:** run the new installer — it detects and replaces the existing version.

**Uninstall:** Settings → Apps → CrossTerm → Uninstall, or use the bundled uninstaller in the install directory.

### MSI package (enterprise/GPO)

The \`.msi\` package supports silent/unattended installation via Group Policy or SCCM.

**Silent install:**
\`\`\`powershell
msiexec /i CrossTerm_0.2.4_x64_en-US.msi /quiet /norestart
\`\`\`

**Silent uninstall:**
\`\`\`powershell
msiexec /x CrossTerm_0.2.4_x64_en-US.msi /quiet /norestart
\`\`\`

### Verify checksum (optional)

\`\`\`powershell
Get-FileHash CrossTerm_0.2.4_x64-setup.exe -Algorithm SHA256
# Compare output to CrossTerm_0.2.4_x64-setup.exe.sha256
\`\`\`

---

## Linux

### Debian / Ubuntu — .deb package

\`\`\`bash
# Download
wget https://github.com/alal76/CrossTerm/releases/latest/download/CrossTerm_0.2.4_amd64.deb

# Install
sudo dpkg -i CrossTerm_0.2.4_amd64.deb

# Fix any missing dependencies
sudo apt-get install -f
\`\`\`

CrossTerm is added to the application launcher and available as \`crossterm\` in PATH.

**Upgrade:**
\`\`\`bash
sudo dpkg -i CrossTerm_new_version_amd64.deb
\`\`\`

**Uninstall:**
\`\`\`bash
sudo dpkg -r crossterm
\`\`\`

### Red Hat / Fedora / SUSE — .rpm package

\`\`\`bash
# Fedora / RHEL
sudo dnf install CrossTerm-0.2.4-1.x86_64.rpm
\`\`\`

**Upgrade:**
\`\`\`bash
sudo rpm -U CrossTerm-new_version-1.x86_64.rpm
\`\`\`

**Uninstall:**
\`\`\`bash
sudo rpm -e crossterm-app
\`\`\`

### AppImage — universal portable binary

AppImages run on any modern Linux distribution without installation.

\`\`\`bash
# Download and make executable
wget https://github.com/alal76/CrossTerm/releases/latest/download/CrossTerm_0.2.4_amd64.AppImage
chmod +x CrossTerm_0.2.4_amd64.AppImage

# Run directly
./CrossTerm_0.2.4_amd64.AppImage
\`\`\`

**Upgrade:** download the new AppImage, make it executable, and replace the old file.

---

## System Requirements

| Platform | Minimum |
|----------|---------|
| macOS | 11.0 (Big Sur) or later, Apple Silicon or Intel |
| Windows | Windows 10 (1903) or later, 64-bit |
| Linux | Ubuntu 22.04 / Fedora 37 or equivalent; glibc 2.35+ |
| RAM | 256 MB (512 MB recommended) |
| Disk | 80 MB |

CrossTerm uses the system WebView (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux).

---

## Release channels

All releases are published on the GitHub releases page. There is currently one channel:

| Channel | Description |
|---------|-------------|
| **Stable** | Tagged \`vX.Y.Z\` releases — recommended for all users |`,
  },
  {
    slug: "getting-started",
    title: "Getting Started",
    category: "basics",
    order: 1,
    keywords: ["start", "setup", "first", "connect", "session", "terminal", "new", "quick"],
    body: `# Getting Started with CrossTerm

Welcome to CrossTerm — a cross-platform terminal emulator and remote access suite. This guide walks you through the interface and your first connections.

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
- Explore [Keyboard Shortcuts](keyboard-shortcuts) to work faster.`,
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

The simplest method. Enter your username and password when connecting. Passwords can be stored in the Credential Vault for quick access.

1. Open Quick Connect (**Ctrl+Shift+N** / **⌘⇧N**).
2. Enter hostname, port, and username.
3. Select **Password** authentication.
4. Enter your password or choose a saved credential.

### SSH Key Authentication

More secure than passwords. CrossTerm supports OpenSSH, PEM, and PKCS#8 key formats.

1. Open Quick Connect or create a new session.
2. Select **SSH Key** authentication.
3. Provide the path to your private key, or paste the key content.
4. If your key has a passphrase, enter it or use a vault credential.

**Supported key types:**
- RSA (2048-bit and above)
- Ed25519 (recommended)
- ECDSA (P-256, P-384, P-521)

### Certificate Authentication

For organizations using SSH certificates:

1. Select **Certificate** authentication.
2. Provide your certificate and matching private key.
3. Supports PEM and PKCS#12 formats.

## Jump Hosts (ProxyJump)

Connect through an intermediate server when direct access is not possible:

1. Edit your session settings.
2. Under **Advanced**, enable **Jump Host**.
3. Enter the jump host address, port, and credentials.
4. CrossTerm establishes the hop automatically.

You can chain multiple jump hosts for complex network topologies.

## Port Forwarding

### Local Forwarding

Forward a port from your local machine to a remote destination through the SSH tunnel:

- **Use case**: Access a remote database on \`db-server:5432\` through your SSH connection.
- **Configuration**: Local port 15432 → Remote \`db-server:5432\`.
- Connections to \`localhost:15432\` are tunneled to \`db-server:5432\` via the SSH server.

### Remote Forwarding

Expose a local service to the remote host:

- **Use case**: Let the remote server access your local dev server on port 3000.
- **Configuration**: Remote port 8080 → Local \`localhost:3000\`.

### Dynamic Forwarding (SOCKS Proxy)

Create a SOCKS5 proxy through the SSH tunnel:

- **Use case**: Route all browser traffic through the SSH server.
- Allocates a local SOCKS port that proxies traffic through the remote host.

## Agent Forwarding

Allow the remote server to use your local SSH keys for onward connections:

1. In session settings, enable **Agent Forwarding**.
2. Your local SSH agent (or CrossTerm's built-in agent) will be forwarded.
3. On the remote host, you can SSH to other servers using your local keys.

**Security note:** Only enable agent forwarding to trusted servers.

## Keep-Alive Settings

To prevent idle disconnections:

- **Keep Alive Interval**: Sends a packet every N seconds (default: 60).
- Configure per-session or globally in Settings → SSH.

## Host Key Verification

On first connection, CrossTerm displays the server's host key fingerprint. You can:

- **Accept**: Trust this key and save it.
- **Reject**: Cancel the connection.
- **Accept Once**: Connect without saving the key.

If a previously saved host key changes, CrossTerm shows a warning. This may indicate a server reinstallation or a potential security issue.

## Connection Troubleshooting

If you have trouble connecting, see the [Troubleshooting](troubleshooting) guide.`,
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

There are several ways to access the SFTP browser:

1. **New tab**: Click **+** → **New SFTP Browser**, then connect to a host.
2. **Bottom panel**: When connected to an SSH session, open the bottom panel (**Ctrl+J** / **⌘J**) and select the **SFTP** tab.
3. **From an SSH session**: The SFTP browser automatically uses the same connection credentials.

## Browsing Remote Files

The SFTP browser provides a dual-pane interface:

- **Left pane**: Local file system.
- **Right pane**: Remote file system.

Navigate directories by double-clicking folders. The path bar at the top shows the current location and supports direct path entry.

For each file and directory, you can see: file name and icon, file size (human-readable), last modified date, and Unix permissions.

## Uploading Files

### Drag and Drop

1. Open the SFTP browser.
2. Drag files from your desktop or file manager.
3. Drop them onto the remote pane.
4. A progress indicator shows the transfer status.

### Upload Button

1. Click the **Upload Files** button in the toolbar.
2. Select files from the file picker dialog.
3. Files are uploaded to the current remote directory.

Select multiple files to upload them in a batch. CrossTerm handles transfers concurrently for better performance.

## Downloading Files

1. Select one or more files in the remote pane.
2. Click **Download** or drag them to the local pane.
3. Choose a local destination if prompted.
4. Files are downloaded to the selected directory.

## Transfer Queue

When transferring multiple files, CrossTerm maintains a transfer queue:

- View active and pending transfers in the bottom panel.
- Each transfer shows progress percentage and speed.
- You can pause, resume, or cancel individual transfers.
- Failed transfers can be retried.

## File Operations

Right-click files or directories for additional operations:

- **Rename**: Change the file or directory name.
- **Delete**: Remove the file or directory (with confirmation).
- **Change permissions**: Modify Unix file permissions.
- **Create directory**: Create a new folder in the current location.

## Keyboard Navigation

- **Arrow keys**: Navigate the file list.
- **Enter**: Open directory or download file.
- **Delete/Backspace**: Delete selected files (with confirmation).
- **Ctrl+A / ⌘A**: Select all files.

## Performance Tips

- Large file transfers benefit from a stable connection with adequate bandwidth.
- Uploading many small files is slower than a few large files due to per-file overhead.
- Use compression in SSH settings for text-heavy transfers over slow connections.

## Security

All SFTP transfers are encrypted through the SSH tunnel. No data is transmitted in plaintext. File permissions and ownership are preserved during transfers when possible.`,
  },
  {
    slug: "credential-vault",
    title: "Credential Vault",
    category: "security",
    order: 4,
    keywords: ["vault", "credential", "password", "key", "security", "encrypt", "lock", "master", "store", "secret"],
    body: `# Credential Vault

The Credential Vault is CrossTerm's encrypted storage for passwords, SSH keys, API tokens, and other sensitive credentials. All data is encrypted with AES-256-GCM using a master password.

## Setting Up the Vault

On first launch, you are prompted to create a master password:

1. Choose a strong password (minimum 8 characters).
2. Confirm the password.
3. The vault is created and unlocked.

**Important:** There is no password recovery mechanism. If you forget your master password, vault contents cannot be recovered.

## Unlocking the Vault

When you start CrossTerm, the vault is locked by default:

1. Click the lock icon in the sidebar or status bar.
2. Enter your master password.
3. The vault unlocks, and credentials become available for connections.

## Storing Credentials

### Password Credentials

Store username/password pairs:

1. Open the vault (sidebar → Vault icon).
2. Click **Add Credential**.
3. Select type **Password**.
4. Enter a name, username, and password.
5. Click **Save**.

### SSH Keys

Store private keys for SSH authentication:

1. Click **Add Credential** → **SSH Key**.
2. Enter a descriptive name.
3. Paste your private key content.
4. Optionally add the passphrase.
5. Click **Save**.

### API Tokens

Store tokens for cloud services and APIs:

1. Click **Add Credential** → **API Token**.
2. Enter the provider name and token value.
3. Optionally set an expiry date.
4. Click **Save**.

### Cloud Credentials

Store AWS, Azure, or GCP access keys:

1. Click **Add Credential** → **Cloud Credential**.
2. Select the cloud provider.
3. Enter access key, secret key, and optional region.
4. Click **Save**.

## Using Credentials

When creating or editing a session:

1. In the authentication section, click the key icon.
2. A dropdown shows matching credentials from the vault.
3. Select a credential to auto-fill the authentication fields.

Credentials are referenced by ID — if you update a credential, all sessions using it get the new values automatically.

## Auto-Lock

For security, the vault automatically locks after a period of inactivity:

- **Default**: 15 minutes.
- **Configurable**: Settings → Security → Vault Auto-Lock.
- **Manual lock**: Click the lock icon or use Command Palette → Lock Vault.
- Setting auto-lock to 0 disables automatic locking.

## Changing the Master Password

1. Open Settings → Security.
2. Click **Change Master Password**.
3. Enter your current password.
4. Enter and confirm your new password.
5. All credentials are re-encrypted with the new key.

## Clipboard Security

When copying passwords from the vault:

- Passwords are automatically cleared from the clipboard after a configurable time.
- **Default**: 30 seconds.
- **Configurable**: Settings → Security → Clipboard Auto-Clear.
- Setting to 0 disables auto-clear.

## Security Details

- **Encryption**: AES-256-GCM with PBKDF2 key derivation.
- **Key material**: Held in memory with zeroize-on-drop protection.
- **No plaintext storage**: Credentials are never written to disk unencrypted.
- **Audit logging**: All vault access events are recorded in the audit log.`,
  },
  {
    slug: "network-explorer",
    title: "Network Explorer",
    category: "tools",
    order: 5,
    keywords: ["network", "scan", "port", "host", "subnet", "ping", "nmap", "wifi", "wireless", "explore", "discovery"],
    body: `# Network Explorer

The Network Explorer scans your local network to discover hosts, open ports, running services, and wireless access points. It is intended for network diagnostics and auditing on networks you own or have permission to scan.

Open the Network Explorer from the **+** button → **Network Explorer**, or via **Connect → Network Explorer** in the macOS menu bar.

---

## Subnet Detection

When the Network Explorer opens it automatically detects all active local network interfaces and pre-populates the subnet field with the detected ranges (e.g. \`192.168.1.0/24\`). You can edit the subnet manually if needed.

Multiple subnets can be scanned by separating them with commas:
\`\`\`
192.168.1.0/24, 10.0.0.0/24
\`\`\`

---

## Scanning

### Quick Scan

Scans the most common 100 ports across all hosts in the subnet. Fast, suitable for an overview of a typical home or office network.

- **Estimated time:** 5–30 seconds for a /24 subnet.

### Full Scan

Scans all 65 535 TCP ports on discovered hosts. Much slower but reveals unusual services.

- **Estimated time:** Several minutes for a /24 subnet.

### Starting a scan

1. Enter or confirm the subnet in the input field.
2. Click **Quick Scan** or **Full Scan**.
3. Results stream in as hosts respond — you do not need to wait for the full scan to finish.

Use the **Stop** button to abort a scan in progress.

---

## Results

Each discovered host is shown as a card with:

| Field | Description |
|-------|-------------|
| IP Address | IPv4 address of the host |
| Hostname | Reverse-DNS lookup result (if available) |
| MAC Address | Hardware address (ARP, LAN only) |
| Vendor | NIC manufacturer derived from the MAC OUI |
| Open Ports | List of responding TCP ports |
| Services | Inferred service names (SSH, HTTP, HTTPS, RDP, etc.) |
| Latency | Round-trip ping time in milliseconds |
| OS Guess | Operating system fingerprint (best-effort) |

Click any column header to sort by that field. Use the search box to filter by IP, hostname, or service name.

Clicking **SSH** or **RDP** next to an open port immediately opens a Quick Connect dialog pre-filled with that host's IP and the detected port.

---

## WiFi Analysis (macOS)

On macOS, the **WiFi** tab displays all visible wireless networks using the CoreWLAN framework.

| Field | Description |
|-------|-------------|
| SSID | Network name |
| BSSID | Access point MAC address |
| RSSI | Signal strength in dBm (higher is better, e.g. −50 dBm is excellent) |
| Channel | 2.4 GHz or 5 GHz channel number |
| Band | 2.4 GHz or 5 GHz |
| Security | WPA2, WPA3, Open, etc. |
| Country | Regulatory country code |

Click **Refresh** to re-scan. The currently connected network is highlighted.

> **Note (macOS):** The first time you use WiFi scanning, macOS may ask for location permission. This is required by Apple for apps that read WiFi details.

---

## Exporting Results

Click **Export** to save scan results:

- **JSON** — machine-readable, includes all fields per host.
- **CSV** — spreadsheet-compatible, one row per host/port combination.

---

## Connection History

The Explorer tracks every host you have previously connected to or scanned. Previously seen hosts are flagged in results so you can quickly identify new or unexpected devices.

---

## Security & Ethics

The Network Explorer uses raw TCP connection attempts and ICMP probes. Only scan networks you own or have explicit written permission to scan. Unauthorized port scanning may be illegal in your jurisdiction and violates the terms of service of most cloud providers.

CrossTerm's scan engine never sends exploit payloads — it only opens and closes TCP connections and sends standard ICMP echo requests.`,
  },
  {
    slug: "keyboard-shortcuts",
    title: "Keyboard Shortcuts",
    category: "reference",
    order: 6,
    keywords: ["keyboard", "shortcut", "hotkey", "key", "binding", "accelerator", "command"],
    body: `# Keyboard Shortcuts

A complete reference of all keyboard shortcuts in CrossTerm. On macOS, **Ctrl** is replaced with **⌘** (Command) unless otherwise noted.

## General

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Command Palette | Ctrl+Shift+P | ⌘⇧P |
| Help Panel | F1 | F1 |
| Keyboard Shortcuts | Ctrl+/ | ⌘/ |
| Settings | Ctrl+, | ⌘, |
| Quick Connect | Ctrl+Shift+N | ⌘⇧N |
| Toggle Sidebar | Ctrl+B | ⌘B |
| Toggle Bottom Panel | Ctrl+J | ⌘J |

## Tabs

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| New Local Shell | Ctrl+T | ⌘T |
| Close Tab | Ctrl+W | ⌘W |
| Next Tab | Ctrl+Tab | ⌃Tab |
| Previous Tab | Ctrl+Shift+Tab | ⌃⇧Tab |
| Go to Tab 1–9 | Ctrl+1 – Ctrl+9 | ⌘1 – ⌘9 |

## Terminal

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Copy | Ctrl+Shift+C | ⌘C |
| Paste | Ctrl+Shift+V | ⌘V |
| Select All | Ctrl+Shift+A | ⌘A |
| Clear Terminal | Ctrl+Shift+K | ⌘K |
| Search in Terminal | Ctrl+Shift+F | ⌘F |
| Zoom In | Ctrl+= | ⌘= |
| Zoom Out | Ctrl+- | ⌘- |
| Reset Zoom | Ctrl+0 | ⌘0 |

## Split Panes

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Split Right | Ctrl+Shift+D | ⌘⇧D |
| Split Down | Ctrl+Shift+E | ⌘⇧E |
| Focus Left Pane | Alt+Left | ⌥Left |
| Focus Right Pane | Alt+Right | ⌥Right |
| Focus Up Pane | Alt+Up | ⌥Up |
| Focus Down Pane | Alt+Down | ⌥Down |
| Close Pane | Ctrl+Shift+W | ⌘⇧W |

## Navigation

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Focus Sidebar | Ctrl+Shift+S | ⌘⇧S |
| Focus Terminal | Ctrl+Shift+T | ⌘⇧T |
| Focus Bottom Panel | Ctrl+Shift+J | ⌘⇧J |
| Cycle Focus Region | F6 | F6 |

## Vault

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Lock Vault | Ctrl+Shift+L | ⌘⇧L |
| Unlock Vault | (prompted on access) | (prompted on access) |

## SFTP Browser

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Upload Files | Ctrl+U | ⌘U |
| Download Selected | Ctrl+D | ⌘D |
| Refresh | Ctrl+R | ⌘R |
| Select All | Ctrl+A | ⌘A |
| Delete Selected | Delete | ⌫ |
| Navigate Up | Backspace | ⌫ |

## Broadcast Mode

| Action | Windows / Linux | macOS |
|--------|----------------|-------|
| Toggle Broadcast | Ctrl+Shift+B | ⌘⇧B |

Broadcast mode sends your input to all open terminal panes simultaneously — useful for running the same command across multiple servers.`,
  },
  {
    slug: "settings",
    title: "Settings Reference",
    category: "reference",
    order: 7,
    keywords: ["settings", "preferences", "configure", "theme", "font", "ssh", "keyboard", "notifications", "security", "advanced", "appearance"],
    body: `# Settings Reference

Open Settings with **⌘,** (macOS) or **Ctrl+,** (Windows/Linux), or via **Settings → Preferences…** in the menu bar.

The Settings panel is divided into ten tabs. All changes take effect immediately and are persisted to your active profile.

---

## General

| Setting | Description | Default |
|---------|-------------|---------|
| Default Profile | Which profile to activate on launch | Default |
| Startup Behavior | What to do when the app starts: Restore last session, Open new tab, or Show session browser | Restore last session |
| Confirm Close Tab | Ask for confirmation before closing a tab | Off |
| Show Status Bar | Display the status bar at the bottom of the window | On |
| Window Always on Top | Keep the CrossTerm window above all other windows | Off |
| Auto Update | Automatically download and install updates in the background | On |
| GPU Acceleration | Use WebGL hardware acceleration for the terminal renderer (requires restart) | On |

### Profile Sync

Export all settings, sessions, themes, and snippets to a \`.ctbundle\` archive, or import a bundle to restore a configuration. Useful for moving your setup between machines.

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
| Font Family | Terminal font | JetBrains Mono |
| Font Size | Point size, 8–32 | 14 |
| Line Height | Vertical line spacing multiplier, 1.0–2.0 | 1.2 |
| Letter Spacing | Extra horizontal space between characters, −2–10 px | 0 |
| Font Ligatures | Enable programming ligatures (e.g. \`=>\`, \`!=\`) for supported fonts | On |

### Terminal Display

| Setting | Description | Default |
|---------|-------------|---------|
| Cursor Style | block, underline, or bar | block |
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
| Bell Style | None, Visual (screen flash), or Sound | Visual |
| Terminal Encoding | UTF-8, ISO-8859-1, GBK, or Shift-JIS | UTF-8 |
| Word Separators | Characters that delimit words for double-click selection | \` ()[]{}'""\` |
| Default Shell | Override the system default shell for new local tabs | (system default) |
| Tab Title Format | Template for tab titles. Tokens: \`{host}\`, \`{user}\`, \`{cwd}\`, \`{cmd}\` | \`{host}\` |

---

## SSH

| Setting | Description | Default |
|---------|-------------|---------|
| Default SSH Port | Port used when no port is specified | 22 |
| Keepalive Interval | Seconds between SSH keepalive packets. 0 disables keepalives | 60 |
| Strict Host Checking | Reject connections if the host key has changed | On |
| SSH Compression | Enable zlib compression for SSH data streams | Off |
| Agent Forwarding | Forward your local SSH agent to remote sessions by default | Off |
| X11 Forwarding | Forward X11 display connections through the SSH tunnel by default | Off |

---

## Connections

| Setting | Description | Default |
|---------|-------------|---------|
| Connection Timeout | Seconds to wait before giving up on a new connection | 30 |
| Reconnect on Disconnect | Automatically attempt to reconnect if a session drops | On |
| Reconnect Delay | Seconds to wait before the first reconnect attempt | 5 |
| Max Reconnect Attempts | Stop retrying after this many consecutive failures. 0 = unlimited | 5 |
| Max Concurrent Connections | Limit the total number of simultaneously active sessions | 20 |

---

## File Transfer

| Setting | Description | Default |
|---------|-------------|---------|
| Default Remote Directory | Starting directory when opening the SFTP browser | \`/home/{user}\` |
| SFTP Encoding | Character encoding for remote file names | UTF-8 |
| Confirm Overwrite | Ask before overwriting existing files during transfers | On |
| Preserve Timestamps | Keep the source file's modification time on the destination | On |
| Concurrent Transfer Jobs | Number of files to upload or download simultaneously | 4 |

---

## Keyboard

| Setting | Description | Default |
|---------|-------------|---------|
| Backspace Sends Delete | Send DEL instead of BS when Backspace is pressed | Off |
| Alt is Meta | Treat the Alt/Option key as the terminal Meta key | On |
| Ctrl+H is Backspace | Interpret Ctrl+H as Backspace (legacy terminal compatibility) | Off |
| Home/End Scroll Buffer | Home and End keys scroll the terminal buffer | Off |
| Right-Click Action | Context Menu, Paste, or Select Word | Context Menu |

---

## Notifications

| Setting | Description | Default |
|---------|-------------|---------|
| Notify on Disconnect | Show a system notification when a session disconnects unexpectedly | On |
| Notify on Bell | Trigger a system notification when the terminal bell fires | Off |
| Notify on Long Process | Notify when a command has been running longer than the threshold | On |
| Long Process Threshold | Seconds a command must run before a notification fires | 30 |
| Flash Tab on Bell | Highlight the tab title when the bell fires in a background tab | On |

---

## Security

### Clipboard

| Setting | Description | Default |
|---------|-------------|---------|
| Copy on Select | Automatically copy selected text to the clipboard | Off |
| Paste Warning Threshold | Warn before pasting more than this many lines | 5 |
| Clipboard History Size | Number of clipboard entries to retain | 20 |

### Vault Lock

| Setting | Description | Default |
|---------|-------------|---------|
| Idle Lock Timeout | Automatically lock the credential vault after this many seconds of inactivity. 0 disables auto-lock | 0 (disabled) |

---

## Advanced

| Setting | Description | Default |
|---------|-------------|---------|
| Log Level | Backend log verbosity: Error, Warn, Info, Debug, Trace | Warn |
| Telemetry | Send anonymous crash and usage statistics | Off |

The bottom of the Advanced tab shows read-only diagnostic information: Version, Platform, and Renderer. A button to **Export Audit Log** writes all audit events to a CSV file.

---

## Settings Persistence

Settings are stored per-profile in \`~/Library/Application Support/com.crossterm.app/\` (macOS), \`%APPDATA%\\com.crossterm.app\\\` (Windows), or \`~/.config/com.crossterm.app/\` (Linux).

To back up or migrate your settings, use **Profile Sync → Export** in the General tab.`,
  },
  {
    slug: "customization",
    title: "Customization",
    category: "reference",
    order: 8,
    keywords: ["theme", "custom", "keybinding", "font", "color", "appearance", "terminal", "style", "import", "shortcut"],
    body: `# Customization

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

Select **System** to follow your operating system's light/dark mode preference. CrossTerm listens for \`prefers-color-scheme\` changes and updates automatically.

### Importing Custom Themes

You can import custom themes from JSON files:

1. Open **Settings** → **Appearance** → **Import Theme**.
2. Select a \`.json\` theme file from your filesystem.
3. The theme is loaded and applied immediately.

Custom theme files must follow the CrossTerm design token schema. A valid theme file includes:

\`\`\`json
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
\`\`\`

### Design Token System

CrossTerm uses a three-layer token indirection system:

1. **CSS Custom Properties** — Raw values defined at the \`:root\` level.
2. **Tailwind Config** — Maps tokens to Tailwind utility classes.
3. **Utility Classes** — Used in components (e.g., \`bg-surface-primary\`, \`text-accent-primary\`).

## Terminal Appearance

### Font Settings

Configure terminal fonts from **Settings** → **Appearance**:

- **Font Family** — Default: \`JetBrains Mono\`. Any monospace font installed on your system works.
- **Font Size** — Default: 14px. Range: 8–32px.
- **Line Height** — Default: 1.2. Adjust for tighter or looser line spacing.
- **Letter Spacing** — Default: 0. Fine-tune character spacing.
- **Font Ligatures** — Enable/disable programming ligatures (e.g., \`=>\`, \`!=\`).

### Cursor

- **Cursor Style** — Block, Underline, or Bar.
- **Cursor Blink** — Enable or disable cursor blinking.

### Opacity

Adjust terminal background transparency from **Settings** → **Appearance** → **Terminal Opacity**. Values range from 0 (fully transparent) to 1 (fully opaque).

### Bell Mode

Choose how the terminal bell character is rendered:

- **None** — Silent.
- **Visual** — Brief flash on the terminal pane.
- **Audio** — System beep sound.

## Tab Title Format

Customize how tab titles are displayed from **Settings** → **Terminal** → **Tab Title Format**. Use template variables:

- \`{name}\` — Session name
- \`{host}\` — Remote hostname
- \`{user}\` — Username
- \`{shell}\` — Shell name

Example: \`{user}@{host}\` displays as \`root@server1\`.

## Scrollback Buffer

Configure the number of lines kept in the terminal scrollback buffer from **Settings** → **Terminal** → **Scrollback Lines**. Default is 10,000 lines. Higher values use more memory.

## GPU Acceleration

Enable or disable hardware-accelerated rendering from **Settings** → **General** → **GPU Acceleration**. Enabled by default for smooth performance. Disable if you experience rendering issues on older hardware.`,
  },
  {
    slug: "security-guide",
    title: "Security Guide",
    category: "security",
    order: 9,
    keywords: ["security", "vault", "encrypt", "aes", "argon2", "password", "master", "known hosts", "cipher", "credential", "safe"],
    body: `# Security Guide

CrossTerm is designed with security as a core principle. This guide covers the encryption model, credential storage, SSH security, and best practices for keeping your connections safe.

## Credential Vault Encryption

### Encryption Algorithm

All credentials stored in the vault are encrypted using **AES-256-GCM** (Advanced Encryption Standard with 256-bit keys in Galois/Counter Mode). This provides both confidentiality and authenticity — any tampering with encrypted data is detected.

- **Algorithm:** AES-256-GCM
- **Key Size:** 256 bits
- **Nonce:** 96-bit random nonce per encryption operation
- **Tag Size:** 128-bit authentication tag

### Key Derivation

The encryption key is derived from your master password using **Argon2id**, the winner of the Password Hashing Competition. Argon2id combines memory-hardness (requires significant RAM, making GPU/ASIC attacks expensive), time-hardness (multiple iterations increase computation cost), and side-channel resistance.

Default parameters:
- Memory: 64 MiB
- Iterations: 3
- Parallelism: 4
- Salt: 16 bytes (randomly generated per vault)
- Output: 32 bytes (256-bit key)

### Memory Protection

Key material is held in Rust's \`Zeroizing<Vec<u8>>\` wrapper, which guarantees the key is overwritten with zeros when it goes out of scope. This prevents key material from lingering in memory after the vault is locked.

### No Backdoor

There is no password recovery mechanism. If you forget your master password, the vault contents **cannot** be recovered. This is by design — it ensures that even the application developers cannot access your credentials.

## Master Password Best Practices

Choose a strong master password:

1. **Length** — Use at least 12 characters. Longer is better.
2. **Complexity** — Mix uppercase, lowercase, numbers, and symbols.
3. **Uniqueness** — Never reuse your master password elsewhere.
4. **Passphrase** — Consider a random multi-word passphrase (e.g., "correct-horse-battery-staple").
5. **Password Manager** — Store your master password in a separate password manager as a backup.

## SSH Security

### Known Hosts Verification

CrossTerm verifies SSH server fingerprints against a known hosts database:

1. **First Connection** — You are prompted to accept or reject the server's host key fingerprint.
2. **Subsequent Connections** — The fingerprint is compared against the stored value. If it changes, CrossTerm warns you of a potential man-in-the-middle attack.
3. **Storage** — Known hosts are stored locally per profile, isolated from other profiles.

### Cipher Policy

CrossTerm supports modern SSH ciphers and key exchange algorithms:

**Ciphers (in order of preference):**
- \`chacha20-poly1305@openssh.com\`
- \`aes256-gcm@openssh.com\`
- \`aes128-gcm@openssh.com\`
- \`aes256-ctr\`, \`aes192-ctr\`, \`aes128-ctr\`

**Key Exchange:**
- \`curve25519-sha256\`
- \`ecdh-sha2-nistp256\`, \`ecdh-sha2-nistp384\`
- \`diffie-hellman-group16-sha512\`, \`diffie-hellman-group14-sha256\`

Legacy algorithms (e.g., \`arcfour\`, \`blowfish-cbc\`, \`diffie-hellman-group1-sha1\`) are **not** supported.

### SSH Key Types

- **Ed25519** (recommended) — Fast, secure, compact keys
- **ECDSA** (P-256, P-384, P-521) — Elliptic curve keys
- **RSA** (2048-bit minimum) — Legacy compatibility; 4096-bit recommended

### Agent Forwarding

SSH agent forwarding allows you to use your local SSH keys on remote servers without copying the keys. **Caution:** Only enable agent forwarding on trusted servers, as a compromised server could use your forwarded agent.

## Credential Storage

The vault stores six types of credentials:

| Type | Use Case |
|------|----------|
| Password | Username/password authentication |
| SSH Key | Key-based SSH authentication |
| Certificate | X.509 certificate authentication |
| API Token | Service tokens (GitHub, AWS, etc.) |
| Cloud Credential | AWS/Azure/GCP access keys |
| TOTP Seed | Time-based one-time passwords |

Sensitive fields (passwords, private keys, tokens) are marked with \`#[serde(skip_serializing)]\` and never written to logs or exported.

## Audit Trail

CrossTerm logs security-relevant events to the Audit Log:

- Vault unlock / lock events
- Credential access (read, create, update, delete)
- SSH connection attempts and authentication results
- Port forwarding setup and teardown
- Profile switches

View the audit log from the **Bottom Panel** → **Audit Log** tab.

## Security Checklist

- Use a strong, unique master password (12+ characters)
- Enable vault auto-lock (15 minutes or less)
- Enable clipboard auto-clear for copied passwords
- Use Ed25519 SSH keys where possible
- Verify SSH host key fingerprints on first connection
- Only enable SSH agent forwarding on trusted hosts
- Keep CrossTerm updated for security patches`,
  },
  {
    slug: "troubleshooting",
    title: "Troubleshooting",
    category: "support",
    order: 10,
    keywords: ["error", "problem", "fix", "debug", "fail", "connect", "timeout", "refuse", "key", "auth", "firewall", "dns"],
    body: `# Troubleshooting

This guide covers common issues and their solutions when using CrossTerm.

## Connection Issues

### Authentication Failed

**Symptom:** "Authentication failed" or "Permission denied" error.

**Possible causes and solutions:**

1. **Wrong password**: Double-check your password. Passwords are case-sensitive.
2. **Wrong username**: Verify the username for the remote host.
3. **SSH key not accepted**: Ensure the public key is in the server's \`~/.ssh/authorized_keys\`.
4. **Key format mismatch**: CrossTerm supports OpenSSH, PEM, and PKCS#8 formats. Convert if needed.
5. **Passphrase required**: If your key has a passphrase, provide it in the credential or at the prompt.
6. **Server restrictions**: The server may not allow the authentication method you're using. Check \`/etc/ssh/sshd_config\`.

### Connection Timed Out

**Symptom:** Connection hangs and eventually shows "Connection timed out."

**Possible causes and solutions:**

1. **Host unreachable**: Verify the hostname or IP address. Try \`ping hostname\` in a local terminal.
2. **Wrong port**: SSH typically uses port 22. Verify the correct port with your administrator.
3. **Firewall blocking**: A firewall may be blocking the connection. Check both local and remote firewall rules.
4. **DNS resolution**: If using a hostname, ensure DNS resolves correctly. Try connecting by IP address.

### Connection Refused

**Symptom:** "Connection refused" error immediately.

**Possible causes and solutions:**

1. **SSH server not running**: The SSH daemon may not be running on the remote host.
2. **Wrong port**: The SSH server may be listening on a non-standard port.
3. **Firewall rules**: The port may be blocked by a firewall.
4. **Host-based restrictions**: The server may restrict connections by source IP.

### Host Key Verification Failed

**Symptom:** Warning about changed host key or verification failure.

This could mean the server was reinstalled, SSH keys were regenerated, or — in the worst case — a potential man-in-the-middle attack.

1. Verify with your system administrator that the key change is expected.
2. If confirmed safe, remove the old key and accept the new one.
3. If unexpected, do not connect and investigate further.

## Terminal Issues

### Terminal Not Rendering

1. Try resizing the terminal window.
2. Check Settings → Appearance → Font Family for a valid monospace font.
3. Toggle GPU Acceleration in Settings → General.
4. Restart the terminal session.

### Copy/Paste Not Working

1. **Copy**: Select text and use Ctrl+Shift+C / ⌘C, or enable "Copy on Select" in Settings.
2. **Paste**: Use Ctrl+Shift+V / ⌘V.
3. Multi-line paste shows a confirmation dialog by default. Check Settings → Security → Paste Confirmation.

### Colors Look Wrong

1. Check your current theme in Settings → Appearance → Theme.
2. Try a different theme to see if the issue persists.
3. Verify the remote server's terminal type setting (\`$TERM\`).

## Performance Issues

### Slow Terminal Response

1. Check your network latency to the remote host.
2. Reduce scrollback buffer in Settings → Terminal → Scrollback Lines.
3. Disable GPU Acceleration if it's causing issues.
4. Close unused tabs to free resources.

### High Memory Usage

1. Reduce scrollback buffer size.
2. Close idle sessions you're not using.
3. Large file transfers via SFTP can temporarily increase memory usage.

## Vault Issues

### Forgot Master Password

The master password cannot be recovered. You will need to:

1. Delete the vault data file.
2. Create a new vault.
3. Re-enter all credentials.

### Vault Won't Unlock

1. Ensure Caps Lock is not enabled.
2. Check if you're using the correct profile (each profile can have its own vault).
3. Try restarting CrossTerm.

## Getting Help

If your issue is not covered here:

1. Check the other help articles for feature-specific guidance.
2. Visit the CrossTerm issue tracker to report bugs.
3. Include the following when reporting issues: CrossTerm version, OS version, and steps to reproduce.`,
  },
  {
    slug: "ssh-protocol-reference",
    title: "SSH Protocol Reference",
    category: "protocol-reference",
    order: 11,
    keywords: ["ssh", "cipher", "key exchange", "port forwarding", "proxy", "agent", "known_hosts"],
    body: `# SSH Protocol Reference

This reference covers the SSH protocol capabilities supported by CrossTerm, including cipher suites, key exchange algorithms, authentication, port forwarding, and performance tuning.

## Cipher Suites

CrossTerm negotiates ciphers in preference order:

| Cipher | Key Size | Mode | Notes |
|--------|----------|------|-------|
| \`chacha20-poly1305@openssh.com\` | 256-bit | AEAD | Preferred. Constant-time, no padding oracle. |
| \`aes256-gcm@openssh.com\` | 256-bit | AEAD | Hardware-accelerated on AES-NI CPUs. |
| \`aes128-gcm@openssh.com\` | 128-bit | AEAD | Faster than AES-256 with slightly smaller key. |
| \`aes256-ctr\` | 256-bit | CTR | Widely compatible fallback. |
| \`aes192-ctr\` | 192-bit | CTR | Intermediate key size. |
| \`aes128-ctr\` | 128-bit | CTR | Maximum compatibility. |

AEAD ciphers (ChaCha20-Poly1305 and AES-GCM) provide both encryption and integrity in a single operation.

## Key Exchange Algorithms

| Algorithm | Type | Notes |
|-----------|------|-------|
| \`curve25519-sha256\` | ECDH | Default. Fast, constant-time, 128-bit security. |
| \`curve25519-sha256@libssh.org\` | ECDH | Alias for the above (legacy naming). |
| \`ecdh-sha2-nistp256\` | ECDH | NIST P-256 curve. FIPS 140-2 compliant. |
| \`ecdh-sha2-nistp384\` | ECDH | NIST P-384 curve. Higher security margin. |
| \`diffie-hellman-group16-sha512\` | DH | 4096-bit modulus. Fallback for legacy servers. |
| \`diffie-hellman-group14-sha256\` | DH | 2048-bit modulus. Minimum recommended. |

CrossTerm rejects \`diffie-hellman-group1-sha1\` and \`diffie-hellman-group14-sha1\` due to SHA-1 deprecation.

## Host Key Verification

CrossTerm uses Trust-On-First-Use (TOFU) for host key management. On first connection, the server's public key fingerprint is stored in \`~/.crossterm/known_hosts\`. Subsequent connections verify the key matches.

If the host key changes, CrossTerm displays a **host key mismatch warning** and refuses the connection to protect against MITM attacks.

Supported host key algorithms: \`ssh-ed25519\`, \`ecdsa-sha2-nistp256\`, \`rsa-sha2-512\`, \`rsa-sha2-256\`.

## Authentication Methods

### Password

Standard password authentication. Passwords can be stored in the encrypted Credential Vault.

### Public Key

Supported key formats: OpenSSH, PEM (PKCS#1/PKCS#8). Supported key types: Ed25519 (recommended), ECDSA (P-256, P-384), RSA (2048-bit minimum).

### SSH Agent Forwarding

When enabled, CrossTerm forwards the local SSH agent socket to the remote host, allowing the remote session to authenticate to further hosts using the local agent's keys without storing private keys on the remote machine.

Agent forwarding uses the \`SSH_AUTH_SOCK\` environment variable on Unix systems.

## ProxyJump (Jump Hosts)

CrossTerm supports multi-hop connections through one or more jump hosts. Each hop establishes an independent SSH session, then the next hop tunnels through the previous session's \`direct-tcpip\` channel.

## Port Forwarding

### Local Forwarding (-L)

Binds a local port and forwards connections through the SSH tunnel to a remote destination. Use cases: accessing remote databases, internal web services, or APIs behind a firewall.

### Remote Forwarding (-R)

Binds a port on the remote server and forwards incoming connections back through the tunnel to a local destination. Use cases: exposing a local development server to a remote network.

### Dynamic Forwarding (SOCKS5) (-D)

Creates a local SOCKS5 proxy. Applications configured to use this proxy have their traffic forwarded through the SSH tunnel. Supports IPv4, IPv6, and domain name address types.

## Performance Tuning

- **Keepalive interval**: Configurable interval (default 15s) for SSH keepalive packets.
- **Keepalive max**: Maximum missed keepalives before disconnecting (default 3).
- **TCP_NODELAY**: Enabled by default to reduce latency for interactive sessions.
- **Compression**: zlib compression can be enabled for slow links. Not recommended for fast networks.
- **Channel window size**: Adjustable for bulk data transfer (SFTP). Larger windows improve throughput on high-latency links.

## Security Considerations

- All sessions use strict key exchange when supported by the server.
- Passwords and key passphrases are held in \`Zeroizing\` memory and cleared after authentication.
- Host key verification prevents MITM attacks. Never disable host key checking in production.
- Agent forwarding should only be enabled to trusted servers.`,
  },
  {
    slug: "rdp-protocol-reference",
    title: "RDP Protocol Reference",
    category: "protocol-reference",
    order: 12,
    keywords: ["rdp", "remote desktop", "NLA", "clipboard", "multi-monitor", "RemoteApp"],
    body: `# RDP Protocol Reference

This reference covers the Remote Desktop Protocol (RDP) capabilities supported by CrossTerm, including security modes, codec options, clipboard integration, and multi-monitor support.

## Connection Security

### Network Level Authentication (NLA)

NLA is the default and recommended authentication mode. It authenticates the user before establishing the full RDP session, reducing the attack surface of the remote host. NLA uses CredSSP which wraps NTLM or Kerberos authentication inside a TLS channel.

CrossTerm requires NLA for all connections by default.

### TLS Transport

All RDP connections are encrypted using TLS 1.2 or 1.3. Server certificate validation is enforced — self-signed certificates trigger a warning dialog showing the certificate fingerprint and expiration.

### RDP Security Layer (Legacy)

The original RDP encryption using RC4. This mode is considered insecure and is only available as a fallback for legacy Windows XP/Server 2003 hosts. CrossTerm displays a security warning when this mode is negotiated.

## Codec Options

| Codec | Compression | Notes |
|-------|-------------|-------|
| RemoteFX (RFX) | Progressive | Best quality. Hardware-accelerated decode. |
| NSCodec | Lossy | Good balance of quality and bandwidth. |
| Bitmap (RLE) | Lossless | Fallback. Higher bandwidth usage. |

**Color Depth:** Supported: 32-bit (true color, default), 24-bit, 16-bit, and 8-bit.

**Frame Rate:** Configurable from 1–60 FPS. Default is 30 FPS.

## Display Configuration

### Resolution

CrossTerm supports arbitrary resolutions up to 8192×8192 pixels per monitor. Resolution can be:

- **Fit to window**: Automatically scales to the CrossTerm pane size.
- **Fixed**: Set a specific resolution (e.g., 1920×1080).
- **Match local**: Uses the local monitor's native resolution.

### Multi-Monitor

CrossTerm supports spanning the remote desktop across multiple monitors. Each monitor is reported to the remote host with its geometry (position, size, DPI).

### DPI Scaling

DPI-aware rendering ensures text and UI elements appear at the correct size on high-DPI displays.

## Clipboard Integration

Bidirectional clipboard sharing supports:

- **Text**: Plain text and rich text (RTF).
- **Files**: Drag-and-drop file transfer via clipboard redirection.
- **Images**: Bitmap clipboard content (e.g., screenshots).

Clipboard redirection can be disabled per-session for security-sensitive environments.

## Device Redirection

### Drive Mapping

Local drives or directories can be mapped into the remote session as network drives.

### Audio

Remote audio can be played locally (default), played on the remote host, or disabled. Audio recording redirection (microphone) is supported.

### Printer

Local printers can be redirected to the remote session, allowing printing from remote applications to local printers.

## RemoteApp

RemoteApp mode launches individual applications from the remote host as if they were local windows, without showing the full remote desktop. Each RemoteApp window integrates with the local taskbar and window management.

## Performance Tuning

- **Bandwidth auto-detect**: CrossTerm negotiates codec and quality settings based on measured connection speed.
- **Persistent bitmap caching**: Caches frequently used bitmaps locally to reduce repeated transfers.
- **Font smoothing**: ClearType can be disabled to reduce bandwidth.
- **Desktop composition**: Aero/DWM can be disabled for lower bandwidth usage.
- **Reconnection**: Automatic reconnection attempts on network interruption with session state preservation.

## Security Considerations

- Always use NLA to prevent unauthenticated resource consumption on the remote host.
- Verify server certificates to protect against MITM attacks.
- Disable clipboard and drive redirection when connecting to untrusted servers.
- CrossTerm rejects SSL 3.0 and TLS 1.0/1.1.`,
  },
  {
    slug: "vnc-protocol-reference",
    title: "VNC Protocol Reference",
    category: "protocol-reference",
    order: 13,
    keywords: ["vnc", "TLS", "encoding", "clipboard", "tight", "ultra", "zrle"],
    body: `# VNC Protocol Reference

This reference covers the VNC (Virtual Network Computing) protocol capabilities supported by CrossTerm, including security modes, encoding types, clipboard handling, and performance tuning.

## Connection Modes

### Direct Connection

Connect directly to a VNC server by specifying hostname and port. The default VNC port is 5900 (display :0). Display number N maps to port 5900+N.

### Reverse Connection (Listening Mode)

CrossTerm can listen for incoming VNC connections. The remote server initiates the connection to CrossTerm on a specified port (default 5500). Useful when the VNC server is behind a firewall.

### Gateway/Repeater

Connect through a VNC repeater or gateway by specifying the repeater address and the target server's ID.

## Security Modes

| Mode | Authentication | Encryption | Notes |
|------|---------------|------------|-------|
| VeNCrypt TLS + X.509 | Certificate | TLS 1.2+ | Strongest. Mutual authentication. |
| VeNCrypt TLS + Password | VNC password | TLS 1.2+ | Encrypted channel with password auth. |
| VeNCrypt Plain TLS | None | TLS 1.2+ | Encrypted but unauthenticated. |
| TLS Anonymous | None | TLS (anon DH) | Vulnerable to MITM. Not recommended. |

Standard VNC password authentication uses DES-based challenge-response. CrossTerm warns when connecting with VNC authentication over an unencrypted channel.

## Encoding Types

| Encoding | Type | Best For |
|----------|------|----------|
| Tight | Lossy/Lossless | General use. JPEG for photos, zlib for text. |
| ZRLE (Zlib Run-Length) | Lossless | Good compression, moderate CPU. |
| Ultra | Lossy | Low bandwidth. Aggressive compression. |
| Hextile | Lossless | Legacy. Low CPU, moderate bandwidth. |
| RRE | Lossless | Simple scenes with large solid areas. |
| Raw | None | LAN only. Highest bandwidth, lowest CPU. |
| CopyRect | N/A | Window moves and scrolling. Always enabled. |

Tight encoding uses JPEG compression for photographic regions (quality 1–9, configurable) and zlib for text/UI elements.

## Pixel Format

CrossTerm negotiates pixel format with the server:

- **True color (32-bit)**: Default. Full color fidelity.
- **16-bit**: Reduces bandwidth by ~50% with minor color loss.
- **8-bit**: Palette mode. Maximum bandwidth savings.

## Clipboard Integration

VNC clipboard uses \`ServerCutText\` and \`ClientCutText\` messages for bidirectional text transfer.

- **Text only**: No file or image clipboard support in standard VNC.
- **Latin-1 encoding**: Standard VNC clipboard is limited to ISO 8859-1. Extended clipboard (if supported by server) enables UTF-8.

## Input Handling

CrossTerm translates local key events to X11 keysyms for transmission. Dead keys and compose sequences are supported for international input. All mouse buttons (left, middle, right, scroll up/down) are transmitted.

## Performance Tuning

- **Encoding selection**: Use Tight or ZRLE for WAN. Use Raw for gigabit LAN.
- **JPEG quality**: Lower values (1–3) for slow links. Higher (7–9) for LAN.
- **Color depth**: Reduce to 16-bit or 8-bit for constrained bandwidth.
- **Compression level**: Zlib compression level (1–9) trades CPU for bandwidth. Level 6 is default.
- **Cursor handling**: Local cursor rendering eliminates cursor lag on high-latency connections.

## Security Considerations

- Always use VeNCrypt with TLS for connections over untrusted networks.
- Standard VNC authentication without TLS exposes the password hash and all session data.
- Disable clipboard sharing when connecting to untrusted servers.
- VNC passwords are limited to 8 characters. Use TLS client certificates for stronger authentication.`,
  },
  {
    slug: "serial-protocol-reference",
    title: "Serial Protocol Reference",
    category: "protocol-reference",
    order: 14,
    keywords: ["serial", "baud", "parity", "flow control", "RS-232", "UART", "break"],
    body: `# Serial Protocol Reference

This reference covers serial port communication capabilities in CrossTerm, including baud rates, data framing, flow control, and break signal handling.

## Connection Parameters

### Baud Rate

| Category | Rates |
|----------|-------|
| Low speed | 300, 1200, 2400, 4800 |
| Standard | 9600 (default), 19200, 38400 |
| High speed | 57600, 115200, 230400 |
| Very high speed | 460800, 921600, 1000000, 2000000, 4000000 |

Custom baud rates can be entered manually. Common embedded defaults: 9600 (Arduino), 115200 (ESP32, Raspberry Pi).

### Data Bits

| Value | Usage |
|-------|-------|
| 8 | Default. Standard for modern devices. |
| 7 | Legacy ASCII terminals, some industrial protocols (Modbus ASCII). |
| 6 | Rare. Historical teletypes. |
| 5 | Baudot code. Teletype (TTY) devices. |

### Stop Bits

| Value | Usage |
|-------|-------|
| 1 | Default. Sufficient for most applications. |
| 1.5 | Used with 5 data bits on some hardware. |
| 2 | Provides more margin at low baud rates or long cables. |

### Parity

| Mode | Description |
|------|-------------|
| None | No parity bit. Default for most modern devices. |
| Odd | Parity bit set so total 1-bits (including parity) is odd. |
| Even | Parity bit set so total 1-bits (including parity) is even. |
| Mark | Parity bit always 1. Used for 9-bit addressing in multi-drop. |
| Space | Parity bit always 0. Rarely used. |

Common configurations: \`9600 8N1\` is the most widely used default.

## Flow Control

### Hardware Flow Control (RTS/CTS)

Uses dedicated RS-232 signal lines. When CTS is deasserted, the sender pauses transmission until CTS is reasserted. This is the most reliable flow control method and is recommended for high baud rates.

### Software Flow Control (XON/XOFF)

Uses in-band control characters (XOFF: Ctrl+S to pause, XON: Ctrl+Q to resume). Works over 3-wire connections but is not suitable for binary data.

### No Flow Control

Suitable for low baud rates or when the application protocol handles its own flow control.

## Break Signal

The break signal is a special condition where the TX line is held in the spacing state for longer than one frame duration. Uses:

- **Attention/interrupt**: Some devices use break as an attention signal.
- **Magic SysRq**: Linux kernel triggers special debugging functions on serial break.
- **Cisco IOS**: Break signal enters ROM monitor mode.

CrossTerm can send break via the terminal menu. Break duration is configurable (default 250ms).

## RS-232 Signal Lines

| Signal | Direction | Purpose |
|--------|-----------|---------|
| DTR | Output | Data Terminal Ready |
| RTS | Output | Request To Send |
| DSR | Input | Data Set Ready |
| CTS | Input | Clear To Send |
| DCD | Input | Data Carrier Detect |
| RI | Input | Ring Indicator |

## Line Endings

| Mode | TX sends | RX interprets |
|------|----------|---------------|
| CR | \`\\r\` | Carriage return as newline |
| LF | \`\\n\` | Line feed as newline |
| CRLF | \`\\r\\n\` | CR+LF as newline |

## Hex View

CrossTerm provides a hex viewer mode for serial sessions, displaying received data as hexadecimal bytes alongside ASCII representation. Useful for debugging binary protocols.

## Logging

Serial session data can be logged to a file in raw, hex, or timestamped formats. Timestamps use ISO 8601 format with millisecond precision.

## Troubleshooting

- **No data received**: Verify baud rate, data bits, parity, and stop bits match the device.
- **Garbled output**: Usually indicates mismatched baud rate. Try common rates: 9600, 115200.
- **Data loss at high rates**: Enable hardware flow control (RTS/CTS).
- **Permission denied (Linux)**: Add your user to the \`dialout\` group.`,
  },
  {
    slug: "plugin-api-guide",
    title: "Plugin API Developer Guide",
    category: "developers",
    order: 15,
    keywords: ["plugin", "api", "wasm", "extension", "developer"],
    body: `# Plugin API Developer Guide

> **Note**: The Plugin API is planned for CrossTerm Phase 3. This guide documents the planned architecture and API surface. Implementation is in progress.

## Overview

CrossTerm plugins extend terminal functionality through a sandboxed WebAssembly (WASM) runtime. Plugins can:

- **Add sidebar panels** — Custom UI panels rendered in the sidebar region.
- **React to SSH output** — Intercept and process terminal output streams for pattern detection, alerting, or logging.
- **Process terminal lines** — Transform, annotate, or highlight individual output lines before rendering.
- **Register custom commands** — Extend the command palette with plugin-provided actions.

Plugins run inside a WASI sandbox with explicit capability grants. They cannot access the host filesystem, network, or other system resources unless the user grants the corresponding permission.

## Plugin Manifest

Every plugin requires a \`plugin.toml\` manifest at its root:

\`\`\`toml
[plugin]
name = "my-plugin"
version = "0.1.0"
author = "Your Name <you@example.com>"
description = "A brief description of what this plugin does."
license = "MIT"
min_crossterm_version = "1.0.0"

[permissions]
terminal_read = true
terminal_write = false
ssh_metadata = true
filesystem_read = false
network_outbound = false
\`\`\`

## Lifecycle Hooks

| Hook | Trigger | Arguments |
|------|---------|-----------|
| \`on_connect\` | SSH/terminal session established | \`session_id\`, \`host\`, \`port\` |
| \`on_disconnect\` | Session closed or connection lost | \`session_id\`, \`reason\` |
| \`on_output_line\` | Each line of terminal output | \`session_id\`, \`line\` |
| \`on_command\` | User executes a registered command | \`command_name\`, \`args\` |
| \`on_tab_open\` | A new tab is opened | \`tab_id\`, \`session_id\` |
| \`on_tab_close\` | A tab is closed | \`tab_id\` |

Hooks are invoked asynchronously. If a hook returns an error, CrossTerm logs the failure and continues without crashing.

## Permission Model

| Capability | Grants |
|------------|--------|
| \`terminal:read\` | Read terminal output streams |
| \`terminal:write\` | Write input to terminal sessions |
| \`ssh:metadata\` | Access SSH session metadata (host, user, port) |
| \`filesystem:read\` | Read files from the local filesystem (scoped paths) |
| \`network:outbound\` | Make HTTP/TCP requests to external services |

Capabilities follow the principle of least privilege. Plugins cannot escalate permissions after installation — manifest changes require user re-approval.

## Example Plugin

A minimal Rust plugin that logs SSH commands:

\`\`\`rust
use crossterm_plugin_sdk::{Plugin, HookResult, SessionInfo};

pub struct CommandLogger;

impl Plugin for CommandLogger {
    fn name(&self) -> &str { "command-logger" }

    fn on_connect(&mut self, session: &SessionInfo) -> HookResult {
        log::info!("Connected to {}@{}:{}", session.user, session.host, session.port);
        HookResult::Ok
    }

    fn on_output_line(&mut self, session_id: &str, line: &str) -> HookResult {
        if line.starts_with('$') || line.starts_with('#') {
            log::info!("[{}] cmd: {}", session_id, line);
        }
        HookResult::Ok
    }

    fn on_disconnect(&mut self, session_id: &str, reason: &str) -> HookResult {
        log::info!("Disconnected {}: {}", session_id, reason);
        HookResult::Ok
    }
}

crossterm_plugin_sdk::export_plugin!(CommandLogger);
\`\`\`

## Plugin Cookbook

### Syntax Highlighter

Apply ANSI color codes to recognized keywords in terminal output:

\`\`\`rust
fn on_output_line(&mut self, _session_id: &str, line: &str) -> HookResult {
    let highlighted = line
        .replace("ERROR", "\\x1b[31mERROR\\x1b[0m")
        .replace("WARN",  "\\x1b[33mWARN\\x1b[0m")
        .replace("OK",    "\\x1b[32mOK\\x1b[0m");
    HookResult::Replace(highlighted)
}
\`\`\`

### Session Metrics Dashboard

Track connection durations and byte counts, exposed via a sidebar panel:

\`\`\`rust
fn on_connect(&mut self, session: &SessionInfo) -> HookResult {
    self.sessions.insert(session.id.clone(), Instant::now());
    HookResult::Ok
}

fn on_disconnect(&mut self, session_id: &str, _reason: &str) -> HookResult {
    if let Some(start) = self.sessions.remove(session_id) {
        log::info!("Session {} lasted {:?}", session_id, start.elapsed());
    }
    HookResult::Ok
}
\`\`\`

## Building & Testing

Build your plugin targeting WASI:

\`\`\`bash
cargo build --target wasm32-wasi --release
\`\`\`

Run the test harness:

\`\`\`bash
cargo install crossterm-plugin-test
crossterm-plugin-test ./target/wasm32-wasi/release/my_plugin.wasm
\`\`\`

For development iteration, use plugin dev mode:

\`\`\`bash
crossterm --plugin-dev ./path/to/plugin.wasm
\`\`\`

## Distribution

1. **Package** — Run \`crossterm-plugin pack\` to create a \`.ctplugin\` archive.
2. **Verify** — The packer runs automated security checks.
3. **Submit** — Upload via \`crossterm-plugin publish\` or through the web portal.
4. **Review** — Plugins requesting sensitive permissions undergo manual review before listing.

Users install plugins from **Settings → Plugins → Browse Registry** or via the command palette.`,
  },
];

/** All help articles sorted by the `order` field for display in the Help panel. */
export const helpArticles: HelpArticle[] = [...rawArticles].sort((a, b) => a.order - b.order);

/**
 * Full-text search across article titles, body content, and keyword tags.
 * Returns all articles when the query is blank (used to populate the initial list).
 * Case-insensitive; no stemming — a simple substring match is sufficient for
 * the volume of content and avoids a dependency on a search library.
 */
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

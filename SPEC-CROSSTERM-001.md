# CrossTerm — Cross-Platform Terminal Emulator & Remote Access Suite

## Specification Document

| Field              | Value                                      |
|--------------------|---------------------------------------------|
| Spec ID            | SPEC-CROSSTERM-001                          |
| Version            | 1.0                                         |
| Status             | Draft                                       |
| Target Audience    | Coding LLM / Development Team               |
| Comparable Product | MobaXterm, Termius, Royal TS, mRemoteNG     |

---

## 1. Executive Summary

Build **CrossTerm**, a cross-platform terminal emulator and remote access suite that consolidates SSH, RDP, VNC, serial console, SFTP, and cloud CLI management into a single application. The solution must run natively on Windows, macOS, Linux, and Android, with a unified codebase where feasible. It must support multi-user profiles, encrypted credential vaults, session persistence, and first-class integration with AWS, Azure, and GCP cloud environments.

The product targets IT professionals, DevOps engineers, system administrators, and enterprise architects who manage heterogeneous infrastructure across on-premises and multi-cloud environments.

---

## 2. Platform Targets & Technology Stack

### 2.1 Supported Platforms

| Platform | Minimum Version              | Distribution Format           |
|----------|------------------------------|-------------------------------|
| Windows  | Windows 10 21H2+            | MSI, MSIX, portable ZIP       |
| macOS    | macOS 12 Monterey+          | DMG, Homebrew cask             |
| Linux    | Ubuntu 22.04+ / Fedora 38+  | AppImage, Flatpak, DEB, RPM   |
| Android  | Android 11 (API 30)+        | APK, Google Play Store         |

### 2.2 Recommended Technology Stack

Use the following stack unless a clearly superior alternative exists for a specific subsystem. Justify any deviation.

| Layer                  | Technology                                                              |
|------------------------|-------------------------------------------------------------------------|
| Desktop UI Framework   | **Tauri 2.x** (Rust backend + WebView frontend)                        |
| Frontend               | **React 18+** with TypeScript, TailwindCSS                             |
| Terminal Renderer      | **xterm.js** (GPU-accelerated via WebGL addon)                         |
| Android                | Tauri mobile target (Android) OR Kotlin Compose with shared Rust core  |
| Cryptography           | **ring** (Rust) or **libsodium** via FFI                               |
| SSH Library            | **russh** (pure Rust SSH2) or **libssh2** via FFI                      |
| RDP Client             | **FreeRDP** embedded via FFI / subprocess                              |
| VNC Client             | **libvncclient** via FFI or pure-Rust VNC implementation               |
| Serial Console         | **serialport-rs** (Rust)                                               |
| Database (local)       | **SQLite** via **rusqlite** (encrypted with SQLCipher)                  |
| Cloud SDKs             | AWS SDK for Rust, Azure CLI wrapper, GCP gcloud CLI wrapper             |
| Auto-Update            | Tauri built-in updater (desktop), Play Store (Android)                  |
| Plugin System          | WASM-based plugin runtime (wasmtime)                                   |

### 2.3 Build & CI/CD

Use a monorepo structure. Provide GitHub Actions workflows for:

- Cross-compilation for all four platforms.
- Automated signing (Windows: Authenticode, macOS: notarisation, Android: APK signing).
- Release artefact generation with checksums.

---

## 3. User Profile & Identity Management

### 3.1 Local User Profiles

The application must support multiple local user profiles on a single installation, each fully isolated.

Each profile contains:

- A unique profile name and optional avatar.
- An independent encrypted credential vault (see §4).
- A separate set of saved sessions, groups, and folders.
- Independent application settings (theme, font, keybindings, default shell).
- A session history log (opt-in, encrypted).

Profile switching must not require restarting the application. An active profile indicator must always be visible in the UI chrome.

### 3.2 Profile Authentication

Each profile must be protected by one of the following mechanisms (user's choice during profile creation):

- **Master password** — derived via Argon2id with a minimum 128-bit salt.
- **Biometric unlock** — platform-native (Windows Hello, macOS Touch ID, Android fingerprint).
- **Hardware key** — FIDO2/WebAuthn via USB or NFC (desktop only).
- **OS credential store delegation** — Windows Credential Manager, macOS Keychain, Linux Secret Service (libsecret/kwallet).

On first launch, the application creates a "Default" profile with mandatory password setup.

### 3.3 Profile Import / Export

- Export a profile as an encrypted archive (AES-256-GCM, password-protected `.crossterm-profile` file).
- Import profiles from the archive format.
- Import sessions from MobaXterm (`.mxtsessions`), PuTTY (registry export), Termius JSON export, and Royal TS `.rtsz` files.

### 3.4 Profile Sync (Optional / Phase 2)

- End-to-end encrypted sync via a user-provided backend (WebDAV, S3-compatible, or Git repository).
- The sync service must never have access to plaintext credentials.
- Conflict resolution strategy: last-write-wins with manual merge UI for session definitions.

---

## 4. Credential Vault

### 4.1 Architecture

The credential vault is a SQLCipher-encrypted SQLite database, one per profile. The encryption key is derived from the profile's master password using Argon2id (memory cost ≥ 64 MiB, iterations ≥ 3, parallelism = 4).

### 4.2 Stored Credential Types

| Type                 | Fields                                                                 |
|----------------------|------------------------------------------------------------------------|
| Password             | username, password, domain (optional)                                  |
| SSH Key              | private key (PEM/OpenSSH), passphrase, associated public key           |
| Certificate          | X.509 client cert + private key (PEM or PKCS#12)                       |
| API Token            | provider, token string, expiry date                                    |
| Cloud Credential     | provider (AWS/Azure/GCP), access key, secret key, region, profile name |
| TOTP Seed            | secret, issuer, digits, period                                         |

### 4.3 Vault Behaviour

- Credentials are never written to disk in plaintext under any circumstances.
- The vault locks automatically after a configurable idle timeout (default: 15 minutes).
- Clipboard auto-clear after 30 seconds when a password is copied.
- A credential can be linked to one or more sessions; deleting a credential prompts the user about affected sessions.

---

## 5. Session Management

### 5.1 Session Types

The application must support the following session (connection) types:

| Session Type        | Protocol / Mechanism         | Key Capabilities                                                                                       |
|---------------------|------------------------------|--------------------------------------------------------------------------------------------------------|
| SSH Terminal        | SSH2                         | Shell, exec, port forwarding (local/remote/dynamic SOCKS5), agent forwarding, jump hosts (ProxyJump)   |
| SFTP Browser        | SFTP over SSH2               | Graphical dual-pane file browser, drag-and-drop, resume, queue, sync folders                           |
| SCP Transfer        | SCP over SSH2                | Quick file push/pull from session context menu                                                         |
| RDP                 | RDP (FreeRDP)                | Multi-monitor, clipboard sync, drive/printer redirection, NLA, gateway support                         |
| VNC                 | RFB (VNC)                    | Multiple encodings (Tight, ZRLE, Raw), clipboard, scaling, view-only mode                              |
| Telnet              | Telnet                       | Legacy device support, ANSI colour                                                                     |
| Serial Console      | RS-232 / USB-Serial          | Configurable baud, data bits, stop bits, parity, flow control                                          |
| Local Shell         | OS shell                     | Spawns the user's default shell (PowerShell/cmd, bash/zsh, sh)                                         |
| WSL Shell           | Windows only                 | Launches a named WSL distribution                                                                      |
| Cloud Shell         | AWS/Azure/GCP                | Opens the provider's managed cloud shell environment (see §8)                                          |
| Web Console (iframe)| HTTPS                        | Embeds a web-based management console (e.g., router web UI) in a tab                                  |
| Kubernetes Exec     | kubectl exec                 | Shell into a pod, namespace/context selection                                                          |
| Docker Exec         | docker exec                  | Shell into a running container, container picker                                                       |

### 5.2 Session Definition Schema

Every session is stored as a structured record with at least the following fields:

```
session:
  id: UUID
  name: string (user-defined display name)
  type: enum (from §5.1)
  group: string (folder path, e.g., "Production/EU-West")
  tags: string[] (freeform labels)
  icon: string (optional, emoji or custom icon reference)
  color_label: string (optional, hex colour for visual grouping)
  credential_ref: UUID (reference to vault entry)
  connection:
    host: string
    port: integer
    protocol_options: map (type-specific, see below)
  startup_script: string (optional, commands to run on connect)
  environment_variables: map<string, string>
  notes: string (freeform markdown, stored encrypted)
  created_at: datetime
  updated_at: datetime
  last_connected_at: datetime (nullable)
  auto_reconnect: boolean (default: true)
  keep_alive_interval_seconds: integer (default: 60)
```

### 5.3 Session Organisation

- Sessions are organised in a tree of **folders** (unlimited depth).
- Sessions can also be **tagged** with freeform labels for cross-folder filtering.
- A **Favourites** bar provides one-click access to pinned sessions.
- A **Recent** list shows the last 25 connected sessions with timestamps.
- Full-text **search** across session names, hostnames, tags, and notes.

### 5.4 Session Groups & Bulk Operations

- "Connect all in folder" — opens every session in a folder simultaneously.
- "Send command to all" — broadcast a command to all open terminal sessions (with confirmation safeguard).
- "Export folder" — exports a folder of sessions as an encrypted or plaintext JSON/YAML file.

---

## 6. Terminal Emulator Core

### 6.1 Emulation

- Full **xterm-256color** and **truecolor (24-bit)** support.
- ANSI, VT100, VT220, VT340 (Sixel graphics) escape sequence support.
- Kitty graphics protocol support for inline image display.
- Unicode 15.1+ with full-width CJK, combining character, and emoji rendering.
- Bidirectional text (BiDi) for RTL language support.

### 6.2 Rendering

- GPU-accelerated rendering via WebGL (xterm.js WebGL addon).
- Target ≥ 120 FPS on a 4K display with ≤ 5 ms input latency.
- Configurable font (default: a bundled Nerd Font variant of JetBrains Mono).
- Font ligature support (configurable on/off).
- Adjustable line height, letter spacing, and cursor style (block, underline, bar, blinking variants).

### 6.3 Scrollback & Search

- Configurable scrollback buffer (default: 10,000 lines, max: unlimited with disk-backed virtualisation).
- Incremental regex-capable search within scrollback (Ctrl+Shift+F).
- Clickable URLs with modifier-key override (Ctrl+click to open, plain click to position cursor).
- Semantic URL detection (file paths, IP addresses, error codes, stack traces).

### 6.4 Input & Productivity

- Bracketed paste mode (enabled by default; disables for legacy hosts).
- Multi-line paste warning (prompts when pasting content containing newlines into a shell, configurable threshold).
- Autocomplete suggestions from shell history and known hostnames (opt-in, local only).
- **Snippets manager**: save, tag, and quickly insert reusable command fragments. Snippets support `{{placeholder}}` template variables that prompt the user on insertion.
- **Command palette** (Ctrl+Shift+P): fuzzy-searchable list of all application actions.

### 6.5 Session Logging

- Log session output to file (plaintext, HTML with ANSI colours, or raw binary).
- Configurable log rotation (by size or date).
- Timestamped logging mode (prepends a timestamp to each received line).
- Logs are stored under the profile's data directory; location is configurable.

---

## 7. Remote Desktop & VNC Viewer

### 7.1 RDP Client

Built on FreeRDP. Must support:

- Network Level Authentication (NLA / CredSSP).
- TLS 1.2+ transport encryption.
- Multi-monitor span and multi-monitor select.
- RemoteFX / GFX progressive codec for bandwidth-constrained links.
- Clipboard synchronisation (text, files, images).
- Drive redirection (map local folders into the remote session).
- Printer redirection.
- Audio redirection (playback and recording).
- Smart card passthrough.
- RD Gateway / RD Web Access connections.
- Dynamic resolution resize (resize the remote desktop when the tab/window resizes).
- Session recording to video file (MP4/WebM, optional, must warn user).

### 7.2 VNC Client

Must support:

- RFB protocol versions 3.3, 3.7, 3.8.
- Security types: None, VNC Authentication, VeNCrypt (TLS + x509).
- Encodings: Raw, CopyRect, RRE, Hextile, ZRLE, Tight, Cursor pseudo-encoding.
- Clipboard sync (Latin-1 and UTF-8 extended clipboard).
- Scaling modes: fit-to-window, scroll, 1:1 pixel.
- View-only mode toggle.
- Screenshot capture (PNG, clipboard).

### 7.3 Tabbed & Tiled Viewing

- Both RDP and VNC sessions open as tabs alongside terminal tabs.
- Tabs can be dragged to tile 2 or 4 sessions in a grid within the main window.
- Tabs can be detached into standalone windows.

---

## 8. Cloud Provider Integration

### 8.1 General Architecture

Cloud integrations rely on the respective provider's CLI tools and SDKs. CrossTerm must detect whether the CLI is installed, prompt the user to install it if missing, and manage named CLI profiles.

### 8.2 Amazon Web Services (AWS)

| Feature                     | Detail                                                                              |
|-----------------------------|--------------------------------------------------------------------------------------|
| AWS CLI Profile Management  | List, create, switch between named profiles in `~/.aws/credentials` and `config`.    |
| SSO Login                   | Trigger `aws sso login` flow, cache tokens, auto-refresh.                            |
| EC2 Instance Browser        | List instances by region, display name/ID/state/IP. One-click SSH or SSM connect.    |
| SSM Session Manager         | Start a Session Manager shell session without needing inbound SSH (via `aws ssm start-session`). |
| S3 Browser                  | Dual-pane graphical S3 bucket browser (list, upload, download, presigned URLs).      |
| CloudWatch Logs Tail        | Stream log group/log stream output into a terminal tab in real time.                 |
| ECS Exec                    | Shell into a running ECS Fargate/EC2 task.                                           |
| Lambda Invoke               | Invoke a function with a JSON payload, display result.                               |
| Cost Dashboard (read-only)  | Display current-month spend summary via Cost Explorer API.                           |

### 8.3 Microsoft Azure

| Feature                     | Detail                                                                              |
|-----------------------------|--------------------------------------------------------------------------------------|
| Azure CLI Profile Mgmt     | Manage subscriptions and accounts (`az account list/set`).                           |
| Azure AD / Entra Login     | Trigger `az login`, device code flow, and managed identity.                          |
| VM Browser                 | List VMs by subscription/resource group. One-click SSH, Serial Console, or RDP.      |
| Azure Bastion              | Connect through Bastion (via `az network bastion ssh/rdp`).                          |
| Azure Cloud Shell          | Embed Azure Cloud Shell (Bash or PowerShell) in a terminal tab via websocket relay.  |
| Storage Explorer           | Browse Blob containers, file shares. Upload, download, generate SAS tokens.         |
| AKS kubectl Integration    | Fetch kubeconfig, switch cluster contexts, launch kubectl exec.                      |
| Log Analytics Query        | Run KQL queries against a workspace, display results in a table view.               |

### 8.4 Google Cloud Platform (GCP)

| Feature                     | Detail                                                                              |
|-----------------------------|--------------------------------------------------------------------------------------|
| gcloud Profile Management  | Manage named configurations (`gcloud config configurations`).                        |
| IAP Tunnel SSH             | SSH to VMs through Identity-Aware Proxy (`gcloud compute ssh --tunnel-through-iap`). |
| Compute Instance Browser   | List VMs by project/zone. One-click SSH or serial console.                           |
| GCS Browser                | Browse buckets and objects. Upload, download, manage ACLs.                           |
| Cloud Shell                | Embed GCP Cloud Shell in a terminal tab.                                             |
| GKE kubectl Integration    | Fetch credentials, switch contexts, exec into pods.                                  |
| Cloud Logging Tail         | Stream logs from a resource via `gcloud logging tail`.                               |

### 8.5 Multi-Cloud Dashboard

Provide a unified "Cloud Assets" sidebar panel that merges resources from all configured providers into a single tree view, grouped by provider and then by resource type (Compute, Storage, Kubernetes, Serverless). Each node must offer right-click context actions appropriate to its type.

---

## 9. Local Network Features

### 9.1 Network Scanner

- Scan a CIDR range or local subnet for live hosts (ICMP ping sweep + TCP SYN on common ports).
- Display results in a sortable table: IP, hostname (reverse DNS), MAC address (local subnet only), open ports, OS fingerprint guess.
- One-click "Connect" from scan results: auto-detect the best session type (SSH if 22 open, RDP if 3389 open, VNC if 5900 open, HTTP if 80/443 open).
- Save scan results to the session library as a folder of sessions.

### 9.2 Wake-on-LAN

- Send WOL magic packets to saved MAC addresses.
- WOL targets can be stored per-session or in a standalone list.

### 9.3 Port Forwarding Manager

- Standalone UI for managing SSH tunnels independent of an interactive shell session.
- Define persistent tunnel rules (local, remote, dynamic/SOCKS5) that auto-establish on application start-up.
- Visual indicator (tray icon badge) when tunnels are active.

### 9.4 Built-in TFTP / HTTP File Server

- One-click temporary TFTP or HTTP file server rooted at a chosen local directory.
- Useful for firmware uploads to network devices and quick file sharing.
- Server auto-stops after a configurable timeout or on application exit.

---

## 10. UX Specification — Cross-Platform Design System

This section defines the complete user experience specification for CrossTerm across all four platforms. Each subsection contains a **shared design intent** followed by **platform-specific adaptation tables** that override or extend the shared behaviour. When implementing for a given platform, the platform column is authoritative. If a platform cell reads "Shared", apply the shared behaviour unchanged.

---

### 10.1 Design Principles

All platform implementations must adhere to these principles, in priority order:

1. **Terminal first** — The terminal canvas is the hero surface. Every pixel of chrome must justify its existence; if it doesn't aid the active session, it should be hideable.
2. **Zero-surprise navigation** — Moving between sessions, profiles, and tools must never cause the user to lose their place. State is preserved across tab switches, pane splits, and even app restarts.
3. **Progressive disclosure** — Expose basic connect-and-type functionality by default. Advanced features (macros, automation, cloud asset browsers) reveal themselves through discoverable affordances, not upfront complexity.
4. **Muscle-memory portability** — Default keybindings, gestures, and menu structures should feel native on each platform so that a user switching from Windows to macOS (or vice versa) is never disoriented.
5. **Offline-capable** — Every feature that does not inherently require a network (session management, vault, settings, snippets, macros) must work fully offline.

---

### 10.2 Design Tokens & Visual Language

Implement a design token system so that themes, spacing, and typography are centrally defined and consumed by all UI components. Every visual property below must be a named token, not a hardcoded value.

#### 10.2.1 Colour System

Define the following semantic colour token categories. Each theme (see §10.9) provides concrete values for every token.

| Token Category         | Tokens (minimum set)                                                                                       |
|------------------------|------------------------------------------------------------------------------------------------------------|
| Surface                | `surface-primary`, `surface-secondary`, `surface-elevated`, `surface-sunken`, `surface-overlay`            |
| Text                   | `text-primary`, `text-secondary`, `text-disabled`, `text-inverse`, `text-link`                             |
| Border                 | `border-default`, `border-subtle`, `border-strong`, `border-focus`                                         |
| Interactive            | `interactive-default`, `interactive-hover`, `interactive-active`, `interactive-disabled`                    |
| Status                 | `status-connected` (green), `status-disconnected` (red), `status-connecting` (amber), `status-idle` (grey) |
| Terminal               | ANSI 0–15 + foreground, background, cursor, selection (16+4 tokens per theme)                              |
| Accent                 | `accent-primary`, `accent-secondary` (used for focused tab indicator, active sidebar icon, badges)         |

#### 10.2.2 Typography

| Token               | Desktop Default                         | Android Default                     |
|----------------------|-----------------------------------------|-------------------------------------|
| `font-ui`            | Inter, system-ui, sans-serif            | Roboto, system-ui, sans-serif       |
| `font-mono`          | JetBrains Mono NF (bundled Nerd Font)   | JetBrains Mono NF                   |
| `font-size-xs`       | 11 px                                   | 12 sp                               |
| `font-size-sm`       | 12 px                                   | 14 sp                               |
| `font-size-md`       | 13 px                                   | 16 sp                               |
| `font-size-lg`       | 15 px                                   | 18 sp                               |
| `font-size-xl`       | 18 px                                   | 22 sp                               |
| `font-size-terminal` | 14 px (user-configurable)               | 13 sp (user-configurable)           |
| `line-height-terminal`| 1.2 (user-configurable)                | 1.25 (user-configurable)            |

#### 10.2.3 Spacing & Layout Grid

Use a **4 px base unit**. All padding, margin, and gap values must be multiples of 4 px. Define tokens `space-1` (4 px) through `space-16` (64 px).

| Token       | Value | Common Usage                              |
|-------------|-------|-------------------------------------------|
| `space-1`   | 4 px  | Inline icon-to-text gap                   |
| `space-2`   | 8 px  | Compact list item padding                 |
| `space-3`   | 12 px | Standard input field padding              |
| `space-4`   | 16 px | Card/panel internal padding               |
| `space-6`   | 24 px | Section spacing                           |
| `space-8`   | 32 px | Major layout gutters                      |
| `space-12`  | 48 px | Android minimum touch target height       |

#### 10.2.4 Elevation & Depth

| Level | Shadow / Treatment            | Usage                                              |
|-------|-------------------------------|-----------------------------------------------------|
| 0     | None                          | Inset / sunken panels (SFTP browser, terminal)      |
| 1     | Subtle shadow (0 1px 3px)     | Cards, sidebar, tab bar                             |
| 2     | Medium shadow (0 4px 12px)    | Dropdowns, popovers, floating toolbars              |
| 3     | Heavy shadow (0 8px 24px)     | Modal dialogs, command palette overlay               |

#### 10.2.5 Motion & Animation

| Property           | Value              | Notes                                                 |
|--------------------|--------------------|-------------------------------------------------------|
| Duration — micro   | 100 ms             | Button press, toggle switch                           |
| Duration — short   | 150 ms             | Tab switch, dropdown open                             |
| Duration — medium  | 250 ms             | Sidebar expand/collapse, panel slide                  |
| Duration — long    | 400 ms             | Page transitions, modal enter/exit                    |
| Easing — default   | cubic-bezier(0.4, 0, 0.2, 1) | Standard Material-style ease                |
| Easing — decelerate| cubic-bezier(0, 0, 0.2, 1)   | Elements entering the viewport              |
| Easing — accelerate| cubic-bezier(0.4, 0, 1, 1)   | Elements leaving the viewport               |
| Reduce motion      | Honour `prefers-reduced-motion`; collapse all durations to 0 ms  |            |

All animations must be interruptible. No animation should block user input.

#### 10.2.6 Iconography

Use **Lucide** as the primary icon set (MIT licensed, consistent stroke weight). Supplement with custom icons only for session-type indicators (SSH, RDP, VNC, Serial, Cloud Shell) and cloud provider logos. Icons render at 16 px (desktop) / 24 dp (Android) by default; sidebar uses 20 px / 28 dp.

---

### 10.3 Application Shell — Shared Layout

The application shell consists of five regions. The relative sizing, visibility, and interaction model for each region varies by platform (see §10.4–10.7).

```
┌─────────────────────────────────────────────────────────────────┐
│ A  TITLE BAR / WINDOW CONTROLS                                  │
├────────┬────────────────────────────────────────────────────────┤
│        │ B  TAB BAR                                             │
│        │   [ ● SSH: prod-web-1 ][ ◉ RDP: dc01 ][ + New ]      │
│ C      ├────────────────────────────────────────────────────────┤
│        │                                                        │
│ SIDE   │ D  SESSION CANVAS                                      │
│ BAR    │    (Terminal / RDP / VNC / SFTP / Cloud Shell)          │
│        │    ┌──────────────────┬─────────────────────┐          │
│        │    │  Split Pane L    │   Split Pane R       │          │
│        │    │                  │                      │          │
│        │    └──────────────────┴─────────────────────┘          │
│        ├────────────────────────────────────────────────────────┤
│        │ E  BOTTOM PANEL (toggleable)                           │
│        │    SFTP Browser │ Snippet Manager │ Audit Log          │
├────────┴────────────────────────────────────────────────────────┤
│ F  STATUS BAR                                                    │
└─────────────────────────────────────────────────────────────────┘
```

**Region Descriptions:**

| Region | Name           | Purpose                                                                                                   |
|--------|----------------|-----------------------------------------------------------------------------------------------------------|
| A      | Title Bar      | Application title, profile indicator, window controls. Custom-drawn on Windows/Linux; native on macOS.    |
| B      | Tab Bar        | Horizontally scrollable list of open sessions. Each tab shows session-type icon, name, and connection status dot. A `+` button opens the new-session dialog. Tabs are drag-reorderable and detachable. |
| C      | Sidebar        | Multi-mode panel that switches between: Sessions tree, Cloud Assets, Snippets, Port Forwards, Network Scanner results. Mode selection via an icon rail on the sidebar's left edge. |
| D      | Session Canvas | The primary content area. Renders the active session's terminal, remote desktop, file browser, or cloud shell. Supports recursive horizontal/vertical splits. |
| E      | Bottom Panel   | Toggleable (Ctrl+J) auxiliary panel. Hosts the SFTP browser, snippet manager, audit log viewer, or search results. Resizable via drag handle. Default height: 30% of canvas. |
| F      | Status Bar     | Single row showing: active profile name, connection state + latency, encoding, terminal dimensions (cols × rows), active tunnels count, notification badge. Clickable segments open relevant dialogs. |

---

### 10.4 Platform Adaptation — Windows

#### 10.4.1 Window Chrome

| Aspect                  | Specification                                                                                 |
|-------------------------|-----------------------------------------------------------------------------------------------|
| Title bar               | Custom-drawn (Tauri `decorations: false`). Embed the profile switcher dropdown and a search field directly in the title bar to the left of the window controls. Background colour follows `surface-primary`. |
| Window controls         | Render Windows-style minimize / maximize / close buttons in the top-right corner. Support snap layouts (Win+Z) by responding to `WM_GETMINMAXINFO`. |
| Taskbar integration     | Show a Jump List with: recent sessions (last 10), "New SSH Session", "New Local Shell", and "Open Vault". Pin-to-taskbar must work. |
| System tray             | Minimize-to-tray (configurable). Tray icon shows a badge for active tunnels and notifications. Right-click menu: Quick Connect, active sessions list, Reconnect All, Quit. |
| Multi-window            | Support detaching tabs into separate windows. Each window is an independent OS window but shares the same Tauri app process and profile state. |
| DPI scaling             | Respond to per-monitor DPI. Terminal font size adjusts automatically; user can override. All icons render as SVG to remain crisp. |
| Native dialogs          | Use the Windows file picker for import/export dialogs (Tauri `dialog` API). |

#### 10.4.2 Sidebar — Windows

| Aspect              | Specification                                                                                     |
|----------------------|---------------------------------------------------------------------------------------------------|
| Default state        | Expanded (240 px wide). Pinned by default.                                                        |
| Collapse behaviour   | Collapses to a 48 px icon rail on toggle (Ctrl+B) or when window width < 900 px. Hovering the icon rail temporarily expands it with a 250 ms slide animation. |
| Resize               | Drag the sidebar's right edge. Min: 180 px, Max: 400 px.                                         |
| Content modes         | Icon rail shows: Sessions (tree icon), Cloud (cloud icon), Snippets (code icon), Tunnels (lock icon), Network (radar icon). Clicking an icon switches the sidebar's content panel. Active mode is indicated by an accent-coloured left-edge bar on the icon. |

#### 10.4.3 Tab Bar — Windows

| Aspect               | Specification                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------|
| Position              | Below the title bar, spanning the width minus the sidebar.                                       |
| Tab width             | Flexible width, min 120 px, max 240 px. When tabs overflow, show left/right scroll arrows and a "…" dropdown listing all tabs. |
| Tab content           | Session-type icon (16 px) + session name (truncated with ellipsis) + status dot (8 px, coloured per `status-*` tokens) + close button (appears on hover). |
| Drag and drop         | Tabs reorder by drag within the tab bar. Dragging a tab outside the tab bar detaches it into a new window. Dropping a tab onto a split-pane divider docks it into that pane. |
| Middle-click          | Closes the tab (with confirmation if session is active).                                          |
| New tab button        | `+` button at the right end of the tab bar. Click opens a dropdown: Local Shell, SSH, RDP, VNC, Serial, Cloud Shell, SFTP, from Saved Session. Ctrl+T opens the dropdown focused. |
| Pinned tabs           | Right-click → "Pin Tab" shrinks the tab to icon-only (32 px) and locks it to the left of the tab bar. Pinned tabs survive app restart. |
| Context menu          | Right-click a tab: Rename, Duplicate, Pin/Unpin, Split Right, Split Down, Move to New Window, Reconnect, Close, Close Others, Close All. |

#### 10.4.4 Keyboard & Input — Windows

| Aspect                | Specification                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------|
| Default modifier      | `Ctrl` for application shortcuts. Terminal passthrough uses `Ctrl+Shift` prefix to avoid conflicts (e.g., `Ctrl+Shift+C` = copy from terminal, `Ctrl+C` = SIGINT). |
| Command palette       | `Ctrl+Shift+P` — opens a full-width overlay at the top of the window, fuzzy-searchable.          |
| Quick connect         | `Ctrl+Shift+N` — opens a minimal dialog: type `ssh user@host`, `rdp host`, or `vnc host:display` and press Enter. Supports saved-session name autocomplete. |
| Tab navigation        | `Ctrl+Tab` / `Ctrl+Shift+Tab` cycles tabs. `Ctrl+1`–`Ctrl+9` jumps to tab by position.          |
| Pane navigation       | `Alt+Arrow` moves focus between split panes. `Alt+Shift+Arrow` resizes the active pane.          |
| Paste safeguard       | Multi-line paste: show a confirmation dialog with a preview and a "Paste as single line" option. Configurable threshold (default: 2 lines). |
| PowerShell integration| Register CrossTerm as a selectable terminal profile in Windows Terminal settings export. |

#### 10.4.5 Context Menus & Right-Click — Windows

Right-click on the terminal canvas shows: Copy, Paste, Select All, Search Selection on Web, Open URL (if cursor is on a detected URL), Run as Snippet, separator, Clear Scrollback, Session Settings.

Register a Windows Explorer shell extension: right-click a folder → "Open CrossTerm Here" launches a local shell tab with `cwd` set to that folder.

---

### 10.5 Platform Adaptation — macOS

#### 10.5.1 Window Chrome

| Aspect                  | Specification                                                                                 |
|-------------------------|-----------------------------------------------------------------------------------------------|
| Title bar               | **Native macOS title bar** with traffic-light buttons on the left. Use a toolbar-style title bar (`titleBarStyle: 'hiddenInset'` equivalent in Tauri) to embed the tab bar directly into the title bar area for a compact look. The profile switcher sits as a small dropdown at the far right of the title bar. |
| Full screen             | Support native macOS full-screen (green traffic light) and Split View (drag to screen edge). The tab bar remains visible in full screen. |
| Dock integration        | Dock icon badge shows a count of disconnected sessions requiring attention. Right-click Dock icon: recent sessions, New SSH, New Local Shell. |
| Touch Bar (legacy)      | For MacBook Pro models with Touch Bar: display the active tab's session name, a quick-connect field, and favourite session shortcuts. |
| Native dialogs          | Use macOS native file pickers (NSOpenPanel / NSSavePanel) via Tauri. Alerts use macOS-style sheet dialogs attached to the window. |
| Accent colour           | Inherit the system accent colour (System Preferences → General) for focus rings, selection highlights, and the active tab indicator. |

#### 10.5.2 Sidebar — macOS

| Aspect              | Specification                                                                                     |
|----------------------|---------------------------------------------------------------------------------------------------|
| Default state        | Expanded (220 px). Visually styled like a macOS source list (translucent background with vibrancy/blur when supported). |
| Collapse behaviour   | Toggle via `Cmd+B` or the sidebar button in the toolbar. Collapses with a 200 ms slide. No icon rail; fully hidden when collapsed (macOS convention). Re-show via `Cmd+B` or swipe from left edge on trackpad. |
| Vibrancy             | Apply `NSVisualEffectView`-style backdrop blur to the sidebar (Tauri's `transparent` + CSS `backdrop-filter: blur(20px)`). |

#### 10.5.3 Tab Bar — macOS

| Aspect               | Specification                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------|
| Position              | Integrated into the title bar region (below the traffic lights, same row). This mimics Safari/Finder tab behaviour. |
| Tab style             | Rounded-rect tab shapes, slightly overlapping, matching macOS native tab appearance. Active tab is elevated (subtle shadow). |
| Tab close             | Hover reveals an × close button on the left side of the tab (macOS convention). |
| New tab               | `Cmd+T`. The `+` button sits at the right end of the tab bar.                                    |
| Drag detach           | Dragging a tab away from the tab bar creates a new native window (macOS native tab detach feel). |

#### 10.5.4 Keyboard & Input — macOS

| Aspect                | Specification                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------|
| Default modifier      | `Cmd` replaces `Ctrl` for all application shortcuts. `Cmd+C` copies from terminal (not SIGINT). `Ctrl+C` is passed through to the shell as SIGINT. This matches iTerm2 / Terminal.app behaviour. |
| Command palette       | `Cmd+Shift+P`.                                                                                   |
| Quick connect         | `Cmd+Shift+N`.                                                                                   |
| Tab navigation        | `Cmd+Shift+]` / `Cmd+Shift+[` for next/previous tab (Safari convention). `Cmd+1`–`Cmd+9` for positional. |
| Preferences           | `Cmd+,` opens Settings (macOS standard).                                                         |
| Trackpad gestures     | Three-finger swipe left/right cycles tabs. Pinch-to-zoom adjusts terminal font size (with Cmd+0 to reset). |
| Emoji & special input | `Ctrl+Cmd+Space` triggers the macOS Character Viewer for emoji/special characters.               |

#### 10.5.5 Menu Bar — macOS

Follow Apple Human Interface Guidelines for the menu bar structure:

| Menu         | Key Items                                                                                         |
|--------------|---------------------------------------------------------------------------------------------------|
| CrossTerm    | About, Preferences (Cmd+,), Check for Updates, Hide, Hide Others, Quit (Cmd+Q)                   |
| File         | New SSH, New Local Shell, New RDP, New VNC, Open Saved Session, Close Tab (Cmd+W), Close Window   |
| Edit         | Copy, Paste, Select All, Find (Cmd+F), Snippets                                                  |
| View         | Toggle Sidebar, Toggle Bottom Panel, Toggle Full Screen, Zoom In/Out, Actual Size                 |
| Sessions     | Connect All in Folder, Disconnect, Reconnect, Broadcast Input, Session Settings                   |
| Cloud        | AWS, Azure, GCP submenus (each: Browse Resources, Cloud Shell, CLI Profile)                       |
| Tools        | Network Scanner, Port Forwards, SSH Key Manager, Macros, SFTP Browser, Diff Viewer               |
| Window       | Minimize, Zoom, Tile, Bring All to Front, list of open windows/tabs                              |
| Help         | Keyboard Shortcuts (Cmd+/), Documentation, Release Notes, Report Issue                            |

---

### 10.6 Platform Adaptation — Linux

#### 10.6.1 Window Chrome

| Aspect                  | Specification                                                                                 |
|-------------------------|-----------------------------------------------------------------------------------------------|
| Title bar               | Custom-drawn by default (consistent look across DEs). Detect the running desktop environment (GNOME, KDE, XFCE, Sway/i3) and offer a setting to use the DE's native title bar decorations instead (`decorations: true` in Tauri). |
| CSD vs SSD              | Default to Client-Side Decorations (CSD) on GNOME/Wayland. On KDE or i3/Sway, default to Server-Side Decorations (SSD) with a config toggle. |
| Tray                    | Use `libappindicator` / StatusNotifierItem for the system tray icon. Gracefully degrade on tiling WMs (no tray → skip). |
| Wayland vs X11          | Ship with Wayland-native support (wl_surface). Fall back to XWayland only when required by FreeRDP or other X11-only dependencies. Terminal input (IME, clipboard) must work natively on both. |
| Tiling WM awareness     | Respond correctly to tiling WM layout commands (resize, move, focus). Do not fight tiling by overriding window geometry. When the window manager tiles the window, auto-collapse the sidebar to icon-rail mode if the resulting width < 900 px. |

#### 10.6.2 Sidebar — Linux

| Aspect              | Specification                                                                                     |
|----------------------|---------------------------------------------------------------------------------------------------|
| Default state        | Expanded (240 px), same as Windows.                                                               |
| Collapse             | `Ctrl+B`. Collapses to icon rail on narrow windows. On tiling WMs, detect if window is tiled to less than half-screen and auto-collapse. |
| Transparency         | No vibrancy/blur by default (unreliable across compositors). Offer a setting to enable `backdrop-filter` for users with a compatible compositor (KWin, Mutter with pipewire). |

#### 10.6.3 Keyboard & Input — Linux

| Aspect                | Specification                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------|
| Default modifier      | `Ctrl` (same as Windows defaults).                                                               |
| IME support           | Support IBus and Fcitx5 input methods for CJK input. Ensure composition windows position correctly relative to the terminal cursor. |
| Clipboard             | Support both `CLIPBOARD` (Ctrl+Shift+C) and `PRIMARY` (middle-click paste) X11/Wayland selections. Terminal mouse selection auto-populates PRIMARY; explicit copy goes to CLIPBOARD. |
| DE integration        | Register a `.desktop` file with `Terminal=false` and appropriate MIME types. Support `xdg-open` for URL handling. Register with `update-alternatives` as an x-terminal-emulator option (Debian-based). |

#### 10.6.4 File Manager Integration — Linux

Register a Nautilus/Nemo/Dolphin context-menu extension (or freedesktop Actions entry): right-click a folder → "Open CrossTerm Here". Provide install instructions in the settings UI for manual setup on unsupported file managers.

---

### 10.7 Platform Adaptation — Android

#### 10.7.1 Application Shell — Android

The Android layout is fundamentally restructured for touch interaction:

```
┌────────────────────────────────────────┐
│ A  TOP APP BAR                         │
│    ☰  CrossTerm    [profile] [search]  │
├────────────────────────────────────────┤
│                                        │
│ D  SESSION CANVAS (full-width)         │
│    Terminal / RDP / VNC / Cloud Shell   │
│                                        │
│                                        │
│                                        │
├────────────────────────────────────────┤
│ G  EXTRA-KEYS BAR                      │
│  [Esc][Tab][Ctrl][Alt][↑][↓][←][→][|] │
├────────────────────────────────────────┤
│    [ Soft Keyboard ]                   │
├────────────────────────────────────────┤
│ H  BOTTOM NAV BAR                      │
│  [Sessions] [Cloud] [Tools] [Settings] │
└────────────────────────────────────────┘
```

| Region | Name           | Description                                                                                          |
|--------|----------------|------------------------------------------------------------------------------------------------------|
| A      | Top App Bar    | Hamburger menu (opens the session drawer), app title showing active session name, profile avatar button, search icon. Collapses (scrolls up) when the user scrolls terminal output to maximise canvas space. |
| D      | Session Canvas | Full-width, full-height (minus bars). No split panes by default; split-pane mode available on tablets (screen width ≥ 600 dp) or landscape on phones. |
| G      | Extra-Keys Bar | A horizontally scrollable row of modifier and special keys not present on the Android soft keyboard. User-configurable: long-press a key to swap it for another from a picker. Ships with: Esc, Tab, Ctrl, Alt, ↑, ↓, ←, →, \|, /, ~, -. |
| H      | Bottom Nav Bar | Four destinations: Sessions (tree/list), Cloud (asset browser), Tools (scanner, tunnels, keys, macros), Settings. Shows a badge on Sessions when a connection drops. Hides when the keyboard is open to maximise terminal space. |

#### 10.7.2 Navigation Model — Android

| Action                  | Gesture / Control                                                                                |
|-------------------------|--------------------------------------------------------------------------------------------------|
| Open session drawer     | Tap hamburger ☰ or swipe right from the left edge.                                              |
| Switch tabs             | Swipe left/right on the top app bar area. Or tap a session in the drawer.                       |
| Close tab               | In the drawer, swipe a session entry left to reveal a "Disconnect" action (red). Swipe further to remove. |
| Quick connect           | Floating Action Button (FAB) in the session drawer's bottom-right corner. Tap → connect dialog with host, type picker, and credential selector. |
| Context menu            | Long-press on terminal canvas → Copy, Paste, Select All, Clear.                                |
| Select text             | Double-tap a word to select. Drag handles to extend selection (Android-native text selection). Then toolbar: Copy, Search Web, Run as Snippet. |
| Zoom                    | Pinch-to-zoom adjusts terminal font size live. Double-tap to reset to default size.             |
| RDP/VNC mouse modes     | Floating toolbar overlay with 3 modes: Touchpad (finger is relative pointer), Touch (tap = click at point), Direct (1:1 cursor tracking). The toolbar auto-hides after 3 seconds; tap anywhere to re-show. |

#### 10.7.3 Session Drawer — Android

The session drawer replaces the sidebar. It is a full-height navigation drawer from the left edge (Material Design 3 style).

| Section (top to bottom) | Content                                                                                            |
|--------------------------|----------------------------------------------------------------------------------------------------|
| Profile header           | Active profile avatar + name + unlock status. Tap to switch profiles.                             |
| Search bar               | Filter sessions by name, host, or tag.                                                            |
| Favourites               | Horizontally scrollable row of pinned session chips.                                              |
| Recent                   | Last 10 connected sessions with timestamps and status.                                            |
| Session tree             | Collapsible folder tree. Each session entry: icon, name, host, status dot. Tap to connect. Long-press for context menu (Edit, Duplicate, Delete, Move to Folder). |
| Footer                   | "Manage Sessions" link (opens full session management view). "Import" button.                     |

#### 10.7.4 Notifications & Background — Android

| Aspect                   | Specification                                                                                    |
|--------------------------|--------------------------------------------------------------------------------------------------|
| Foreground service       | When at least one SSH session is active, run a foreground service with a persistent notification showing: number of active sessions, a "Disconnect All" action, and a "Return to CrossTerm" tap target. This keeps sessions alive when the app is backgrounded. |
| Connection alerts        | Post a heads-up notification when a session disconnects unexpectedly. Tapping the notification opens the app to that session and triggers auto-reconnect. |
| Pattern match alerts     | Same as desktop (§11.8); delivered as Android notifications with the match snippet as body text. |
| Notification channel     | Register a dedicated Android notification channel ("CrossTerm Sessions") so users can configure alert behaviour at the OS level. |

#### 10.7.5 Tablet Adaptations (≥ 600 dp width)

| Aspect              | Specification                                                                                     |
|----------------------|---------------------------------------------------------------------------------------------------|
| Layout               | Switch from bottom-nav to a persistent left sidebar (narrow, icon + label) combined with the session drawer. The bottom nav bar is removed. |
| Split panes           | Enable horizontal split (two sessions side-by-side) by default. Draggable divider.               |
| Keyboard              | When a hardware keyboard is attached (Bluetooth or USB), hide the extra-keys bar and soft keyboard. Apply the full desktop keybinding set. Show a "Keyboard Mode: Desktop" indicator in the status bar. |
| Landscape             | On phones in landscape, stack the extra-keys bar vertically on the right edge (single column) to maximise horizontal terminal width. |

---

### 10.8 Session Canvas — Detailed Interaction Spec

This section defines interaction behaviour within the session canvas, shared across platforms unless a platform override is noted.

#### 10.8.1 Terminal Session Canvas

| Feature                  | Behaviour                                                                                       |
|--------------------------|-------------------------------------------------------------------------------------------------|
| Cursor                   | Block cursor (default), user-configurable to underline or bar. Blinks at 530 ms on / 530 ms off (configurable, or disabled). Colour: `cursor` token. |
| Selection                | Click-and-drag to select text. Double-click selects a word (using configurable word separators). Triple-click selects the entire line. Shift+click extends selection. Selected text is highlighted with `selection` token colour at 40% opacity. |
| Copy on select           | Configurable (default: off on desktop, on for Android). When enabled, any selection automatically copies to the clipboard. |
| URL detection            | Underline detected URLs on hover. Ctrl+click (Cmd+click on macOS) opens in the default browser. Show a tooltip with the full URL on hover. Customisable regex for URL patterns (e.g., to detect Jira ticket IDs as links). |
| Drag and drop            | Drop a file from the OS file manager onto the terminal → prompt: "Upload via SFTP?" (if SSH session) or "Paste file path?" (if local shell). |
| Right-click / long-press | Context menu: Copy, Paste, Select All, Search Selection, Open URL, Add to Snippets, separator, Session Settings, Clear Buffer. |
| Bell                     | Configurable: visual flash (invert colours for 100 ms), audio (system bell sound), notification, or disabled. Default: visual flash. |
| Bracketed paste          | Enabled by default. When the remote shell advertises bracketed paste mode, wrap pasted content in escape sequences. Display a yellow "Paste Mode" badge in the status bar during a bracketed paste. |

#### 10.8.2 RDP / VNC Session Canvas

| Feature                  | Behaviour                                                                                       |
|--------------------------|-------------------------------------------------------------------------------------------------|
| Scaling                  | Default: Fit to pane. Options: 1:1 pixel, fit to width, fit to height, percentage (50%–200%).   |
| Dynamic resize           | When the pane or window resizes, send a resolution-change request to the remote host (RDP: RAIL, VNC: SetDesktopSize). Debounce 300 ms. |
| Toolbar                  | Floating semi-transparent toolbar at the top-centre of the canvas (auto-hides after 3 seconds, re-show on mouse movement to top edge). Buttons: Disconnect, Full Screen, Ctrl+Alt+Del, Scale Mode, Clipboard Sync Toggle, Screenshot. |
| Clipboard                | Bidirectional clipboard sync (text and images for RDP; text only for VNC unless extended clipboard is negotiated). A clipboard indicator in the status bar shows sync status. |
| Multi-monitor (RDP)      | Settings dialog shows a monitor layout preview. User can select which monitors to span or use a single virtual monitor. |

#### 10.8.3 SFTP Browser Canvas

| Feature                  | Behaviour                                                                                       |
|--------------------------|-------------------------------------------------------------------------------------------------|
| Layout                   | Dual-pane: local filesystem (left), remote filesystem (right). Each pane has: breadcrumb path bar, file list (icon, name, size, date, permissions), and a status footer (items count, selected size). |
| Navigation               | Double-click a directory to enter it. Breadcrumb segments are clickable. Type-ahead filtering: start typing to jump to matching filenames. |
| Transfer                 | Drag files between panes or from the OS file manager. A transfer queue panel slides up from the bottom showing progress bars, speed, ETA. Right-click a transfer: Pause, Resume, Cancel, Retry. |
| File operations          | Right-click: Rename, Delete, Change Permissions (chmod dialog), New Folder, Download, Upload, Edit in Editor (opens integrated editor), View (inline preview for text/images). |
| Sync wizard              | Accessible from the toolbar. Compares two directories, shows a diff table (new, modified, deleted, unchanged), and lets the user select sync direction per file or in bulk. |

---

### 10.9 Theming System

#### 10.9.1 Shipped Themes

| Theme Name        | Style    | Palette Notes                                                    |
|-------------------|----------|------------------------------------------------------------------|
| CrossTerm Dark    | Dark     | Neutral dark grey surfaces, teal accent. Default theme.          |
| CrossTerm Light   | Light    | Warm white surfaces, indigo accent.                              |
| Solarized Dark    | Dark     | Ethan Schoonover's Solarized palette.                            |
| Solarized Light   | Light    | Ethan Schoonover's Solarized palette (light variant).            |
| Dracula           | Dark     | Dracula community palette.                                       |
| Nord              | Dark     | Arctic Nord palette.                                             |
| Monokai Pro       | Dark     | Warm dark with vivid ANSI colours.                               |
| High Contrast     | Dark     | WCAG AAA-compliant. Pure white text on pure black. For accessibility. |

#### 10.9.2 Theme File Format

Themes are JSON files (`.crossterm-theme`) containing values for every design token defined in §10.2. The schema is versioned; a `schema_version` field ensures forward compatibility.

```json
{
  "schema_version": 1,
  "name": "My Custom Theme",
  "author": "user",
  "variant": "dark",
  "tokens": {
    "surface-primary": "#1e1e2e",
    "surface-secondary": "#27273a",
    "text-primary": "#cdd6f4",
    "accent-primary": "#89b4fa",
    "terminal-background": "#1e1e2e",
    "terminal-foreground": "#cdd6f4",
    "terminal-ansi-0": "#45475a",
    "...": "..."
  }
}
```

#### 10.9.3 Theme Behaviour

- **OS auto-switch**: By default, follow the OS light/dark mode preference. The user can pair a light theme and a dark theme for auto-switching, or pin a single theme.
- **Per-session override**: A session definition can specify a theme override (e.g., production servers use a red-tinted theme as a visual warning).
- **Import/export**: Themes can be imported from `.crossterm-theme` files or from iTerm2 `.itermcolors` and Windows Terminal `settings.json` colour schemes.
- **Live preview**: Changing a theme in settings applies instantly to all open sessions without reconnecting.

---

### 10.10 Onboarding & Empty States

#### 10.10.1 First Launch Wizard

The first-launch wizard runs exactly once per profile. It consists of a sequence of full-canvas steps with a progress indicator, back/next navigation, and a "Skip All" link.

| Step | Title                  | Content                                                                                       |
|------|------------------------|-----------------------------------------------------------------------------------------------|
| 1    | Welcome                | Logo animation, tagline, "Get Started" button.                                                |
| 2    | Create Profile         | Profile name, master password (with strength meter), optional biometric enrollment.           |
| 3    | Import Sessions        | Auto-detect installed tools (PuTTY, MobaXterm, Termius, `~/.ssh/config`). List detected sources with checkboxes. "Import Selected" or "Skip". |
| 4    | Cloud CLIs             | Detect installed CLIs (aws, az, gcloud). Show status per provider. Link to install instructions for missing ones. "Configure Credentials" button per provider. |
| 5    | Choose Theme           | Grid of theme previews with a live terminal preview showing coloured `ls` output. Select one. |
| 6    | Ready                  | Summary of what was configured. "Open CrossTerm" button → lands on the main window.          |

On Android, the wizard uses a vertical scroll layout with large touch targets instead of horizontal steps.

#### 10.10.2 Empty States

Every panel must have a purposeful empty state rather than a blank area.

| Panel              | Empty State Content                                                                              |
|--------------------|--------------------------------------------------------------------------------------------------|
| Session tree       | Illustration + "No sessions yet. Create your first connection or import from another tool." Two buttons: "New Session" and "Import". |
| Tab bar (no tabs)  | Centre the canvas with a large "+" icon and "Press Ctrl+T to open a new session."               |
| Cloud Assets       | Per-provider empty state: "AWS CLI not detected. Install it to browse your cloud resources." with a link to the install guide. |
| SFTP browser       | "Connect to a remote host to browse its file system." with a session picker dropdown.            |
| Snippet manager    | "No snippets saved. Highlight a command in any terminal and choose 'Save as Snippet'."          |
| Search results     | "No results for '[query]'. Try a different search term."                                         |

---

### 10.11 Responsive Breakpoints

The application must respond to viewport size changes (window resize, tiling, display changes) according to these breakpoints:

| Breakpoint Name | Width Range     | Layout Adjustments                                                                              |
|-----------------|-----------------|--------------------------------------------------------------------------------------------------|
| Compact         | < 600 px        | Android phone layout. Sidebar hidden (drawer only). Bottom nav visible. No split panes. Single tab visible in top bar; others in drawer. |
| Medium          | 600–1024 px     | Sidebar collapses to icon rail. Tab bar shows max 5 tabs before overflow. Bottom panel available but defaults to hidden. One split-pane level. |
| Expanded        | 1025–1440 px    | Full sidebar expanded. Tab bar shows 8+ tabs. Bottom panel available. Multi-level split panes.   |
| Large           | > 1440 px       | Same as Expanded but sidebar can show two content modes simultaneously (e.g., sessions tree above, cloud assets below, with a horizontal divider). |

Breakpoints are evaluated on the **application window width**, not the monitor width, to behave correctly on tiling WMs and side-by-side window arrangements.

---

### 10.12 Accessibility

#### 10.12.1 Standards

Target **WCAG 2.1 AA** for all non-terminal UI surfaces (sidebar, dialogs, settings, file browser, cloud asset panels). The terminal canvas itself is inherently visual, but must support the following assistive technology interactions.

#### 10.12.2 Specific Requirements

| Area                   | Requirement                                                                                      |
|------------------------|--------------------------------------------------------------------------------------------------|
| Focus management       | Visible focus ring (2 px, `border-focus` token) on all interactive elements. Focus order follows a logical DOM order. No focus traps except in modal dialogs (which trap focus per WAI-ARIA). |
| Screen reader          | ARIA roles on all UI regions (`navigation`, `main`, `complementary`, `status`). Live regions (`aria-live="polite"`) for: connection state changes, transfer progress updates, notification toasts. Session canvas announced as an `application` landmark with a descriptive label. |
| Keyboard only          | Every feature must be operable without a mouse. Tab, Shift+Tab, Enter, Space, Escape, Arrow keys must work per WAI-ARIA widget patterns (tabs, tree, menu, dialog, listbox). |
| Colour independence    | Never use colour alone to convey status. Connection status dots are supplemented with icon shapes: ● connected, ◌ disconnected, ◉ connecting, ○ idle. |
| Text sizing            | UI text respects a user-configurable scale factor (75%–200%) independent of terminal font size. On Android, respect the system font size preference. |
| Motion                 | Respect `prefers-reduced-motion`. When active, disable all transitions and animations, replacing them with instant state changes. |
| High contrast          | The High Contrast theme (§10.9.1) must achieve ≥ 7:1 contrast ratio for all text and ≥ 3:1 for all interactive boundaries (WCAG AAA for text). |

---

### 10.13 Localisation

| Aspect                   | Specification                                                                                    |
|--------------------------|--------------------------------------------------------------------------------------------------|
| String externalisation   | All user-facing strings extracted into JSON locale files using ICU MessageFormat syntax. No hardcoded strings in UI components. |
| Base locale              | `en` (English). All keys are English-readable fallbacks.                                         |
| Locale resolution        | OS locale → user override in settings → fallback to `en`.                                        |
| RTL support              | Layout must flip correctly for RTL locales (Arabic, Hebrew). Use CSS `direction` and logical properties (`inline-start`, `inline-end`) throughout. The sidebar anchors to the inline-start edge. |
| Pluralisation            | Use ICU `{count, plural, one {# item} other {# items}}` for all countable strings.              |
| Date and time            | Use the `Intl.DateTimeFormat` API with the active locale. Never hardcode date formats.           |
| Number formatting        | Use `Intl.NumberFormat` for file sizes, transfer speeds, and counts.                             |
| Community locales        | A `locales/` directory in the user data folder. Drop a `xx.json` file to add a language. The Settings → Language panel lists detected locale files and allows switching without restart. |
| Android-specific         | Also provide Android-standard `values-xx/strings.xml` for any OS-level strings (notification channel names, foreground service label). |

---

### 10.14 Error & Feedback States

Define consistent patterns for communicating errors, warnings, and success across the application.

#### 10.14.1 Toast Notifications

Transient, non-blocking messages that appear in the bottom-right corner (desktop) or top-centre (Android).

| Type    | Colour                | Icon       | Duration    | Dismissal                     |
|---------|-----------------------|------------|-------------|-------------------------------|
| Success | `status-connected`    | Check      | 4 seconds   | Auto-dismiss or swipe/click ×  |
| Info    | `accent-primary`      | Info       | 5 seconds   | Auto-dismiss or swipe/click ×  |
| Warning | `status-connecting`   | Triangle ! | 8 seconds   | Manual dismiss only            |
| Error   | `status-disconnected` | Circle ×   | Persistent  | Manual dismiss only            |

Toasts stack vertically (max 3 visible; older ones queue). Each toast has a progress bar showing remaining display time.

#### 10.14.2 Inline Validation

Form fields (settings, session editor, credential vault) display validation errors inline below the field with `status-disconnected` coloured text and a shake animation (150 ms, 3 px horizontal oscillation, respects reduced-motion).

#### 10.14.3 Connection Failure Flow

When a session fails to connect:

1. The tab's status dot turns red (◌).
2. A banner appears at the top of the session canvas: "Connection failed: [error message]." with buttons: "Retry", "Edit Session", "Dismiss".
3. If auto-reconnect is enabled, show a countdown: "Reconnecting in 5… 4… 3…" with a "Cancel" link. Use exponential backoff: 5 s → 10 s → 20 s → 60 s (max). After 5 failed attempts, stop auto-reconnect and show a persistent error banner.

---

### 10.15 UX Extensibility Contract

This UX spec is designed to be updated per platform without disrupting the shared design system. The following rules govern future UX updates:

1. **Token-driven changes only** — All visual changes (colour, spacing, typography) must be made by updating design tokens, never by adding one-off CSS overrides.
2. **Platform sections are independent** — A change to §10.5 (macOS) must not require a change to §10.4 (Windows) unless the shared spec (§10.2, §10.3, §10.8) is modified.
3. **New platforms** — To add a platform (e.g., iOS, ChromeOS), add a new §10.X section. Define only the deltas from the shared spec. Default behaviour is the shared spec.
4. **Component library versioning** — The React component library is versioned independently of the application. Platform-specific component variants (e.g., `<Sidebar variant="macos">`) are co-located in the same component file using the platform token.
5. **Design token file location** — Tokens live in `src/themes/tokens.json` (shared) with platform override files `tokens.windows.json`, `tokens.macos.json`, `tokens.linux.json`, `tokens.android.json` that are merged at build time.

---

## 11. Advanced Productivity Features

### 11.1 Split Panes

- Split any tab horizontally or vertically (unlimited recursive splits).
- Each pane is an independent session.
- Keyboard shortcuts to navigate between panes (Alt+Arrow).
- "Broadcast input to all panes in tab" toggle.

### 11.2 Session Recording & Playback

- Record terminal sessions in **asciicast v2** format (compatible with asciinema).
- Playback with speed control (0.5×, 1×, 2×, 4×) and seek bar.
- Export recordings to GIF or MP4 for documentation purposes.

### 11.3 Macro & Automation Engine

- Record a sequence of keystrokes / commands as a **macro**.
- Macros support delays, loops, conditional waits (wait for regex pattern in output), and variable prompts.
- Execute macros on one session or broadcast to a selection of sessions.
- Macros are stored per-profile and are exportable as JSON.

### 11.4 Expect-Style Automation Scripts

- Support a simple YAML-based scripting DSL for automated interactions:

```yaml
script: deploy_app
steps:
  - expect: "password:"
    send: "{{vault:deploy_password}}"
  - expect: "\\$"
    send: "cd /opt/app && ./deploy.sh"
  - expect: "Deploy complete"
    notify: "Deployment finished on {{session.host}}"
```

- Vault references (`{{vault:name}}`) resolve credentials at runtime from the encrypted vault.

### 11.5 Integrated Diff Viewer

- Compare two files side-by-side with syntax highlighting (pulled from remote hosts via SFTP or local filesystem).
- Inline diff (unified) and side-by-side diff modes.

### 11.6 SSH Key Manager

- Generate RSA (3072, 4096), Ed25519, and ECDSA keys.
- View, import, export keys.
- Deploy public keys to remote hosts (equivalent of `ssh-copy-id`).
- SSH agent integration: load keys into the OS SSH agent or CrossTerm's built-in agent.

### 11.7 Integrated Code Editor

- Lightweight embedded editor (based on Monaco or CodeMirror 6) for quick edits of remote files opened via SFTP.
- Syntax highlighting for 50+ languages.
- Save-on-close triggers SFTP upload back to the remote host.

### 11.8 Notification System

- Desktop notifications (and Android notifications) for:
  - Connection lost / reconnected.
  - Pattern match in terminal output (user-defined regex triggers).
  - Long-running command completion (detect shell prompt return after > N seconds of output silence).
  - Tunnel failure.
- Notification history panel within the application.

---

## 12. Security Requirements

### 12.1 Data at Rest

- All credentials, private keys, and session passwords encrypted with AES-256-GCM.
- The vault database uses SQLCipher with a key derived per §4.1.
- Session logs are optionally encrypted (user toggle; default: off for performance).
- Temporary files are written to a platform-appropriate secure directory and wiped on exit.

### 12.2 Data in Transit

- SSH: enforce minimum key exchange of curve25519-sha256, chacha20-poly1305 cipher, hmac-sha2-256 MAC. Allow the user to configure stricter policies.
- RDP: enforce TLS 1.2+ with NLA. Refuse legacy RDP security.
- VNC: warn the user when connecting without TLS (VeNCrypt). Block unencrypted VNC by default (overridable per session).
- HTTPS-only for any cloud API communication.

### 12.3 Application Security

- No telemetry or analytics data collection of any kind without explicit opt-in.
- Auto-update mechanism must verify code signatures before applying updates.
- Plugin/WASM runtime is sandboxed: no filesystem access outside a declared plugin data directory, no network access unless explicitly granted by the user.
- Memory: credential strings are stored in pinned, non-swappable memory where the platform permits it. Zeroize on drop (Rust `zeroize` crate).

### 12.4 Audit Log

- Maintain an append-only local audit log per profile recording: session connect/disconnect events, credential access events, vault unlock/lock events, profile export events.
- The audit log is viewable within the application and exportable as CSV.

---

## 13. Plugin / Extension System

### 13.1 Architecture

- Plugins are compiled to WebAssembly (WASM) and run inside a sandboxed runtime (wasmtime).
- A plugin manifest declares: name, version, author, required permissions (filesystem scope, network hosts, UI panel slot).
- The user must approve permissions on first load.

### 13.2 Plugin API Surface

Plugins may:

- Register new session types.
- Add sidebar panels.
- Add context menu items to sessions and the terminal.
- React to lifecycle hooks (on_connect, on_disconnect, on_output_line, on_command).
- Store key-value data in a plugin-scoped encrypted store.
- Make HTTP requests to user-approved hosts.

### 13.3 Plugin Distribution

- Plugins can be loaded from a local `.wasm` file.
- A community plugin registry (phase 2) indexed as a public Git repository of manifests.

---

## 14. SFTP / File Transfer

### 14.1 SFTP Browser

- Dual-pane interface: local filesystem on the left, remote filesystem on the right.
- Drag-and-drop between panes and from the OS file manager.
- Queue-based transfer with progress, pause, resume, retry.
- Bandwidth throttling (configurable per transfer or globally).
- Symlink handling: follow or display as symlinks (user preference).
- Inline file preview (text, images, PDFs) without downloading.
- Folder synchronisation wizard: compare local and remote directories, show diff, sync in either direction.

### 14.2 File Transfer Protocols

In addition to SFTP, support:

- SCP (for legacy hosts).
- FTP / FTPS (explicit TLS) for environments that require it.
- (Phase 2) Rsync-over-SSH for efficient incremental sync.

---

## 15. Configuration & Settings Model

### 15.1 Settings Hierarchy

Settings resolve in the following order (highest priority first):

1. **Session-level overrides** — stored in the session definition.
2. **Folder-level defaults** — inheritable settings on a folder node.
3. **Profile-level settings** — the profile's global preferences.
4. **Application defaults** — hardcoded sensible defaults.

### 15.2 Settings Storage

- All settings are stored as JSON files within the profile's data directory.
- A settings editor UI is provided with search and categorisation.
- An "Advanced: Edit JSON" option is always available for direct editing.
- Settings changes take effect immediately (no restart required) except for changes to the updater or plugin runtime.

### 15.3 Portable Mode

- When a `.crossterm-portable` sentinel file exists in the application directory, all profile data is stored alongside the executable (useful for USB-drive deployments).
- Portable mode must work on Windows and Linux.

---

## 16. Performance Targets

| Metric                            | Target                        |
|-----------------------------------|-------------------------------|
| Cold start to usable UI           | < 2 seconds (desktop)         |
| Tab open to shell prompt (local)  | < 500 ms                      |
| SSH connect to shell prompt (LAN) | < 1.5 seconds                 |
| Terminal throughput               | ≥ 80 MB/s sustained (cat large file) |
| SFTP transfer throughput          | ≥ 90% of raw SCP throughput   |
| Memory per idle terminal tab      | < 15 MB                       |
| Memory baseline (1 tab open)      | < 120 MB (desktop)            |
| APK install size                  | < 50 MB                       |

---

## 17. Testing Requirements

### 17.1 Unit Tests

- Minimum 80% line coverage on all Rust backend modules (credential vault, SSH, config parser, cloud integrations).
- Terminal escape sequence parser must have a dedicated conformance test suite derived from vttest.

### 17.2 Integration Tests

- Spin up Docker containers (OpenSSH server, xrdp, TigerVNC) as test targets in CI.
- Validate connect, authenticate (password + key), execute command, disconnect lifecycle for each session type.
- Cloud integrations tested against LocalStack (AWS), Azurite (Azure Storage), and fake-gcs-server (GCP).

### 17.3 End-to-End Tests

- Playwright (desktop web view) and Appium (Android) E2E suites covering:
  - Profile creation, lock, unlock.
  - Session CRUD, connect, output verification.
  - SFTP upload/download round-trip.
  - Settings change and verify.

### 17.4 Security Tests

- Credential vault fuzz testing (rust `cargo fuzz`).
- Dependency audit in CI (`cargo audit`, `npm audit`).
- Static analysis (Clippy at `deny` level, ESLint strict).

---

## 18. Packaging & Distribution

### 18.1 Installers

| Platform | Format         | Notes                                                    |
|----------|----------------|----------------------------------------------------------|
| Windows  | MSI + MSIX     | Per-user and per-machine install options. Start menu, PATH entry, shell integration (right-click "Open CrossTerm here"). |
| macOS    | DMG            | Signed + notarised. Homebrew cask formula.               |
| Linux    | AppImage       | Primary. Also provide DEB (Ubuntu/Debian) and RPM (Fedora/RHEL). Flatpak for sandboxed installs. |
| Android  | APK / AAB      | Google Play Store listing. Direct APK for sideloading.   |

### 18.2 Shell Integration

On desktop platforms, install a shell integration script (similar to iTerm2 shell integration) that enables:

- Current working directory tracking in the tab title.
- Command duration display in the status bar.
- Mark and jump to previous command prompts in scrollback.

---

## 19. Licensing & Legal

- The core application should be licensed under **Apache 2.0** or **MIT** (dual license).
- FreeRDP (Apache 2.0) and libvncclient (GPL-2.0) integration must respect their respective licenses. If libvncclient's GPL is problematic, use a permissive alternative or isolate it as a subprocess.
- All third-party dependencies must be auditable via an SBOM (CycloneDX format) generated in CI.

---

## 20. Delivery Phases

### Phase 1 — Core (MVP)

Deliver within the first development cycle:

- Local shell, SSH terminal, SFTP browser.
- Credential vault with master password.
- Multi-profile support.
- Session tree with folders, tags, search.
- Split panes, tab management.
- Basic theming (dark/light).
- Windows + macOS + Linux desktop builds.
- xterm.js GPU-rendered terminal.

### Phase 2 — Remote Desktop & Cloud

- RDP and VNC session types.
- AWS, Azure, GCP CLI integration and resource browsers.
- Network scanner and WOL.
- Android build.
- Session recording (asciicast).
- Snippet manager and command palette.

### Phase 3 — Advanced & Ecosystem

- Plugin/WASM system.
- Macro and expect-script engine.
- Profile sync.
- Community plugin registry.
- Integrated code editor.
- Diff viewer.
- Localisation framework and initial community locales.

---

## 21. Appendix A — File & Directory Structure

```
crossterm/
├── src-tauri/              # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs
│   │   ├── vault/          # Credential vault module
│   │   ├── ssh/            # SSH client module
│   │   ├── rdp/            # FreeRDP FFI bindings
│   │   ├── vnc/            # VNC client module
│   │   ├── serial/         # Serial console module
│   │   ├── cloud/          # AWS, Azure, GCP integrations
│   │   ├── network/        # Scanner, WOL, tunnels
│   │   ├── plugins/        # WASM plugin runtime
│   │   ├── config/         # Settings & profile management
│   │   └── audit/          # Audit log
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                    # React frontend
│   ├── components/
│   │   ├── Terminal/
│   │   ├── SessionTree/
│   │   ├── SftpBrowser/
│   │   ├── RdpViewer/
│   │   ├── VncViewer/
│   │   ├── CloudDashboard/
│   │   ├── Settings/
│   │   ├── Vault/
│   │   └── Shared/
│   ├── hooks/
│   ├── stores/             # State management (Zustand)
│   ├── themes/
│   ├── i18n/
│   └── App.tsx
├── plugins/                # Example WASM plugins
├── scripts/                # Build & CI scripts
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── docs/
├── .github/workflows/
├── package.json
├── tsconfig.json
└── README.md
```

---

## 22. Appendix B — Key Interaction Flows

### B.1 First Launch

1. Application starts → displays welcome wizard.
2. User creates a "Default" profile → prompted to set a master password.
3. Wizard offers to import sessions from detected installations (PuTTY, MobaXterm, Termius, SSH config).
4. Wizard offers to configure cloud CLI credentials (detect installed CLIs).
5. User lands on the main window with a "Getting Started" tab showing tips.

### B.2 SSH Connection with Jump Host

1. User selects a saved session configured with a ProxyJump chain (bastion → target).
2. CrossTerm resolves the chain, authenticates to the bastion first using its linked credential, then tunnels through to the target.
3. Both hops are displayed in the status bar. If either hop drops, the user is notified and auto-reconnect is attempted.

### B.3 Cloud VM Connect (AWS Example)

1. User opens the Cloud Assets sidebar → AWS → EC2 → selects region.
2. Instance list loads (name, ID, state, public/private IP).
3. User right-clicks an instance → "Connect via SSM" or "Connect via SSH".
4. For SSM: CrossTerm invokes `aws ssm start-session` in a new terminal tab.
5. For SSH: CrossTerm uses the instance's public IP (or private IP + bastion) and the linked SSH key credential.

---

## 23. Appendix C — Android-Specific Adaptations

- Replace the menu bar with a hamburger/drawer menu.
- The tab bar becomes a horizontally scrollable strip.
- The sidebar is a swipe-in drawer from the left edge.
- The SFTP browser uses a single-pane layout with a breadcrumb navigator; tap-and-hold for file actions.
- RDP and VNC sessions support pinch-to-zoom and a floating toolbar for mouse mode, keyboard toggle, and disconnect.
- The extra-keys bar above the soft keyboard is customisable (add/remove/reorder keys).
- Support for external Bluetooth/USB keyboards with full shortcut mapping.
- Background service to keep SSH sessions alive when the app is backgrounded (with a persistent notification as required by Android).

---

*End of specification.*

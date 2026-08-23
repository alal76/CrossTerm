# CrossTerm

[![CI](https://github.com/alal76/CrossTerm/actions/workflows/ci.yml/badge.svg)](https://github.com/alal76/CrossTerm/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alal76/CrossTerm)](https://github.com/alal76/CrossTerm/releases/latest)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)](https://github.com/alal76/CrossTerm/releases/latest)

A cross-platform terminal emulator and remote access suite built with Tauri 2.x. CrossTerm consolidates SSH, SFTP, RDP, VNC, serial, telnet, and local shell management into a single fast native application — MobaXterm-level power on every platform.

## Installation

### macOS

**Homebrew (recommended):**
```bash
brew tap alal76/crossterm
brew install --cask crossterm
```

**Direct download:** grab `CrossTerm_x.y.z_aarch64.dmg` from the [latest release](https://github.com/alal76/CrossTerm/releases/latest), open it, and drag CrossTerm.app to Applications.

**Upgrade:**
```bash
brew upgrade --cask crossterm
```

### Windows

Download `CrossTerm_x.y.z_x64-setup.exe` (standard installer) or `CrossTerm_x.y.z_x64_en-US.msi` (MSI for enterprise/GPO deployment) from the [latest release](https://github.com/alal76/CrossTerm/releases/latest) and run it.

**Upgrade:** run the new installer — it replaces the existing installation in-place.

### Linux

**Debian / Ubuntu (.deb):**
```bash
wget https://github.com/alal76/CrossTerm/releases/latest/download/CrossTerm_x.y.z_amd64.deb
sudo dpkg -i CrossTerm_x.y.z_amd64.deb
```

**Red Hat / Fedora / SUSE (.rpm):**
```bash
sudo rpm -i CrossTerm-x.y.z-1.x86_64.rpm
```

**Universal AppImage:**
```bash
chmod +x CrossTerm_x.y.z_amd64.AppImage
./CrossTerm_x.y.z_amd64.AppImage
```

**Upgrade (deb):**
```bash
sudo dpkg -i CrossTerm_new_version_amd64.deb   # dpkg handles upgrade automatically
```

All release assets and their SHA256 checksums are listed on the [releases page](https://github.com/alal76/CrossTerm/releases).

---

## Features

### Terminal & Sessions
- **Local Shell** — spawn bash / zsh / fish / PowerShell with xterm.js WebGL renderer
- **Tab Management** — multi-tab with drag-to-reorder, rename, color labels
- **Split Panes** — horizontal and vertical splits, independent sessions per pane
- **Broadcast Mode** — type once, send to all open panes simultaneously
- **Session Library** — save, folder-organize, tag, and search saved connections
- **Session Recording** — record terminal sessions to `.cast` (asciinema) files and replay

### Remote Protocols
- **SSH** — password, key (RSA/Ed25519/ECDSA), certificate, and SSH agent authentication
- **SSH Port Forwarding** — local, remote, and dynamic (SOCKS5)
- **SSH Jump Hosts** — chain multiple hops for complex network topologies
- **SFTP** — dual-pane graphical file browser, drag-and-drop, queue-based transfers
- **RDP** — Remote Desktop Protocol client
- **VNC** — VNC remote desktop viewer
- **Telnet** — legacy Telnet client
- **Serial** — serial port terminal with baud/parity/stop-bit configuration

### Security
- **Encrypted Credential Vault** — AES-256-GCM encryption, Argon2id key derivation, per-profile isolation
- **SSH Key Manager** — generate, import, and manage SSH key pairs
- **Audit Log** — track all credential access and connection events, export to CSV
- **Idle Lock** — automatically lock the vault after configurable inactivity timeout
- **Host Key Verification** — TOFU (trust-on-first-use) with change detection warnings

### Network Tools
- **Network Explorer** — auto-detect local subnets, scan hosts, enumerate open ports and services
- **Quick Scan / Full Scan** — configurable concurrency and port ranges
- **WiFi Analysis** — signal strength, channel, security type, BSSID (macOS)
- **Export** — save scan results to JSON or CSV

### Productivity
- **Command Palette** — ⌘⇧P / Ctrl+Shift+P — fuzzy-search every action
- **Snippets** — store and execute frequently-used command snippets
- **Macros** — record and replay command sequences
- **Quick Connect** — open a connection without saving a session
- **Profile Sync** — export and import all settings as a `.ctbundle` file

### Appearance & Customization
- **Themes** — Dark, Light, System, Dracula, Nord, Monokai Pro, Solarized Dark/Light, One Dark
- **Custom Themes** — full CSS-variable token system
- **Font** — configurable family, size, ligatures, line height, letter spacing
- **Cursor** — block / underline / bar, with optional blink
- **Opacity** — adjustable terminal background transparency

### Settings (MobaXterm-level depth)
10-tab settings panel covering General, Appearance, Terminal, SSH, Connections, File Transfer, Keyboard, Notifications, Security, and Advanced. See the [Settings Reference](docs/help/settings.md) for full detail.

---

## Quick Start

1. Install CrossTerm for your platform (see above).
2. Press **Ctrl+T** (⌘T on macOS) to open a local shell.
3. Press **Ctrl+Shift+N** (⌘⇧N) to open Quick Connect and SSH to a server.
4. Press **Ctrl+,** (⌘,) to open Settings.
5. Press **Ctrl+Shift+P** (⌘⇧P) to open the Command Palette.

Full documentation is at the [CrossTerm Docs site](https://alal76.github.io/CrossTerm/) or in the [`docs/help/`](docs/help/) directory.

---

## Building from Source

### Prerequisites

| Tool | Version |
|------|---------|
| [Rust](https://rustup.rs) | 1.77.2+ |
| [Node.js](https://nodejs.org) | 20+ |
| [npm](https://npmjs.com) | 9+ |

**Linux only — system libraries:**
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

### Build

```bash
# Install frontend dependencies
npm install

# Development mode with hot-reload
npm run tauri dev

# Production build
npm run tauri build
# → binaries in target/release/bundle/
```

### Tests

```bash
npm test                     # frontend unit tests (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit + integration tests
```

---

## Project Structure

```
CrossTerm/
├── src/                        # React frontend (TypeScript)
│   ├── App.tsx                 # Root application component
│   ├── components/             # UI components by feature
│   ├── hooks/                  # Custom React hooks
│   ├── i18n/                   # Internationalisation strings
│   ├── stores/                 # Zustand state stores
│   ├── themes/                 # Theme JSON token files (9 themes)
│   └── types/                  # TypeScript type definitions
├── src-tauri/                  # Tauri / Rust backend
│   └── src/
│       ├── audit/              # Audit logging
│       ├── config/             # Profile & settings persistence
│       ├── network/            # Network scan, WiFi analysis
│       ├── ssh/                # SSH client, tunnels, jump hosts
│       ├── sftp/               # SFTP client
│       ├── vault/              # Encrypted credential vault
│       ├── terminal/           # Local PTY management
│       └── ...                 # rdp, vnc, telnet, serial, macros, …
├── docs/                       # MkDocs documentation site
│   └── help/                   # User-facing help articles
├── packaging/
│   └── homebrew/               # Homebrew Cask formula
└── .github/workflows/          # CI (ci.yml) + Release (release.yml)
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri 2.x (Rust + WebView) |
| Frontend | React 18, TypeScript, TailwindCSS |
| Terminal Renderer | xterm.js (WebGL addon) |
| SSH | russh (pure Rust) |
| Cryptography | AES-256-GCM, Argon2id |
| Database | SQLite (rusqlite) |
| State Management | Zustand |
| Build / Bundle | Vite |

---

## Contributing

Contributions are welcome.

1. Fork the repository and create a feature branch.
2. Run `cargo clippy -- -D warnings` and `npm run lint` before committing.
3. Ensure `npm test` passes.
4. Open a Pull Request against `main`.

See [`.github/dev-guidelines.md`](.github/dev-guidelines.md) for coding standards.

## License

Licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).

Copyright © 2024-2026 Abhishek Lal.

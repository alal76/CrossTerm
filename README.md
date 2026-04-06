# CrossTerm

A cross-platform terminal emulator and remote access suite built with Tauri 2.x. CrossTerm consolidates SSH, SFTP, and local shell management into a single, fast, native application.

## Features (Phase 1 — MVP)

- **Local Terminal** — Spawn your default shell (bash/zsh/PowerShell) with xterm.js (WebGL-accelerated)
- **SSH Client** — Connect to remote hosts with password or key authentication
- **Port Forwarding** — Local, remote, and dynamic (SOCKS5) forwarding over SSH
- **Jump Hosts** — Proxy through intermediate SSH servers
- **SFTP Browser** — Dual-pane graphical file browser over SSH
- **Encrypted Credential Vault** — AES-256-GCM encryption, Argon2id key derivation
- **Multi-Profile Support** — Isolated settings, sessions, and vaults per user profile
- **Session Management** — Organize connections in folders, tag, search, and group
- **Audit Logging** — Track credential access and connection events
- **Theming** — Dark and light themes with customizable token system
- **Internationalization** — i18n-ready with English locale included
- **Cross-Platform** — Windows, macOS, and Linux from a single codebase

## Screenshots

> _Screenshots will be added here once the UI is finalized._

## Prerequisites

| Tool | Version |
|------|---------|
| [Rust](https://rustup.rs) | 1.77.2+ |
| [Node.js](https://nodejs.org) | 20+ |
| [npm](https://npmjs.com) | 9+ |

### Linux only

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

## Build Instructions

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot-reload)
npm run tauri dev

# Build for production
npm run tauri build
```

The release binary will be in `src-tauri/target/release/bundle/`.

## Project Structure

```
CrossTerm/
├── src/                        # React frontend (TypeScript)
│   ├── App.tsx                 # Root application component
│   ├── main.tsx                # Entry point
│   ├── components/             # UI components
│   │   ├── Android/            # Android-specific views
│   │   ├── Audit/              # Audit log viewer
│   │   ├── Cloud/              # Cloud provider integrations
│   │   ├── Editor/             # Text/config editor
│   │   ├── Ftp/                # FTP client UI
│   │   ├── Help/               # Help system & documentation viewer
│   │   ├── KeyManager/         # SSH key management
│   │   ├── Macros/             # Macro recording & playback
│   │   ├── NetworkTools/       # Ping, traceroute, DNS tools
│   │   ├── Notifications/      # Toast notification system
│   │   ├── Plugin/             # Plugin management UI
│   │   ├── RdpViewer/          # Remote Desktop viewer
│   │   ├── Recording/          # Session recording & playback
│   │   ├── Serial/             # Serial port terminal
│   │   ├── SessionTree/        # Session sidebar tree
│   │   ├── Settings/           # Settings & preferences panels
│   │   ├── SftpBrowser/        # Dual-pane SFTP file browser
│   │   ├── Shared/             # Shared/common components
│   │   ├── Snippets/           # Snippet manager
│   │   ├── TabBar/             # Tab bar & split pane container
│   │   ├── Telnet/             # Telnet client
│   │   ├── Terminal/           # Local & SSH terminal tabs
│   │   ├── Vault/              # Credential vault UI
│   │   └── VncViewer/          # VNC remote viewer
│   ├── hooks/                  # Custom React hooks
│   ├── i18n/                   # Internationalization strings
│   ├── stores/                 # Zustand state stores
│   │   ├── appStore.ts         #   UI state, theme, layout
│   │   ├── sessionStore.ts     #   Sessions, tabs, connections
│   │   ├── terminalStore.ts    #   Terminal instances
│   │   └── vaultStore.ts       #   Credential vault operations
│   ├── styles/                 # Global CSS
│   ├── themes/                 # Theme JSON files (9 themes)
│   ├── types/                  # TypeScript type definitions
│   ├── utils/                  # Utility functions
│   └── test/                   # Test setup & helpers
├── src-tauri/                  # Tauri / Rust backend
│   ├── src/
│   │   ├── lib.rs              # App setup & command registration
│   │   ├── main.rs             # Entry point
│   │   ├── android/            # Android platform support
│   │   ├── audit/              # Audit logging
│   │   ├── cloud/              # Cloud provider backends
│   │   ├── config/             # Profile & session configuration
│   │   ├── editor/             # Editor backend
│   │   ├── ftp/                # FTP client
│   │   ├── keygen/             # SSH key generation
│   │   ├── keymgr/             # SSH key manager
│   │   ├── l10n/               # Localization backend
│   │   ├── macros/             # Macro engine
│   │   ├── network/            # Network diagnostic tools
│   │   ├── notifications/      # Notification system
│   │   ├── plugin_rt/          # Plugin runtime
│   │   ├── rdp/                # RDP client
│   │   ├── recording/          # Session recording
│   │   ├── security/           # Security utilities
│   │   ├── serial/             # Serial port I/O
│   │   ├── sftp/               # SFTP client
│   │   ├── snippets/           # Snippet storage
│   │   ├── ssh/                # SSH client, port forwarding, jump hosts
│   │   ├── sync/               # Settings sync
│   │   ├── telnet/             # Telnet client
│   │   ├── terminal/           # Local PTY management
│   │   ├── vault/              # Encrypted credential vault
│   │   ├── vnc/                # VNC client
│   │   └── window/             # Window management
│   ├── benches/                # Performance benchmarks
│   ├── capabilities/           # Tauri capability definitions
│   ├── fuzz/                   # Fuzz testing harnesses
│   ├── resources/              # Bundled resources
│   └── tests/                  # Integration tests
├── docs/                       # Project documentation
│   ├── ARCHITECTURE.md         # System architecture
│   ├── CODING-STANDARDS.md     # Code style & conventions
│   ├── DESIGN.md               # Design system & tokens
│   ├── QA.md                   # Testing strategy
│   └── help/                   # User-facing help articles
├── e2e/                        # Playwright end-to-end tests
├── build-scripts/              # Platform build scripts
├── scripts/                    # Dev & CI utility scripts
│   └── shell-integration/      # Shell integration (bash/zsh/fish)
├── icons/                      # App icons (all platforms)
├── packaging/                  # Distribution packaging (Homebrew, etc.)
├── tests/                      # Docker-based test fixtures
├── tools/                      # Development tooling
│   └── mcp-coordinator/        # MCP coordinator server
└── .github/workflows/          # CI/CD (build, test, release)
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

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m 'Add my feature'`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a Pull Request

Please run `cargo clippy -- -D warnings` and `npm run lint` before submitting.

## License

Licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).

Copyright (c) 2024-2026 Abhishek Lal.

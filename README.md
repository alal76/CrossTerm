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
├── src/                    # React frontend (TypeScript)
│   ├── components/         # UI components (Terminal, Settings, Vault, etc.)
│   ├── stores/             # Zustand state stores
│   ├── themes/             # Theme definitions (dark, light, tokens)
│   ├── i18n/               # Internationalization strings
│   └── types/              # TypeScript type definitions
├── src-tauri/              # Tauri / Rust backend
│   └── src/
│       ├── lib.rs          # Tauri app setup & command registration
│       ├── main.rs         # Entry point
│       ├── audit/          # Audit logging module
│       ├── config/         # Profile & session configuration
│       ├── ssh/            # SSH client, port forwarding, jump hosts
│       ├── terminal/       # Local PTY management
│       └── vault/          # Encrypted credential vault
└── .github/workflows/     # CI/CD (build, test, release)
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

Dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option.

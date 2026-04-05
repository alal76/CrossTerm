# CrossTerm — Architecture Specification

| Field          | Value                                |
|----------------|--------------------------------------|
| Spec ID        | ARCH-CROSSTERM-001                   |
| Version        | 1.0                                  |
| Status         | Active                               |
| Last Updated   | 2026-04-05                           |
| Parent Spec    | SPEC-CROSSTERM-001                   |

---

## 1. System Overview

CrossTerm is a **cross-platform terminal emulator and remote access suite** built on a **Tauri 2.x hybrid architecture**: a Rust backend for security-critical and system-level operations, paired with a React/TypeScript frontend rendered in the platform's native WebView.

```
┌──────────────────────────────────────────────────────────────────────┐
│                        WebView (Frontend)                            │
│  React 18 + TypeScript + TailwindCSS + xterm.js                      │
│  ┌────────┐ ┌────────┐ ┌──────────┐ ┌────────────┐ ┌──────────────┐ │
│  │ Stores │ │  i18n  │ │  Themes  │ │ Components │ │    Types     │ │
│  │Zustand │ │i18next │ │CSS Vars  │ │ Terminal/  │ │  index.ts    │ │
│  │        │ │        │ │+ JSON    │ │ Session/   │ │  (enums,     │ │
│  │ app    │ │ en.json│ │  tokens  │ │ Settings/  │ │  interfaces) │ │
│  │ session│ │        │ │ dark.json│ │ Shared/    │ │              │ │
│  │terminal│ │        │ │light.json│ │ Vault/     │ │              │ │
│  │ vault  │ │        │ │          │ │ SftpBrowser│ │              │ │
│  └───┬────┘ └────────┘ └──────────┘ └─────┬──────┘ └──────────────┘ │
│      │           Tauri IPC (invoke / events)        │                │
├──────┴──────────────────────────────────────────────┴────────────────┤
│                         Rust Backend (src-tauri/)                    │
│  ┌─────────┐ ┌───────┐ ┌──────────┐ ┌───────────┐ ┌──────────────┐ │
│  │  vault  │ │  ssh  │ │ terminal │ │  config   │ │    audit     │ │
│  │AES-GCM  │ │ russh │ │  PTY     │ │ profiles  │ │  JSONL log   │ │
│  │Argon2id │ │       │ │portable  │ │ sessions  │ │  append-only │ │
│  │SQLite   │ │       │ │-pty      │ │ settings  │ │              │ │
│  └─────────┘ └───────┘ └──────────┘ └───────────┘ └──────────────┘ │
│                                                                      │
│  Tauri Plugins: dialog, fs, shell, log                               │
│  State Management: tauri::manage() — one instance per module         │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 2. Architectural Style & Patterns

### 2.1 Hybrid Desktop Architecture (Tauri 2.x)

CrossTerm uses the **Tauri IPC bridge** pattern:
- The **frontend** (WebView) is a single-page React application responsible for all rendering and user interaction.
- The **backend** (Rust) handles system-level operations: PTY management, SSH connections, encrypted vault, file I/O, and audit logging.
- Communication flows via two mechanisms:
  - **`invoke()`** — Frontend → Backend synchronous/async RPC calls (command pattern).
  - **`emit()` / `listen()`** — Backend → Frontend event streaming (observer pattern).

**Rationale**: This separation enforces a security boundary. Sensitive operations (cryptography, credential storage, process spawning) never execute in the WebView context.

### 2.2 Pattern Catalogue

| Pattern                          | Where Used                                                  | Description |
|----------------------------------|-------------------------------------------------------------|-------------|
| **Command Pattern (IPC)**        | All `#[tauri::command]` functions in `lib.rs`               | Each backend operation is a named command invoked from the frontend. Commands are registered in `invoke_handler` as a flat handler list. |
| **Observer / Event Emitter**     | `terminal:output`, `terminal:exit`, `ssh:output`, `ssh:disconnected` | Backend pushes data to the frontend via Tauri events. Frontend subscribes with `listen()`. Enables non-blocking I/O for terminal and SSH streams. |
| **Singleton State (Managed)**    | `Vault`, `ConfigState`, `TerminalManager`, `SshState`       | Each backend module exposes a single state struct. Tauri's `manage()` registers it as app-wide shared state, injected into commands via `tauri::State<'_>`. |
| **Flux / Unidirectional Data**   | Zustand stores (`appStore`, `sessionStore`, `terminalStore`, `vaultStore`) | Frontend state flows in one direction: Action → Store mutation → React re-render via selector hooks. No prop-drilling; components subscribe to slices. |
| **Token-Based Theming**          | `index.css` CSS custom properties, `tailwind.config.ts`, `themes/*.json` | Visual properties are indirected through design tokens (CSS variables). Theme files provide concrete values. Tailwind maps utility classes to tokens. |
| **Discriminated Union Types**    | `Credential` type, `SplitPane` type, `SshAuth`, `PortForward` | TypeScript and Rust both use tagged unions with a discriminant field (`type`) for polymorphic data. Enables exhaustive matching. |
| **Registry Pattern**             | `lib.rs` `invoke_handler` — flat list of all commands       | All Tauri commands are registered centrally. Adding a new feature requires adding its commands to the registry. |
| **Repository Pattern**           | `config/mod.rs` — JSON file-based persistence               | Profiles and sessions are stored as JSON files on disk. CRUD operations go through helper functions that read/write to the profiles directory. |
| **Optimistic UI Updates**        | `vaultStore.ts` — `addCredential`, `updateCredential`       | The store updates local state immediately after invoking backend commands, rather than waiting for a re-fetch. Errors roll back via the `error` state field. |
| **Region-Based Layout**          | `App.tsx` — TitleBar, TabBar, Sidebar, SessionCanvas, BottomPanel, StatusBar | The UI is decomposed into six named regions (A–F) matching the spec. Each region is an isolated React component. |
| **Selector Pattern**             | All Zustand `useXxxStore((s) => s.field)` calls             | Components subscribe to individual state slices, preventing unnecessary re-renders. |

---

## 3. Frontend Architecture

### 3.1 Technology Stack

| Layer              | Technology          | Version | Purpose                                  |
|--------------------|---------------------|---------|------------------------------------------|
| Framework          | React               | 18.3+   | Component rendering, hooks               |
| Language           | TypeScript           | 5.5+    | Static typing                            |
| Build              | Vite                 | 5.4+    | Dev server, HMR, bundling                |
| Styling            | TailwindCSS          | 3.4+    | Utility-first CSS with design tokens     |
| State Management   | Zustand              | 4.5+    | Lightweight flux stores                  |
| Terminal           | xterm.js             | 5.5+    | Terminal emulation + WebGL rendering     |
| Icons              | Lucide React         | 0.400+  | Consistent SVG icon set                  |
| i18n               | i18next              | 23.11+  | Internationalization with JSON locales   |
| Utilities          | clsx, uuid           | —       | Class merging, UUID generation           |

### 3.2 Directory Structure & Module Boundaries

```
src/
├── main.tsx                    # Entry point — renders <App /> with i18n
├── App.tsx                     # Root component — all 6 layout regions
├── index.css                   # Theme CSS variables + Tailwind directives
├── vite-env.d.ts               # Vite type augmentation
├── types/
│   └── index.ts                # ALL TypeScript types, enums, interfaces
├── stores/
│   ├── appStore.ts             # UI state: theme, sidebar, panels, profiles
│   ├── sessionStore.ts         # Sessions, tabs, split panes, favorites
│   ├── terminalStore.ts        # Terminal instance tracking (Map-based)
│   └── vaultStore.ts           # Vault lock state, credentials, IPC calls
├── components/
│   ├── Terminal/
│   │   ├── TerminalView.tsx    # xterm.js wrapper with WebGL, fit, events
│   │   └── TerminalTab.tsx     # Tab-level terminal lifecycle
│   ├── SessionTree/
│   │   ├── SessionTree.tsx     # Session list with folders, favorites
│   │   └── SessionEditor.tsx   # Session CRUD modal form
│   ├── Settings/
│   │   └── SettingsPanel.tsx   # Settings categories UI
│   ├── SftpBrowser/
│   │   └── SftpBrowser.tsx     # Dual-pane file browser
│   ├── Shared/
│   │   ├── CommandPalette.tsx  # ⌘⇧P fuzzy command overlay
│   │   ├── QuickConnect.tsx    # ⌘⇧N quick SSH connect dialog
│   │   └── Toast.tsx           # Toast notification system
│   └── Vault/
│       ├── CredentialManager.tsx  # Credential CRUD UI
│       └── VaultUnlock.tsx        # Vault unlock/create form
├── i18n/
│   ├── index.ts                # i18next init (fallback: en)
│   └── en.json                 # English locale strings
└── themes/
    ├── dark.json               # Dark theme token values
    ├── light.json              # Light theme token values
    └── tokens.json             # Token schema / design reference
```

**Module boundary rules:**
- `types/index.ts` is the **single source of truth** for all frontend type definitions.
- Stores never import from components. Components import from stores and types.
- Components import stores via hooks (`useXxxStore`), never direct state access.
- No component imports another component's store selector — each component subscribes independently.

### 3.3 State Architecture (Zustand)

Four stores partition state by domain:

| Store           | Domain           | Key State                                   | IPC Usage |
|-----------------|------------------|---------------------------------------------|-----------|
| `appStore`      | UI chrome        | Theme, sidebar mode/collapsed, bottom panel, profiles, window dims | None (pure UI) |
| `sessionStore`  | Sessions & tabs  | Sessions[], openTabs[], activeTabId, splitPane, favorites, recents  | None (future) |
| `terminalStore` | Terminal instances| `Map<string, TerminalInstance>` — cols, rows, status, title        | None (read from events) |
| `vaultStore`    | Credential vault | vaultLocked, credentials[], loading, error  | `invoke()` for all vault ops |

**State flow:**
```
User Action → Store Action → set() → React re-render via selector hook
                                   ↘ invoke() → Rust backend (if side-effect needed)
                                                  ↓
Backend Event ← emit() ← Rust backend
     ↓
listen() → Store update or direct terminal.write()
```

### 3.4 Theme Architecture

Themes use a **three-layer indirection**:

1. **CSS Custom Properties** (`index.css :root`) — Runtime values, toggled by adding/removing `.light` class on `<html>`.
2. **Tailwind Config** (`tailwind.config.ts`) — Maps semantic class names (`bg-surface-primary`, `text-text-secondary`) to CSS variables (`var(--surface-primary)`).
3. **Theme JSON Files** (`themes/dark.json`, `light.json`) — Portable token definitions matching the `ThemeTokens` interface.

Theme switching is instant: `useEffect` in `App.tsx` toggles the `.light` class on `document.documentElement` and applies CSS variable overrides.

---

## 4. Backend Architecture (Rust)

### 4.1 Module Structure

```
src-tauri/src/
├── main.rs          # Tauri bootstrapper (calls lib::run())
├── lib.rs           # Plugin registration, state management, command registry
├── vault/mod.rs     # Encrypted credential vault (Argon2id + AES-256-GCM + SQLite)
├── ssh/mod.rs       # SSH client (russh), port forwarding, event streaming
├── terminal/mod.rs  # PTY management (portable-pty), reader threads
├── config/mod.rs    # Profile & session persistence (JSON files)
└── audit/mod.rs     # Append-only audit log (JSONL format)
```

### 4.2 State Management Pattern

Each module exposes a **state struct** that is registered as Tauri managed state:

```rust
// lib.rs
.manage(vault::Vault::new())
.manage(config::ConfigState::new())
.manage(terminal::TerminalManager::new())
.manage(ssh::SshState::new())
```

Commands access state via dependency injection:
```rust
#[tauri::command]
pub fn some_command(state: tauri::State<'_, MyState>) -> Result<T, MyError> { ... }
```

**Concurrency model by module:**

| Module     | Inner State Type             | Concurrency Strategy                |
|------------|------------------------------|-------------------------------------|
| `vault`    | `Mutex<Option<VaultInner>>`  | Single-threaded lock. Vault is either open or closed. |
| `ssh`      | `Arc<RwLock<HashMap<...>>>`  | Async (tokio). Multiple concurrent SSH sessions. `TokioMutex` per connection. |
| `terminal` | `Mutex<HashMap<...>>`        | Std mutex. Reader threads spawned per PTY via `std::thread::spawn`. |
| `config`   | `RwLock<...>`                | Std RwLock. Read-heavy (profile/session lookups), rare writes. |
| `audit`    | Stateless                    | File-append via `std::fs::OpenOptions`. No in-memory state. |

### 4.3 Vault Architecture

The credential vault implements a **defense-in-depth** model:

```
Master Password
     │
     ▼ Argon2id (64 MiB memory, 3 iterations, parallelism 4)
     │
     ▼ 256-bit Encryption Key (Zeroizing<Vec<u8>>)
     │
     ├──► AES-256-GCM encrypt(credential_json) → ciphertext + nonce
     │     stored in SQLite columns: encrypted_data BLOB, nonce BLOB
     │
     └──► Key held in pinned memory; zeroized on vault lock or Drop
```

**Security properties:**
- Credentials are **never stored or transmitted as plaintext**.
- The `CredentialSummary` type (returned by list operations) excludes secret material — only metadata.
- `CredentialDetail` (returned by explicit `credential_get`) includes decrypted data.
- The `encrypted_data` and `nonce` fields are `#[serde(skip_serializing)]` — never serialized to the frontend.
- Key material is `Zeroizing<Vec<u8>>` — memory-zeroized on drop.
- Salt is 32 bytes (256 bits), generated with `OsRng`.

### 4.4 SSH Architecture

The SSH module uses **russh** (pure-Rust async SSH2) with the following architecture:

```
Frontend                    Rust Backend
   │                            │
   ├── invoke("ssh_connect") ──►├── connect_and_auth()
   │                            │   ├── TCP connect
   │                            │   ├── Authenticate (password or key)
   │                            │   ├── Open channel + request PTY + shell
   │                            │   └── Spawn session task (tokio)
   │                            │
   │◄── emit("ssh:output") ────│←── SshClientHandler::data() callback
   │                            │
   ├── invoke("ssh_write") ────►├── cmd_tx.send(Write(data))
   │                            │         │
   │                            │         ▼ session task loop
   │                            │       channel.data(data)
   │                            │
   ├── invoke("ssh_resize") ──►├── cmd_tx.send(Resize{cols, rows})
   │                            │
   ├── invoke("ssh_disconnect")►├── cmd_tx.send(Close)
   │                            │
   │◄── emit("ssh:disconnected")│←── channel EOF / error
```

**Port forwarding** supports three modes:
- **Local**: `TcpListener::bind()` → forward to remote host via SSH channel.
- **Remote**: SSH `tcpip-forward` request → forward to local host.
- **Dynamic (SOCKS5)**: Local SOCKS5 proxy → SSH direct-tcpip channels.

### 4.5 Terminal (PTY) Architecture

Local terminal sessions use **portable-pty**:

```
Frontend                    Rust Backend
   │                            │
   ├── invoke("terminal_create")►├── pty_system.openpty()
   │                            │   ├── Spawn shell command
   │                            │   ├── Drop slave (master keeps PTY alive)
   │                            │   ├── Clone reader → spawn reader thread
   │                            │   └── Store writer Arc<Mutex<>>
   │                            │
   │◄── emit("terminal:output")│←── reader thread (4096-byte buffer loop)
   │                            │
   ├── invoke("terminal_write")►├── writer.write_all(data)
   │                            │
   ├── invoke("terminal_resize")├── master_pty.resize()
   │                            │
   │◄── emit("terminal:exit") ─│←── reader hits EOF
```

The reader thread runs a tight `loop { read() }` and emits UTF-8-lossy converted output. This keeps the async runtime free for SSH and other I/O.

### 4.6 Config / Persistence Architecture

```
~/.local/share/CrossTerm/          (Linux)
~/Library/Application Support/CrossTerm/  (macOS)
%APPDATA%/CrossTerm/               (Windows)
└── profiles/
    └── {profile_id}/
        ├── profile.json           # Profile metadata + settings
        ├── sessions.json          # Array of SessionDefinition
        ├── vault.db               # SQLite (encrypted)
        └── audit.jsonl            # Append-only audit log
```

- JSON files are the persistence format — human-readable, diffable, portable.
- Settings cascade: Session overrides → Folder defaults → Profile settings → App defaults.
- Config state uses `RwLock` for concurrent read access during session lookups.

### 4.7 Audit Log Architecture

The audit module implements an **append-only JSONL** (JSON Lines) log:

- One event per line, each a `AuditEvent { timestamp, event_type, details }`.
- Event types: vault lock/unlock/create, credential CRUD, session connect/disconnect, profile switch, settings update.
- Read operations parse the file line-by-line with optional filtering and pagination.
- CSV export for compliance/review.
- **No deletion** of individual events — the log is immutable by design.

---

## 5. IPC Contract

### 5.1 Commands (Frontend → Backend)

All commands are registered in `lib.rs::invoke_handler`. Naming convention: `{module}_{action}`.

| Module     | Commands                                                                |
|------------|-------------------------------------------------------------------------|
| `vault`    | `vault_create`, `vault_unlock`, `vault_lock`, `vault_is_locked`, `credential_create`, `credential_list`, `credential_get`, `credential_update`, `credential_delete` |
| `config`   | `profile_list`, `profile_create`, `profile_get`, `profile_update`, `profile_delete`, `profile_switch`, `session_list`, `session_create`, `session_get`, `session_update`, `session_delete`, `session_search`, `settings_get`, `settings_update` |
| `terminal` | `terminal_create`, `terminal_write`, `terminal_resize`, `terminal_close`, `terminal_list` |
| `ssh`      | `ssh_connect`, `ssh_disconnect`, `ssh_write`, `ssh_resize`, `ssh_list_connections`, `ssh_port_forward_add`, `ssh_port_forward_remove` |
| `audit`    | `audit_log_list`, `audit_log_export_csv`                                |

### 5.2 Events (Backend → Frontend)

| Event Name           | Payload                                    | Source Module |
|----------------------|--------------------------------------------|---------------|
| `terminal:output`    | `{ terminal_id, data }`                    | `terminal`    |
| `terminal:exit`      | `{ terminal_id, code }`                    | `terminal`    |
| `ssh:output`         | `{ connection_id, data }`                  | `ssh`         |
| `ssh:disconnected`   | `{ connection_id, reason }`                | `ssh`         |
| `ssh:connected`      | `{ connection_id }`                        | `ssh`         |

### 5.3 Error Handling Contract

Every Tauri command returns `Result<T, ModuleError>`. Error types are:
- Defined per module (`VaultError`, `SshError`, `TerminalError`, `ConfigError`, `AuditError`).
- Implement `Serialize` by serializing to their `Display` string.
- Implement `thiserror::Error` for structured error variants.
- Frontend catches errors via `.catch()` or try/catch on `invoke()` and stores them in the relevant store's `error` field.

---

## 6. Security Architecture

### 6.1 Trust Boundary

```
┌─────────────────────────────────┐
│     UNTRUSTED: WebView          │ ← No access to filesystem, processes,
│     (React frontend)            │    or raw sockets. Sandboxed by Tauri.
├─────────────────────────────────┤
│     TRUST BOUNDARY (IPC)        │ ← Commands are the only gate.
├─────────────────────────────────┤
│     TRUSTED: Rust Backend       │ ← Full system access: PTY, SSH, vault,
│     (src-tauri/src/)            │    filesystem, network.
└─────────────────────────────────┘
```

### 6.2 Security Controls

| Control                    | Implementation                                                |
|----------------------------|---------------------------------------------------------------|
| Credential encryption      | AES-256-GCM with Argon2id-derived keys                       |
| Memory protection          | `zeroize` crate — secrets zeroized on drop                    |
| Secret serialization guard | `#[serde(skip_serializing)]` on encrypted_data and nonce      |
| SSH key exchange           | Enforced modern ciphers via russh config                      |
| TOFU host key verification | MVP: accept-all. **TODO**: known_hosts integration            |
| Capability permissions     | Tauri capabilities file restricts plugin access               |
| No plaintext on disk       | Vault DB encrypted; credentials never written as plaintext    |
| Append-only audit          | Audit log is write-only; no delete/modify operations          |

### 6.3 Known Security Gaps (MVP)

1. **SSH host key verification**: Currently TOFU (Trust On First Use) with accept-all. Must implement `known_hosts` checking before production.
2. **CSP not configured**: `tauri.conf.json` has `"csp": null`. Should be locked down.
3. **No idle vault lock timer**: The `idle_timeout_secs` setting exists but is not yet wired to the auto-lock mechanism.

---

## 7. Build & Deployment

### 7.1 Build Pipeline

```
npm run build          → Vite bundles React app to dist/
cargo tauri build      → Compiles Rust backend + packages with WebView
                         Outputs: .dmg (macOS), .msi (Windows),
                                  .AppImage/.deb (Linux)
```

### 7.2 Development Pipeline

```
npm run dev            → Vite dev server on :1420 with HMR
cargo tauri dev        → Launches Tauri window pointing to dev server
                         Hot-reloads frontend; restarts backend on Rust changes
```

### 7.3 Dependency Summary

**Frontend** (package.json): 13 runtime dependencies, 10 dev dependencies.
**Backend** (Cargo.toml): 20 crate dependencies including `russh`, `aes-gcm`, `argon2`, `portable-pty`, `rusqlite`.

---

## 8. Scalability Considerations

| Dimension              | Current Design                                | Growth Path                        |
|------------------------|----------------------------------------------|------------------------------------|
| Concurrent sessions    | HashMap per module; single-process            | Sufficient for 50+ concurrent sessions |
| Terminal throughput     | 4096-byte read buffer; `from_utf8_lossy`      | Target: ≥80 MB/s (per spec)       |
| Session storage        | JSON files                                    | Migrate to SQLite if >1000 sessions |
| Plugin system           | Not yet implemented                           | WASM runtime (wasmtime) planned    |
| Multi-window           | Single Tauri window                           | Tauri multi-window API available   |

# CrossTerm — Gap Analysis & Remediation Plan

| Field            | Value                     |
|------------------|---------------------------|
| Spec Reference   | SPEC-CROSSTERM-001 v1.0   |
| Analysis Date    | 2025-04-05                |
| Scope            | Phase 1 MVP (§20)         |
| Overall Coverage | **~72%**                  |

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Phase 1 MVP Scope Reminder](#2-phase-1-mvp-scope-reminder)
3. [Gap Register — Rust Backend](#3-gap-register--rust-backend)
4. [Gap Register — React Frontend](#4-gap-register--react-frontend)
5. [Gap Register — Integration & Wiring](#5-gap-register--integration--wiring)
6. [Gap Register — Security](#6-gap-register--security)
7. [Gap Register — Build, CI/CD & Packaging](#7-gap-register--build-cicd--packaging)
8. [Remediation Plan — Sprints](#8-remediation-plan--sprints)
9. [Detailed Testing Requirements](#9-detailed-testing-requirements)
10. [Appendix A — Full Gap Checklist](#appendix-a--full-gap-checklist)

---

## 1. Executive Summary

The implementation has progressed significantly from a compilable skeleton to a **functional MVP candidate**. Core subsystems — SSH terminal (frontend + backend), SFTP browser, credential vault, session tree, split panes, and tab management — are wired end-to-end. 52 of the original gaps have been resolved. The remaining gaps are mostly polish, security hardening, and Phase 2 features.

| Category | Implemented | Present but unstubbed | Missing entirely |
|----------|:-----------:|:---------------------:|:----------------:|
| Local Shell | ✅ | — | — |
| SSH Terminal (backend) | ✅ keep-alive, startup script, exec | Jump hosts, agent fwd | — |
| SSH Terminal (frontend wiring) | ✅ SshTerminalView + QuickConnect | Auto-reconnect UI | — |
| SFTP Browser (backend) | ✅ 14 commands, progress events | SCP fallback, throttling | — |
| SFTP Browser (frontend) | ✅ real ops, context menu, progress | Drag-and-drop | — |
| Credential Vault (backend) | ✅ AES-256-GCM + pw change + rate limit | Auto-lock enforcement | Biometric |
| Credential Vault (frontend) | ✅ wired in App.tsx | — | — |
| Session Tree | ✅ hierarchical, search, favorites, recent | Tag filter chips | — |
| Tab Management | ✅ context menu, scroll, middle-click | Detach, multi-window | — |
| Split Panes | ✅ rendering + drag resize | Keyboard nav, broadcast | — |
| Theming (dark/light) | ✅ toggle + OS auto-follow + reduced-motion | Theme file loading | Additional themes |
| Audit Log | ✅ triggered across modules | — | — |
| First-Launch Wizard | — | — | ❌ |
| Testing | ✅ 96 tests (47 Rust + 49 Frontend) | Integration, E2E, fuzz | — |

**Bottom line**: 52 of the original gaps resolved. 2 P1-BLOCKERs remain (jump host, agent forwarding). 65 gaps remain, mostly P1-MEDIUM/LOW and P2.

---

## 2. Phase 1 MVP Scope Reminder

Per SPEC-CROSSTERM-001 §20.1, Phase 1 must deliver:

1. Local shell terminal
2. SSH terminal (shell, exec, port forwarding, agent forwarding, jump hosts)
3. SFTP browser (dual-pane, drag-and-drop, queue, resume)
4. Credential vault with master password
5. Multi-profile support
6. Session tree with folders, tags, search
7. Split panes & tab management
8. Basic theming (dark/light)
9. Windows + macOS + Linux desktop builds
10. xterm.js GPU-rendered terminal

Cross-cutting concerns that apply to Phase 1: Security (§12), Audit (§12.4), Accessibility (§10.12), i18n foundation (§10.13), Performance targets (§16), Testing (§17).

---

## 3. Gap Register — Rust Backend

### 3.1 SSH Module (`src-tauri/src/ssh/mod.rs`)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-SSH-01 | Jump host / ProxyJump not implemented (`_jump_host` param is unused) | §5.1 | **P1-BLOCKER** | Code stub |
| BE-SSH-02 | SSH agent forwarding not implemented | §5.1 | **P1-BLOCKER** | Missing |
| BE-SSH-03 | Remote port forwarding incomplete — `tcpip_forward()` called but no incoming connection listener spawned | §9.3 | P1-HIGH | Partial |
| BE-SSH-04 | Keep-alive heartbeat not wired — `keep_alive_interval_seconds` stored in session but never used by SSH runtime | §5.2 | P1-HIGH | ✅ Done |
| BE-SSH-05 | Startup script never executed on connect | §5.2 | P1-HIGH | ✅ Done |
| BE-SSH-06 | Known-hosts verification is TOFU only — no persistent known_hosts file | §12.2 | P1-HIGH | TODO in code |
| BE-SSH-07 | SSH cipher/kex algorithm policy not enforced — spec requires curve25519-sha256 minimum | §12.2 | P1-MEDIUM | Uses russh defaults |
| BE-SSH-08 | `last_connected_at` never updated on successful connect | §5.2 | P1-MEDIUM | ✅ Done |
| BE-SSH-09 | Connection state (cipher, latency, protocol version) not exposed to frontend | §10.3/F | P1-LOW | Missing fields |
| BE-SSH-10 | SOCKS5 dynamic forwarding doesn't handle IPv6 (type 0x04) | §9.3 | P2 | Partial |
| BE-SSH-11 | `exec` mode (one-off command execution) not exposed as a command | §5.1 | P1-MEDIUM | ✅ Done |

### 3.2 SFTP Module (MISSING)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-SFTP-01 | **Entire SFTP backend module does not exist** — no `russh-sftp` dependency, no file listing, no upload/download | §14.1 | **P1-BLOCKER** | ✅ Done |
| BE-SFTP-02 | No SCP transfer commands | §14.2 | P1-HIGH | Missing |
| BE-SFTP-03 | No bandwidth throttling infrastructure | §14.1 | P1-MEDIUM | Missing |
| BE-SFTP-04 | No transfer queue/progress tracking | §14.1 | P1-HIGH | ✅ Done |

### 3.3 Credential Vault (`src-tauri/src/vault/mod.rs`)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-VAULT-01 | Auto-lock timeout never enforced — `idle_timeout_secs` field exists but no background timer checks it | §4.3 | P1-HIGH | Infrastructure only |
| BE-VAULT-02 | No `vault_change_password` command — users cannot rotate the master password | §4.3 | P1-HIGH | ✅ Done |
| BE-VAULT-03 | No rate-limiting on `vault_unlock` attempts — brute-force risk | §12 | P1-HIGH | ✅ Done |
| BE-VAULT-04 | Verification token uses hardcoded `b"crossterm-vault-ok"` — predictable if DB leaked | §12.1 | P1-MEDIUM | Weak |
| BE-VAULT-05 | No credential-to-session orphan check on delete | §4.3 | P1-MEDIUM | Missing |
| BE-VAULT-06 | Clipboard auto-clear after 30s not coordinated from backend | §4.3 | P1-LOW | Frontend duty |
| BE-VAULT-07 | Biometric unlock (Touch ID, Windows Hello) not implemented | §3.2 | P2 | Missing |
| BE-VAULT-08 | OS credential store delegation (Keychain, Credential Manager) not implemented | §3.2 | P2 | Missing |
| BE-VAULT-09 | FIDO2/WebAuthn hardware key support not implemented | §3.2 | P2 | Missing |

### 3.4 Terminal (`src-tauri/src/terminal/mod.rs`)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-TERM-01 | Session output logging to file (plaintext/HTML/raw) not implemented | §6.5 | P1-HIGH | Missing |
| BE-TERM-02 | Reader thread never gracefully joined on drop — potential thread leak | §6 | P1-MEDIUM | ✅ Done |
| BE-TERM-03 | No bell event emission (BEL/^G passthrough) | §10.8.1 | P1-LOW | Missing |
| BE-TERM-04 | `String::from_utf8_lossy()` silently drops invalid bytes — should support raw binary output mode | §6.5 | P1-LOW | Lossy |

### 3.5 Config (`src-tauri/src/config/mod.rs`)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-CFG-01 | No session duplication command | §5.4 | P1-MEDIUM | ✅ Done |
| BE-CFG-02 | No bulk "connect all in folder" command | §5.4 | P1-MEDIUM | Missing |
| BE-CFG-03 | No session import (PuTTY, MobaXterm, Termius, `~/.ssh/config`) | §3.3 | P1-HIGH | Missing |
| BE-CFG-04 | No profile export/import as `.crossterm-profile` encrypted archive | §3.3 | P1-HIGH | Missing |
| BE-CFG-05 | Settings hierarchy (session → folder → profile → app defaults) not implemented — flat settings only | §15.1 | P1-MEDIUM | Flat |
| BE-CFG-06 | No portable mode detection (`.crossterm-portable` sentinel file) | §15.3 | P1-LOW | Missing |

### 3.6 Audit (`src-tauri/src/audit/mod.rs`)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-AUDIT-01 | **Audit events never triggered** — `append_event()` exists but is never called from vault, config, terminal, or SSH modules | §12.4 | **P1-BLOCKER** | ✅ Done |

### 3.7 Missing Backend Modules

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BE-MOD-01 | No SFTP module (`src-tauri/src/sftp/`) | §14 | **P1-BLOCKER** | ✅ Done |
| BE-MOD-02 | No SSH key manager (generate, import, export, deploy to host) | §11.6 | P1-HIGH | Missing |
| BE-MOD-03 | Auto-updater not configured in `tauri.conf.json` | §2.2 | P1-MEDIUM | Missing |
| BE-MOD-04 | No CSP (Content Security Policy) in `tauri.conf.json` | §12.3 | P1-MEDIUM | ✅ Done |

---

## 4. Gap Register — React Frontend

### 4.1 Component Wiring (Components Exist But Never Rendered)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-WIRE-01 | `<VaultUnlock>` never mounted — vault lock screen never displayed | §4 | **P1-BLOCKER** | ✅ Done |
| FE-WIRE-02 | `<CredentialManager>` never accessible — no button opens vault management | §4 | **P1-BLOCKER** | ✅ Done |
| FE-WIRE-03 | `<CommandPalette>` never opened — Ctrl+Shift+P not bound in `App.tsx` | §6.4 | P1-HIGH | ✅ Done |
| FE-WIRE-04 | `<SettingsPanel>` never rendered — no Settings button wired | §15 | P1-HIGH | ✅ Done |
| FE-WIRE-05 | `<SessionEditor>` never opened — no "New Session" / "Edit" button triggers it | §5 | **P1-BLOCKER** | ✅ Done |
| FE-WIRE-06 | `<ToastProvider>` not mounted in App.tsx — `useToast()` hook will crash | §10.14.1 | P1-HIGH | ✅ Done |

### 4.2 SSH Frontend Integration (MISSING)

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-SSH-01 | **No SSH terminal rendering** — TerminalView only calls `terminal_write`/`terminal:output` (local PTY). No `ssh_connect` / `ssh_write` / `ssh:output` integration | §5.1 | **P1-BLOCKER** | ✅ Done |
| FE-SSH-02 | No SSH connection dialog (host, port, auth method, credential selection) | §5.1 | **P1-BLOCKER** | ✅ Done |
| FE-SSH-03 | No SSH disconnect handling / auto-reconnect UI (exponential backoff countdown) | §10.14.3 | P1-HIGH | Missing |

### 4.3 SFTP Frontend

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-SFTP-01 | SFTP browser uses **hardcoded mock data** — no real file listing | §14.1 | **P1-BLOCKER** | ✅ Done |
| FE-SFTP-02 | No drag-and-drop file transfer | §14.1 | P1-HIGH | Missing |
| FE-SFTP-03 | No transfer queue/progress UI | §14.1 | P1-HIGH | ✅ Done |
| FE-SFTP-04 | No file operations (rename, delete, chmod, new folder) | §14.1 | P1-HIGH | ✅ Done |
| FE-SFTP-05 | No breadcrumb navigation wired | §10.8.3 | P1-MEDIUM | ✅ Done |

### 4.4 Session Management UI

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-SESS-01 | Session tree renders **flat list only** — no hierarchical folder navigation | §5.3 | P1-HIGH | ✅ Done |
| FE-SESS-02 | No "Favorites" quick-access row at top of sidebar | §5.3 | P1-MEDIUM | ✅ Done |
| FE-SESS-03 | No "Recent" sessions section in sidebar | §5.3 | P1-MEDIUM | ✅ Done |
| FE-SESS-04 | Search bar not visible in sidebar (filter logic exists in component but no text input rendered) | §5.3 | P1-HIGH | ✅ Done |
| FE-SESS-05 | No tag-based filtering UI | §5.3 | P1-MEDIUM | Missing |
| FE-SESS-06 | No folder creation/rename/delete from sidebar context menu | §5.3 | P1-HIGH | ✅ Done |
| FE-SESS-07 | Session data not persisted to disk — all sessions lost on reload | §5 | **P1-BLOCKER** | ✅ Done |

### 4.5 Split Panes

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-SPLIT-01 | **Split pane rendering not implemented** — types and store state exist but zero UI | §11.1 | P1-HIGH | ✅ Done |
| FE-SPLIT-02 | No drag handle for pane resizing | §11.1 | P1-HIGH | ✅ Done |
| FE-SPLIT-03 | No keyboard navigation (Alt+Arrow) between panes | §11.1 | P1-MEDIUM | Missing |
| FE-SPLIT-04 | No "broadcast input to all panes" toggle | §11.1 | P1-MEDIUM | Missing |

### 4.6 Tab Management

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-TAB-01 | No tab detach into new window | §10.4.3 | P2 | Missing |
| FE-TAB-02 | No middle-click to close tab | §10.4.3 | P1-LOW | ✅ Done |
| FE-TAB-03 | No "+" button dropdown (New SSH, RDP, VNC, Local Shell, etc.) | §10.4.3 | P1-HIGH | ✅ Done |
| FE-TAB-04 | No tab context menu (Rename, Duplicate, Split Right, Split Down, etc.) | §10.4.3 | P1-MEDIUM | ✅ Done |
| FE-TAB-05 | Tab scroll overflow when many tabs open — no scroll arrows or "…" dropdown | §10.4.3 | P1-MEDIUM | ✅ Done |

### 4.7 Terminal Features

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-TERM-01 | No right-click context menu (Copy, Paste, Select All, Clear, Search, URL open) | §10.8.1 | P1-HIGH | ✅ Done |
| FE-TERM-02 | No multi-line paste confirmation dialog | §10.4.4 | P1-MEDIUM | Missing |
| FE-TERM-03 | No terminal search UI (Ctrl+Shift+F) | §6.3 | P1-MEDIUM | ✅ Done |
| FE-TERM-04 | No URL click handling (Ctrl+click to open browser) | §6.3 | P1-LOW | Missing |
| FE-TERM-05 | No bell handling (visual flash / audio / notification) | §10.8.1 | P1-LOW | Missing |
| FE-TERM-06 | No cursor style configuration UI | §10.8.1 | P1-LOW | Missing |

### 4.8 Theming

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-THEME-01 | No OS dark/light mode auto-follow (`prefers-color-scheme`) | §10.9.3 | P1-HIGH | ✅ Done |
| FE-THEME-02 | No theme file loading from `.crossterm-theme` JSON | §10.9.2 | P1-MEDIUM | Missing |
| FE-THEME-03 | Only 2 themes shipped (Dark/Light) — spec requires 8 themes | §10.9.1 | P1-LOW | Partial |
| FE-THEME-04 | No `prefers-reduced-motion` handling | §10.2.5 | P1-MEDIUM | ✅ Done |

### 4.9 Accessibility

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-A11Y-01 | No ARIA roles on UI regions (`navigation`, `main`, `complementary`, `status`) | §10.12.2 | P1-HIGH | ✅ Done |
| FE-A11Y-02 | No visible focus rings on interactive elements | §10.12.2 | P1-HIGH | ✅ Done |
| FE-A11Y-03 | No `aria-live` regions for connection state changes | §10.12.2 | P1-MEDIUM | Missing |
| FE-A11Y-04 | Status dots use colour alone (need shape supplement: ●/◌/◉/○) | §10.12.2 | P1-MEDIUM | Partial |

### 4.10 i18n

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-I18N-01 | Hardcoded strings in TerminalTab.tsx, App.tsx, SettingsPanel.tsx | §10.13 | P1-MEDIUM | ~20 strings |
| FE-I18N-02 | No ICU plural format usage | §10.13 | P1-LOW | Missing |
| FE-I18N-03 | No RTL layout support | §10.13 | P2 | Missing |
| FE-I18N-04 | No `Intl.DateTimeFormat` / `Intl.NumberFormat` usage | §10.13 | P1-LOW | Missing |

### 4.11 Other Missing Frontend Features

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| FE-MISC-01 | No first-launch wizard | §10.10.1 | P1-HIGH | Missing |
| FE-MISC-02 | No responsive breakpoint layout switching (Compact/Medium/Expanded/Large) | §10.11 | P1-MEDIUM | Only sidebar collapse |
| FE-MISC-03 | No notification history panel | §11.8 | P2 | Missing |
| FE-MISC-04 | No Snippets manager UI | §6.4 | P2 | Missing |
| FE-MISC-05 | Keyboard shortcuts incomplete — only Ctrl+J wired in App.tsx | §10.4.4 | P1-HIGH | ✅ Done |

---

## 5. Gap Register — Integration & Wiring

These gaps represent disconnects between existing frontend components and backend commands.

| ID | Gap | Details | Severity | Status |
|----|-----|---------|----------|--------|
| INT-01 | **Session persistence** — sessions created in frontend store are never saved to backend (no `invoke("session_create")` calls) | Data lost on reload | **P1-BLOCKER** | ✅ Done |
| INT-02 | **Profile lifecycle** — no profile creation/switching flow in UI. `profile_create`, `profile_switch` commands never invoked | §3.1 | **P1-BLOCKER** | ✅ Done |
| INT-03 | **SSH session lifecycle** — clicking "Connect" on an SSH session doesn't call `ssh_connect`, doesn't open an SSH-aware TerminalView | §5.1 | **P1-BLOCKER** | ✅ Done |
| INT-04 | **Audit event triggers** — vault, config, terminal, SSH modules never call `audit::append_event()` | §12.4 | P1-HIGH | ✅ Done |
| INT-05 | **Settings persistence** — frontend store settings never loaded from backend on startup, never saved back | §15 | P1-HIGH | Missing |
| INT-06 | **Theme persistence** — selected theme not loaded from backend settings on startup | §10.9 | P1-MEDIUM | Missing |
| INT-07 | **Vault auto-lock** — no frontend timer to detect idle and call `vault_lock` | §4.3 | P1-HIGH | ✅ Done |
| INT-08 | **Session status sync** — backend disconnect events (terminal:exit, ssh:disconnected) don't update tab status dots | Tab stays "connected" | P1-HIGH | Missing |
| INT-09 | **Terminal dimensions** — status bar shows "80×24" hardcoded; should read from TerminalView fit result | §10.3/F | P1-LOW | ✅ Done |

---

## 6. Gap Register — Security

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| SEC-01 | No Content Security Policy in `tauri.conf.json` (`security.csp: null`) | §12.3 | P1-HIGH | ✅ Done |
| SEC-02 | No vault unlock rate limiting — unlimited password attempts | §12 | P1-HIGH | ✅ Done |
| SEC-03 | SSH TOFU — no persistent `known_hosts` verification | §12.2 | P1-HIGH | TODO |
| SEC-04 | No SSH cipher/kex policy enforcement | §12.2 | P1-MEDIUM | Missing |
| SEC-05 | Verification token `b"crossterm-vault-ok"` is predictable | §12.1 | P1-MEDIUM | Weak |
| SEC-06 | Vault DB metadata (table structure) is plaintext — not full SQLCipher-level encryption | §4.1 | P1-LOW | By design |
| SEC-07 | No dependency audit in CI (`cargo audit`, `npm audit`) | §17.4 | P1-MEDIUM | Missing |
| SEC-08 | No Clippy `deny` level in CI | §17.4 | P1-MEDIUM | Missing |
| SEC-09 | No SBOM generation (CycloneDX) | §19 | P1-LOW | Missing |

---

## 7. Gap Register — Build, CI/CD & Packaging

| ID | Gap | Spec § | Severity | Status |
|----|-----|--------|----------|--------|
| BLD-01 | No application icons (`icons/` directory) — tauri.conf.json references missing files | §18 | P1-HIGH | Missing |
| BLD-02 | CI workflow exists but never tested — may fail on Windows/Linux | §2.3 | P1-MEDIUM | Unverified |
| BLD-03 | No code signing configuration (Authenticode, notarisation, APK signing) | §2.3 | P1-MEDIUM | Missing |
| BLD-04 | No Tauri auto-updater configuration | §2.2 | P1-MEDIUM | Missing |
| BLD-05 | No shell integration script (CWD tracking, command duration) | §18.2 | P2 | Missing |
| BLD-06 | No `.desktop` file for Linux | §10.6.3 | P1-LOW | Missing |
| BLD-07 | No Homebrew cask formula for macOS | §18.1 | P2 | Missing |

---

## 8. Remediation Plan — Sprints

### Sprint 1: Foundation Wiring (Critical Blockers)

**Goal**: Make the existing code actually work end-to-end. No new features — just wire what exists.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Mount `<ToastProvider>`, `<VaultUnlock>`, `<CommandPalette>` in App.tsx; wire Ctrl+Shift+P hotkey | FE-WIRE-01, FE-WIRE-03, FE-WIRE-06 | S |
| Wire "New Session" button → `<SessionEditor>` modal; wire "Settings" → `<SettingsPanel>` | FE-WIRE-05, FE-WIRE-04 | S |
| Wire sidebar "Credentials" option → `<CredentialManager>` | FE-WIRE-02 | S |
| Add session persistence — call `invoke("session_create/update/delete")` from sessionStore, load sessions on startup | INT-01, FE-SESS-07 | M |
| Add profile creation/switching UI flow in App.tsx | INT-02 | M |
| Wire vault auto-lock timer in frontend (idle detection → `invoke("vault_lock")`) | INT-07, BE-VAULT-01 | S |
| Add audit event calls to vault, config, terminal, ssh modules | BE-AUDIT-01, INT-04 | M |
| Update `last_connected_at` on successful connect | BE-SSH-08 | S |
| Add vault unlock rate-limiting (3 attempts → exponential backoff) | BE-VAULT-03, SEC-02 | S |
| Set CSP in tauri.conf.json | SEC-01 | S |

### Sprint 2: SSH End-to-End

**Goal**: Complete SSH terminal sessions including frontend integration.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Create `SshTerminalView` component that calls `ssh_connect`, listens to `ssh:output`, writes via `ssh_write` | FE-SSH-01 | L |
| SSH connection dialog (host, port, auth method, credential picker, key file upload) | FE-SSH-02 | M |
| Implement known_hosts file verification (persist fingerprints, prompt on unknown/changed) | BE-SSH-06, SEC-03 | M |
| Implement jump host / ProxyJump (nested SSH connection) | BE-SSH-01 | L |
| Implement SSH agent forwarding via `russh` agent API | BE-SSH-02 | M |
| Complete remote port forwarding — spawn listener for incoming connections | BE-SSH-03 | M |
| Wire keep-alive heartbeat pings | BE-SSH-04 | S |
| Execute startup_script on connection established | BE-SSH-05 | S |
| Auto-reconnect UI with exponential backoff countdown | FE-SSH-03 | M |
| Expose connection info (cipher, latency) to frontend via `ssh:connected` event | BE-SSH-09 | S |
| Add SSH exec command (one-off command execution) | BE-SSH-11 | S |
| Enforce SSH cipher/kex policy (config override) | BE-SSH-07, SEC-04 | S |

### Sprint 3: SFTP Browser

**Goal**: Full SFTP file browser with real backend operations.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Add `russh-sftp` dependency to Cargo.toml | BE-SFTP-01 | S |
| Create `src-tauri/src/sftp/mod.rs` — SFTP session manager (open, close, list, stat, read, write, mkdir, rm, rename, chmod) | BE-SFTP-01 | L |
| SFTP transfer queue with progress events (`sftp:transfer_progress`, `sftp:transfer_complete`) | BE-SFTP-04 | M |
| Add SCP fallback for legacy hosts | BE-SFTP-02 | M |
| Bandwidth throttling per-transfer | BE-SFTP-03 | S |
| Rewrite `SftpBrowser.tsx` — real file listing from `invoke("sftp_list")` | FE-SFTP-01 | L |
| Breadcrumb navigation wired to directory changes | FE-SFTP-05 | S |
| File operations context menu (rename, delete, chmod, new folder) | FE-SFTP-04 | M |
| Drag-and-drop between panes + from OS file manager | FE-SFTP-02 | M |
| Transfer queue panel with progress bars, pause/resume/cancel | FE-SFTP-03 | M |

### Sprint 4: Session Tree, Split Panes, Tabs

**Goal**: Complete session management UI and workspace layout features.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Hierarchical folder tree rendering (collapsible nodes, nested groups) | FE-SESS-01 | M |
| Folder CRUD from context menu (new folder, rename, delete, move session) | FE-SESS-06 | M |
| Search bar in sidebar with real-time filter | FE-SESS-04 | S |
| Favorites quick-access row | FE-SESS-02 | S |
| Recent sessions section | FE-SESS-03 | S |
| Tag filter chips | FE-SESS-05 | S |
| Session duplication command | BE-CFG-01 | S |
| Bulk connect all in folder | BE-CFG-02 | S |
| Split pane rendering with resizable drag handles | FE-SPLIT-01, FE-SPLIT-02 | L |
| Alt+Arrow pane navigation | FE-SPLIT-03 | S |
| Broadcast input toggle | FE-SPLIT-04 | S |
| Tab "+" dropdown menu (session type picker) | FE-TAB-03 | S |
| Tab context menu (Rename, Duplicate, Split Right/Down, Close Others) | FE-TAB-04 | M |
| Tab scroll overflow arrows / "…" dropdown | FE-TAB-05 | S |
| Middle-click closes tab | FE-TAB-02 | S |
| Session import from PuTTY / MobaXterm / Termius / `~/.ssh/config` | BE-CFG-03 | L |
| Profile export/import as `.crossterm-profile` encrypted archive | BE-CFG-04 | M |

### Sprint 5: UX Polish, Accessibility, Theming

**Goal**: Production-quality UX and accessibility compliance.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| First-launch wizard (6 steps per §10.10.1) | FE-MISC-01 | L |
| OS dark/light auto-follow via `prefers-color-scheme` | FE-THEME-01 | S |
| Theme file loader (`.crossterm-theme` JSON import) | FE-THEME-02 | M |
| Ship 6 additional themes (Solarized Dark/Light, Dracula, Nord, Monokai Pro, High Contrast) | FE-THEME-03 | M |
| `prefers-reduced-motion` support — disable all animations | FE-THEME-04 | S |
| ARIA roles on all layout regions | FE-A11Y-01 | M |
| Visible focus rings (2px border-focus token) on all interactive elements | FE-A11Y-02 | M |
| `aria-live` regions for connection changes, transfer progress, toasts | FE-A11Y-03 | S |
| Status dot shapes (● ◌ ◉ ○) supplement to colour | FE-A11Y-04 | S |
| Responsive breakpoint layout switching (Compact/Medium/Expanded/Large) | FE-MISC-02 | M |
| Terminal right-click context menu | FE-TERM-01 | M |
| Multi-line paste confirmation dialog | FE-TERM-02 | S |
| Terminal search overlay (Ctrl+Shift+F) | FE-TERM-03 | S |
| Terminal bell handling (visual flash, audio, notification) | FE-TERM-05 | S |
| Wire all keyboard shortcuts per §10.4.4 (Ctrl+Tab, Ctrl+1-9, Alt+Arrow, etc.) | FE-MISC-05 | M |
| Externalize remaining hardcoded strings to i18n | FE-I18N-01 | S |
| Add ICU plural support where relevant | FE-I18N-02 | S |

### Sprint 6: Security Hardening, Build & Packaging

**Goal**: Ship-ready security posture, CI/CD, and installers.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Generate application icons (all sizes/formats) | BLD-01 | S |
| Vault change-password command with re-encryption of all credentials | BE-VAULT-02 | M |
| Randomize vault verification token per encryption key | SEC-05 | S |
| Settings hierarchy (session → folder → profile → defaults) | BE-CFG-05 | M |
| Session output logging to file (plaintext/HTML) | BE-TERM-01 | M |
| SSH key manager (generate, import, deploy to host) | BE-MOD-02 | L |
| `cargo audit` + `npm audit` in CI | SEC-07 | S |
| Clippy at `deny` level in CI | SEC-08 | S |
| SBOM generation (CycloneDX) in release workflow | SEC-09 | S |
| Tauri auto-updater configuration | BE-MOD-03, BLD-04 | S |
| Code signing config for all platforms | BLD-03 | M |
| Linux `.desktop` file | BLD-06 | S |
| Fix terminal reader thread cleanup | BE-TERM-02 | S |
| Portable mode detection | BE-CFG-06 | S |

### Sprint 7: Testing

**Goal**: Achieve spec-mandated test coverage. See §9 below for full details.

| Task | Gaps Addressed | Effort |
|------|---------------|--------|
| Rust unit tests — vault (target 80% coverage) | §17.1 | L |
| Rust unit tests — config | §17.1 | M |
| Rust unit tests — audit | §17.1 | S |
| Rust unit tests — terminal | §17.1 | M |
| Rust unit tests — SSH | §17.1 | L |
| Rust unit tests — SFTP | §17.1 | L |
| Docker-based integration tests (OpenSSH server) | §17.2 | L |
| Frontend unit tests (Vitest + React Testing Library) | §17 | L |
| E2E tests (Playwright / WebdriverIO) | §17.3 | XL |
| Credential vault fuzz testing (`cargo fuzz`) | §17.4 | M |
| Performance benchmarks (§16 targets) | §16 | M |

---

## 9. Detailed Testing Requirements

### 9.1 Rust Unit Tests

**Coverage target**: ≥ 80% line coverage on all backend modules.

**Framework**: `#[cfg(test)]` + `cargo test`, with `cargo-tarpaulin` for coverage reporting.

#### 9.1.1 Vault Module Tests (`src-tauri/src/vault/`)

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-V-01 | `test_vault_create_and_unlock` | Create a vault with a master password, lock it, then unlock with the same password. Assert vault is accessible after unlock. | P0 |
| UT-V-02 | `test_vault_wrong_password` | Create a vault, lock it, attempt unlock with wrong password. Assert `VaultError::InvalidPassword`. | P0 |
| UT-V-03 | `test_credential_roundtrip_password` | Create a Password credential, retrieve it, verify all fields (username, password, domain) match. | P0 |
| UT-V-04 | `test_credential_roundtrip_ssh_key` | Create an SSHKey credential with a passphrase, retrieve it, verify private key and passphrase match. | P0 |
| UT-V-05 | `test_credential_roundtrip_certificate` | Create a Certificate credential (X.509 + private key), retrieve, verify. | P0 |
| UT-V-06 | `test_credential_roundtrip_api_token` | Create an API Token credential with expiry date, retrieve, verify. | P0 |
| UT-V-07 | `test_credential_roundtrip_cloud` | Create a CloudCredential (AWS type), retrieve, verify all fields. | P0 |
| UT-V-08 | `test_credential_roundtrip_totp` | Create a TOTP credential, retrieve, verify secret/issuer/digits/period. | P0 |
| UT-V-09 | `test_credential_update` | Create a credential, update its password, retrieve and verify updated value. | P0 |
| UT-V-10 | `test_credential_delete` | Create a credential, delete it, attempt retrieval. Assert `VaultError::CredentialNotFound`. | P0 |
| UT-V-11 | `test_credential_list` | Create 5 credentials of different types, list all. Assert count = 5, verify summary fields (no secrets returned in list). | P0 |
| UT-V-12 | `test_vault_locked_operations` | Lock the vault, attempt CRUD operations. Assert `VaultError::VaultLocked` for all. | P0 |
| UT-V-13 | `test_encryption_produces_different_ciphertexts` | Encrypt the same plaintext twice, assert the ciphertexts differ (nonce uniqueness). | P1 |
| UT-V-14 | `test_argon2id_parameters` | Verify Argon2id derivation uses m=65536, t=3, p=4, output_len=32. | P1 |
| UT-V-15 | `test_vault_change_password` | Create vault, add credentials, change master password, unlock with new password, verify credentials still accessible. | P0 |
| UT-V-16 | `test_rate_limiting` | Attempt 10 rapid unlock calls with wrong password. Assert rate-limit or backoff after 3 failures. | P1 |
| UT-V-17 | `test_idle_timeout_lock` | Set idle timeout to 1 second, wait 2 seconds, assert vault is auto-locked. | P1 |
| UT-V-18 | `test_zeroize_on_lock` | Lock vault, inspect (via unsafe) that encryption key memory is zeroed. | P2 |
| UT-V-19 | `test_concurrent_access` | Spawn 10 tokio tasks accessing credentials simultaneously. Assert no panics or data corruption. | P1 |
| UT-V-20 | `test_empty_vault_operations` | Perform list/get/update/delete on an empty vault. Assert correct empty results and errors. | P0 |

#### 9.1.2 Config Module Tests (`src-tauri/src/config/`)

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-C-01 | `test_profile_crud` | Create, list, get, update, delete a profile. Verify all operations. | P0 |
| UT-C-02 | `test_session_crud` | Create, list, get, update, delete a session within a profile. | P0 |
| UT-C-03 | `test_session_search_by_name` | Create 5 sessions with distinct names, search for one by name. Assert exactly 1 result. | P0 |
| UT-C-04 | `test_session_search_by_host` | Search sessions by hostname substring. Verify matching. | P0 |
| UT-C-05 | `test_session_search_by_tag` | Create sessions with tags, search by tag. Verify matching. | P0 |
| UT-C-06 | `test_session_search_by_group` | Search by folder/group path. | P0 |
| UT-C-07 | `test_session_duplicate` | Duplicate a session. Verify new UUID, same name + "(Copy)", same connection details. | P1 |
| UT-C-08 | `test_profile_switch` | Create 2 profiles, switch between them. Verify active profile changes. | P0 |
| UT-C-09 | `test_settings_persistence` | Update settings, reload, verify settings persisted. | P0 |
| UT-C-10 | `test_settings_defaults` | Get settings without prior writes. Verify all defaults match spec (theme="dark", scrollback=10000, etc.). | P0 |
| UT-C-11 | `test_session_all_types` | Create one session of each SessionType (13 types). Verify all persist and deserialize correctly. | P0 |
| UT-C-12 | `test_session_protocol_options` | Store SSH-specific protocol_options (cipher preference, etc.) and VNC options (encoding). Verify round-trip. | P1 |
| UT-C-13 | `test_last_connected_at_update` | Update `last_connected_at` on a session. Verify timestamp persists. | P0 |
| UT-C-14 | `test_profile_data_isolation` | Create sessions under profile A. Switch to profile B. List sessions. Assert empty (profile isolation). | P0 |
| UT-C-15 | `test_session_import_ssh_config` | Provide a sample `~/.ssh/config` file. Import sessions. Verify correct host/port/user/key mapping. | P1 |

#### 9.1.3 Audit Module Tests (`src-tauri/src/audit/`)

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-A-01 | `test_append_and_list` | Append 3 audit events, list all. Assert count = 3, correct order. | P0 |
| UT-A-02 | `test_filter_by_event_type` | Append mixed event types. Filter by `VaultUnlock`. Verify only matching events returned. | P0 |
| UT-A-03 | `test_offset_and_limit` | Append 20 events. Request offset=5, limit=10. Verify correct slice. | P0 |
| UT-A-04 | `test_csv_export` | Append events, export as CSV. Parse CSV and verify headers and row count. | P0 |
| UT-A-05 | `test_empty_audit_log` | List events with no log file. Assert empty result (no error). | P0 |
| UT-A-06 | `test_concurrent_append` | Spawn 10 tasks appending events simultaneously. List all. Verify count = 10, no corruption. | P1 |

#### 9.1.4 Terminal Module Tests (`src-tauri/src/terminal/`)

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-T-01 | `test_terminal_create` | Create a local terminal. Assert terminal ID is returned and listed in `terminal_list`. | P0 |
| UT-T-02 | `test_terminal_write_read` | Create terminal, write `echo hello\n`, capture output. Assert "hello" appears in output. | P0 |
| UT-T-03 | `test_terminal_resize` | Create terminal, resize to 120×40. Assert no error. | P0 |
| UT-T-04 | `test_terminal_close` | Create terminal, close it. Assert removed from `terminal_list`. | P0 |
| UT-T-05 | `test_multiple_terminals` | Create 5 terminals. Verify all listed. Close 2. Verify 3 remain. | P0 |
| UT-T-06 | `test_custom_shell` | Create terminal with /bin/sh (or cmd.exe on Windows). Verify shell spawned. | P1 |
| UT-T-07 | `test_custom_env` | Create terminal with `FOO=bar` in environment. Write `echo $FOO\n`. Assert "bar" in output. | P1 |
| UT-T-08 | `test_custom_cwd` | Create terminal with `cwd=/tmp`. Write `pwd\n`. Assert "/tmp" in output. | P1 |
| UT-T-09 | `test_terminal_exit_event` | Create terminal, write `exit\n`. Assert `terminal:exit` event is emitted. | P0 |

#### 9.1.5 SSH Module Tests (`src-tauri/src/ssh/`)

**Note**: SSH tests require either a mock SSH server or Docker-based test fixtures (see §9.3).

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-S-01 | `test_ssh_connect_password` | Connect to test SSH server with password auth. Assert success. | P0 |
| UT-S-02 | `test_ssh_connect_key` | Connect to test SSH server with Ed25519 key auth. Assert success. | P0 |
| UT-S-03 | `test_ssh_connect_wrong_password` | Attempt connection with wrong password. Assert `SshError::AuthFailed`. | P0 |
| UT-S-04 | `test_ssh_connect_unreachable` | Connect to non-existent host. Assert timeout/connection error. | P0 |
| UT-S-05 | `test_ssh_write_read` | Connect, write `echo test\n`, assert "test" in output events. | P0 |
| UT-S-06 | `test_ssh_resize` | Connect, resize to 200×60. Assert no error. | P0 |
| UT-S-07 | `test_ssh_disconnect` | Connect, disconnect. Assert connection removed from list. | P0 |
| UT-S-08 | `test_ssh_local_port_forward` | Create local forward (local:8080 → remote:80). Connect to local:8080. Verify data tunnels through. | P1 |
| UT-S-09 | `test_ssh_dynamic_socks5` | Create SOCKS5 dynamic forward. Attempt SOCKS5 handshake. Verify CONNECT request routes. | P1 |
| UT-S-10 | `test_ssh_remote_port_forward` | Create remote forward. Verify `tcpip_forward` accepted and incoming data forwarded. | P1 |
| UT-S-11 | `test_ssh_jump_host` | Connect via jump host (intermediate SSH server). Verify end-to-end connection. | P1 |
| UT-S-12 | `test_ssh_agent_forwarding` | Connect with agent forwarding enabled. Run `ssh-add -l` on remote. Verify agent is forwarded. | P2 |
| UT-S-13 | `test_ssh_keepalive` | Connect with 1-second keep-alive. Wait 5 seconds. Assert connection is still alive. | P1 |
| UT-S-14 | `test_ssh_startup_script` | Connect with startup script `export FOO=bar`. Write `echo $FOO\n`. Assert "bar" in output. | P1 |
| UT-S-15 | `test_ssh_known_hosts_new` | Connect to unknown host. Assert prompt for fingerprint acceptance. | P1 |
| UT-S-16 | `test_ssh_known_hosts_changed` | Change host key after initial acceptance. Connect again. Assert warning/rejection. | P1 |
| UT-S-17 | `test_ssh_concurrent_connections` | Open 5 SSH connections simultaneously. Write to each. Verify independent outputs. | P1 |
| UT-S-18 | `test_ssh_exec_command` | Execute single command (`ls /`) without interactive shell. Verify output and exit code. | P1 |

#### 9.1.6 SFTP Module Tests (`src-tauri/src/sftp/`)

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| UT-SF-01 | `test_sftp_open_session` | Open SFTP session over existing SSH connection. Assert session ID returned. | P0 |
| UT-SF-02 | `test_sftp_list_directory` | List `/tmp` on remote. Assert at least 1 entry with name, size, permissions, type. | P0 |
| UT-SF-03 | `test_sftp_read_file` | Read a known file from remote. Verify contents match. | P0 |
| UT-SF-04 | `test_sftp_write_file` | Upload a file to remote. Verify file exists and contents match. | P0 |
| UT-SF-05 | `test_sftp_delete_file` | Upload then delete a file. Verify file no longer exists. | P0 |
| UT-SF-06 | `test_sftp_mkdir_rmdir` | Create a directory, verify it exists, remove it, verify gone. | P0 |
| UT-SF-07 | `test_sftp_rename` | Create a file, rename it, verify old path gone and new path exists. | P0 |
| UT-SF-08 | `test_sftp_chmod` | Change file permissions. Stat file. Verify permissions changed. | P1 |
| UT-SF-09 | `test_sftp_stat` | Stat a file. Verify size, permissions, modification time. | P0 |
| UT-SF-10 | `test_sftp_large_file_transfer` | Upload a 10 MB file. Download it. Verify SHA-256 matches. | P1 |
| UT-SF-11 | `test_sftp_transfer_progress` | Upload a file and subscribe to progress events. Verify at least 2 progress callbacks with bytes_transferred. | P1 |
| UT-SF-12 | `test_sftp_transfer_cancel` | Start a large upload. Cancel mid-transfer. Verify partial file cleaned up. | P1 |
| UT-SF-13 | `test_sftp_resume` | Upload 50% of a file. Cancel. Resume upload. Verify complete file. | P2 |
| UT-SF-14 | `test_sftp_symlink_follow` | Create a symlink on remote. Stat with follow=true. Verify resolves to target. | P2 |
| UT-SF-15 | `test_sftp_concurrent_transfers` | Upload 5 files simultaneously. Verify all complete correctly. | P1 |

### 9.2 Frontend Unit Tests

**Framework**: Vitest + React Testing Library + `@testing-library/user-event`.

**Coverage target**: ≥ 70% line coverage on all React components and stores.

#### 9.2.1 Store Tests

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| FT-ST-01 | `sessionStore.addSession` | Add a session. Verify it appears in sessions list. | P0 |
| FT-ST-02 | `sessionStore.removeSession` | Add then remove a session. Verify list is empty. | P0 |
| FT-ST-03 | `sessionStore.openTab` | Open a tab for a session. Verify tab appears, is active. | P0 |
| FT-ST-04 | `sessionStore.closeTab` | Open then close a tab. Verify tab removed, active tab updated. | P0 |
| FT-ST-05 | `sessionStore.reorderTabs` | Open 3 tabs. Reorder. Verify new order persists. | P0 |
| FT-ST-06 | `sessionStore.pinTab` | Pin a tab. Verify pinned tabs sort before unpinned. | P0 |
| FT-ST-07 | `sessionStore.addToRecent` | Connect to 30 sessions. Verify recent list capped at 25. | P0 |
| FT-ST-08 | `sessionStore.toggleFavorite` | Toggle favorite on/off. Verify state changes. | P0 |
| FT-ST-09 | `appStore.toggleTheme` | Toggle theme. Verify theme switches between "dark" and "light". | P0 |
| FT-ST-10 | `appStore.setSidebarMode` | Switch sidebar modes. Verify active mode changes. | P0 |
| FT-ST-11 | `vaultStore.unlock` | Mock `invoke("vault_unlock")`. Call unlock. Verify isLocked=false. | P0 |
| FT-ST-12 | `vaultStore.lock` | Unlock then lock. Verify isLocked=true, credentials cleared. | P0 |
| FT-ST-13 | `vaultStore.addCredential` | Mock `invoke`. Add credential. Verify in list. | P0 |
| FT-ST-14 | `terminalStore.createTerminal` | Create terminal instance. Verify in store. | P0 |
| FT-ST-15 | `terminalStore.removeTerminal` | Create then remove. Verify removed from store. | P0 |

#### 9.2.2 Component Tests

| Test ID | Component | Test Description | Priority |
|---------|-----------|------------------|----------|
| FT-C-01 | `<SessionTree>` | Renders sessions grouped by folder hierarchy. | P0 |
| FT-C-02 | `<SessionTree>` | Search input filters sessions by name. | P0 |
| FT-C-03 | `<SessionTree>` | Right-click session shows context menu with correct items. | P0 |
| FT-C-04 | `<SessionTree>` | Click on session calls onConnect callback. | P0 |
| FT-C-05 | `<SessionTree>` | Empty state renders when no sessions exist. | P0 |
| FT-C-06 | `<SessionEditor>` | Renders all form fields. Validates required fields on submit. | P0 |
| FT-C-07 | `<SessionEditor>` | Port auto-populates when session type changes. | P0 |
| FT-C-08 | `<SessionEditor>` | Submit creates session via onSave callback. | P0 |
| FT-C-09 | `<VaultUnlock>` | Shows "Create" mode when vault does not exist. | P0 |
| FT-C-10 | `<VaultUnlock>` | Shows "Unlock" mode when vault exists but is locked. | P0 |
| FT-C-11 | `<VaultUnlock>` | Validates password minimum length (8 chars). | P0 |
| FT-C-12 | `<VaultUnlock>` | Validates password confirmation matches. | P0 |
| FT-C-13 | `<VaultUnlock>` | Calls `invoke("vault_unlock")` on submit with correct password. | P0 |
| FT-C-14 | `<CredentialManager>` | Renders credential list from store. | P0 |
| FT-C-15 | `<CredentialManager>` | Shows appropriate form fields per credential type. | P0 |
| FT-C-16 | `<CommandPalette>` | Opens on Ctrl+Shift+P. Closes on Escape. | P0 |
| FT-C-17 | `<CommandPalette>` | Filters actions by search query. | P0 |
| FT-C-18 | `<CommandPalette>` | Enter key executes selected action. | P0 |
| FT-C-19 | `<Toast>` | Renders success/info/warning/error with correct styling. | P0 |
| FT-C-20 | `<Toast>` | Auto-dismisses after configured duration. | P0 |
| FT-C-21 | `<Toast>` | Error toast persists (no auto-dismiss). | P0 |
| FT-C-22 | `<QuickConnect>` | Parses `user@host:port` correctly. | P0 |
| FT-C-23 | `<QuickConnect>` | Autocompletes from saved sessions. | P1 |
| FT-C-24 | `<SettingsPanel>` | Renders all 5 categories. Switches on click. | P0 |
| FT-C-25 | `<SettingsPanel>` | Toggle change calls `invoke("settings_update")`. | P0 |
| FT-C-26 | `<SftpBrowser>` | Renders dual panes with file listings. | P0 |
| FT-C-27 | `<SftpBrowser>` | Breadcrumb click navigates to directory. | P0 |
| FT-C-28 | `<SftpBrowser>` | Double-click directory enters it. | P0 |
| FT-C-29 | `<TerminalView>` | Initializes xterm.js and WebGL addon. | P0 |
| FT-C-30 | `<TerminalView>` | User input writes to backend via `invoke("terminal_write")`. | P0 |
| FT-C-31 | `<TerminalView>` | Backend output renders in terminal. | P0 |
| FT-C-32 | `<TerminalView>` | Resize event sends dimensions to backend. | P0 |
| FT-C-33 | `<TerminalTab>` | Shows loading state while terminal creates. | P0 |
| FT-C-34 | `<TerminalTab>` | Shows error state with retry button on failure. | P0 |
| FT-C-35 | `<App>` | Renders all 6 regions (A-F). | P0 |
| FT-C-36 | `<App>` | Ctrl+J toggles bottom panel. | P0 |
| FT-C-37 | `<App>` | Sidebar collapses at window width < 900px. | P0 |
| FT-C-38 | `<App>` | Theme toggle switches dark/light and applies CSS class. | P0 |

### 9.3 Integration Tests (Docker-Based)

**Framework**: `cargo test` with `testcontainers-rs` or `docker-compose` fixtures.

**Docker images needed**:
- `linuxserver/openssh-server:latest` — SSH/SFTP target
- `atmoz/sftp:latest` — SFTP-only target (alternate)
- Container with known files for SFTP testing

#### 9.3.1 SSH Integration Tests

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| IT-SSH-01 | `test_ssh_lifecycle_password` | Start OpenSSH container. Connect with password. Run `whoami`. Verify output. Disconnect. | P0 |
| IT-SSH-02 | `test_ssh_lifecycle_key` | Start OpenSSH container. Connect with Ed25519 key. Run command. Verify. Disconnect. | P0 |
| IT-SSH-03 | `test_ssh_local_forward_http` | Start OpenSSH + nginx container. Create local forward 8080→nginx:80. HTTP request to localhost:8080. Verify response. | P1 |
| IT-SSH-04 | `test_ssh_dynamic_socks` | Start OpenSSH container. Create SOCKS5 dynamic forward. Route HTTP request through SOCKS proxy. Verify. | P1 |
| IT-SSH-05 | `test_ssh_jump_host_chain` | Start 2 OpenSSH containers (bastion + target). Connect via bastion. Run command on target. | P1 |
| IT-SSH-06 | `test_ssh_keepalive_prevents_timeout` | Connect with 2-second keep-alive. Idle for 10 seconds. Verify connection alive. | P1 |
| IT-SSH-07 | `test_ssh_agent_forward` | Start OpenSSH container. Connect with agent forwarding. Verify `SSH_AUTH_SOCK` is set on remote. | P2 |
| IT-SSH-08 | `test_ssh_startup_script` | Connect with startup script that sets env var. Verify env var exists in remote shell. | P1 |
| IT-SSH-09 | `test_ssh_reconnect_on_drop` | Connect. Kill SSH container. Detect disconnect. Restart container. Reconnect. Verify. | P2 |
| IT-SSH-10 | `test_ssh_concurrent_sessions` | Open 10 SSH connections to same container. Run commands in parallel. Verify independent outputs. | P1 |

#### 9.3.2 SFTP Integration Tests

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| IT-SFTP-01 | `test_sftp_upload_download_roundtrip` | Upload 1 MB file. Download it. Compare SHA-256 hash. | P0 |
| IT-SFTP-02 | `test_sftp_directory_operations` | mkdir → list → rmdir lifecycle on remote. | P0 |
| IT-SFTP-03 | `test_sftp_large_file_100mb` | Upload 100 MB file. Verify complete transfer and hash match. | P1 |
| IT-SFTP-04 | `test_sftp_permission_change` | Upload file. chmod 755. Stat. Verify permissions. | P1 |
| IT-SFTP-05 | `test_sftp_rename_and_delete` | Upload → rename → verify → delete → verify gone. | P0 |
| IT-SFTP-06 | `test_sftp_transfer_cancel` | Start large upload. Cancel at 50%. Verify server-side file cleaned up. | P1 |
| IT-SFTP-07 | `test_sftp_nested_directory_tree` | Create /a/b/c/d hierarchy. List at each level. Remove recursively. | P1 |
| IT-SFTP-08 | `test_sftp_special_characters` | Upload file with Unicode name. Download. Verify name preserved. | P1 |

#### 9.3.3 Vault Integration Tests

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| IT-V-01 | `test_vault_persistence_across_restarts` | Create vault, add credentials, drop all state, re-init vault, unlock, verify credentials survive. | P0 |
| IT-V-02 | `test_vault_corrupted_db` | Corrupt SQLite file. Attempt unlock. Verify graceful error (not panic). | P1 |
| IT-V-03 | `test_vault_concurrent_credential_access` | 20 concurrent tasks reading/writing credentials. Verify no data corruption. | P1 |

### 9.4 End-to-End Tests

**Framework**: Playwright (via `@playwright/test` against Tauri WebView) or WebdriverIO with `tauri-driver`.

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| E2E-01 | `test_first_launch_wizard` | Fresh app launch → wizard appears → create profile → set password → select theme → finish → main window. | P0 |
| E2E-02 | `test_profile_lock_unlock` | Open app → vault unlock prompt → enter master password → vault unlocks → sidebar shows sessions. | P0 |
| E2E-03 | `test_session_create_connect` | Open session editor → fill SSH details → save → double-click session → terminal opens → prompt visible. | P0 |
| E2E-04 | `test_session_crud` | Create session → verify in tree → edit name → verify update → delete → verify removed. | P0 |
| E2E-05 | `test_tab_management` | Open 3 sessions → verify 3 tabs → close middle tab → verify 2 remain → reorder tabs via drag. | P0 |
| E2E-06 | `test_split_panes` | Open session → right-click tab → "Split Right" → verify 2 panes → resize divider → close one pane. | P1 |
| E2E-07 | `test_sftp_upload_download` | Connect SSH session → open SFTP panel → navigate to /tmp → upload file → verify in listing → download → verify local copy. | P0 |
| E2E-08 | `test_settings_change_theme` | Open settings → change theme to Light → verify UI updates immediately → close settings → reopen → verify theme persisted. | P0 |
| E2E-09 | `test_credential_vault_lifecycle` | Open vault → add SSH key credential → link to session → connect session → verify key used → delete credential → verify orphan warning. | P1 |
| E2E-10 | `test_command_palette` | Press Ctrl+Shift+P → search "theme" → select "Toggle Theme" → verify theme changes. | P0 |
| E2E-11 | `test_quick_connect` | Press Ctrl+Shift+N → type `ssh user@testhost:22` → Enter → verify SSH session opens. | P0 |
| E2E-12 | `test_session_tree_folders` | Create folder "Production" → add 2 sessions → collapse folder → expand → verify sessions visible. | P0 |
| E2E-13 | `test_session_search` | Create 5 sessions → type search query → verify filtered results → clear search → all visible. | P0 |
| E2E-14 | `test_toast_notifications` | Trigger connection failure → verify error toast appears → verify persists (no auto-dismiss) → click dismiss. | P1 |
| E2E-15 | `test_terminal_copy_paste` | Open terminal → run `echo hello` → select "hello" → Ctrl+Shift+C → Ctrl+Shift+V → verify pasted. | P0 |
| E2E-16 | `test_port_forwarding_ui` | Open port forwards panel → add local forward rule → verify rule listed → remove rule → verify removed. | P1 |
| E2E-17 | `test_responsive_sidebar` | Resize window to < 900px → verify sidebar collapses → resize to > 900px → verify sidebar expands. | P1 |
| E2E-18 | `test_vault_auto_lock` | Set idle timeout to 30s → wait 35s → verify vault lock prompt appears → unlock → verify access restored. | P2 |
| E2E-19 | `test_session_import_ssh_config` | Place sample ~/.ssh/config → import → verify sessions created with correct hosts/ports/keys. | P1 |
| E2E-20 | `test_accessibility_keyboard_only` | Navigate entire app using only Tab, Shift+Tab, Enter, Escape, Arrow keys. Verify all features accessible. | P1 |

### 9.5 Security Tests

**Framework**: `cargo fuzz` (libFuzzer via `cargo-fuzz`), `cargo audit`, `cargo clippy`, `npm audit`, ESLint.

| Test ID | Test Name | Description | Priority |
|---------|-----------|-------------|----------|
| SEC-T-01 | `fuzz_vault_unlock` | Fuzz master password input. Verify no panics, no buffer overflows. | P0 |
| SEC-T-02 | `fuzz_vault_credential_data` | Fuzz credential data payloads (random bytes for each field). Verify graceful errors. | P0 |
| SEC-T-03 | `fuzz_ssh_auth` | Fuzz SSH auth parameters (random passwords, key data). Verify no crashes. | P1 |
| SEC-T-04 | `fuzz_session_json` | Fuzz session JSON deserialization with malformed inputs. Verify no panics. | P1 |
| SEC-T-05 | `cargo_audit` | Run `cargo audit` in CI. Fail build on known vulnerabilities (RUSTSEC advisories). | P0 |
| SEC-T-06 | `npm_audit` | Run `npm audit --audit-level=high` in CI. Fail on high/critical vulns. | P0 |
| SEC-T-07 | `clippy_deny` | Run `cargo clippy -- -D warnings`. All warnings treated as errors. | P0 |
| SEC-T-08 | `eslint_strict` | Run ESLint with strict config. Zero warnings in CI. | P0 |
| SEC-T-09 | `dependency_sbom` | Generate CycloneDX SBOM for both Rust and npm dependencies. Verify artifact produced. | P1 |
| SEC-T-10 | `vault_plaintext_leak_check` | After vault operations, scan `/tmp` and profile data directory for plaintext credential fragments. | P1 |

### 9.6 Performance Tests

**Framework**: Rust benchmarks (`criterion`), custom timing harness.

| Test ID | Metric (Spec §16) | Target | How to Measure | Priority |
|---------|-------------------|--------|------|----------|
| PERF-01 | Cold start to usable UI | < 2 seconds | Launch binary, measure time to first interactive frame (via IPC event). | P1 |
| PERF-02 | Tab open to shell prompt (local) | < 500 ms | Time from `terminal_create` to first prompt character in output. | P1 |
| PERF-03 | SSH connect to prompt (LAN) | < 1.5 seconds | Time from `ssh_connect` to first prompt character in output events (LAN Docker container). | P1 |
| PERF-04 | Terminal throughput | ≥ 80 MB/s | `cat` a 1 GB file of random text. Measure elapsed time. Backend PTY throughput only (frontend rendering is separate). | P2 |
| PERF-05 | Memory per idle tab | < 15 MB | Open 10 idle terminal tabs. Measure total RSS increase / 10. | P2 |
| PERF-06 | Memory baseline (1 tab) | < 120 MB | Launch app with 1 local shell tab. Measure RSS. | P1 |
| PERF-07 | SFTP throughput | ≥ 90% of raw SCP | Transfer 100 MB file via SFTP and via SCP. Compare throughput. | P2 |

### 9.7 CI Test Infrastructure

```yaml
# Required Docker Compose for integration tests
services:
  openssh-server:
    image: linuxserver/openssh-server:latest
    ports:
      - "2222:2222"
    environment:
      - PUID=1000
      - PGID=1000
      - USER_NAME=testuser
      - USER_PASSWORD=testpass123
      - PASSWORD_ACCESS=true
    volumes:
      - ./test-fixtures/ssh-keys:/config/.ssh  # Pre-loaded test keys

  openssh-jump:
    image: linuxserver/openssh-server:latest
    ports:
      - "2223:2222"
    # Jump host for ProxyJump tests

  nginx:
    image: nginx:alpine
    # For port-forwarding HTTP tests
```

**CI Pipeline Requirements**:
1. `cargo test` — unit tests (no Docker required)
2. `cargo test --features integration` — integration tests (Docker required)
3. `npx vitest run` — frontend unit tests
4. `npx playwright test` — E2E tests (requires `tauri dev` running)
5. `cargo fuzz run` — fuzz tests (nightly CI only, 60-second budget)
6. `cargo audit && npm audit` — dependency audit
7. `cargo clippy -- -D warnings` — lint
8. `cargo tarpaulin --out Xml` — coverage report
9. Coverage gate: fail if < 80% Rust, < 70% TypeScript

---

## Appendix A — Full Gap Checklist

Summary of all gaps by priority:

| Priority | Total | ✅ Done | Remaining | Description |
|----------|-------|--------|-----------|-------------|
| **P1-BLOCKER** | 15 | 13 | 2 | Must fix before any MVP release |
| **P1-HIGH** | 40 | 25 | 15 | Required for MVP but not architectural blockers |
| **P1-MEDIUM** | 28 | 8 | 20 | Should have for MVP quality |
| **P1-LOW** | 14 | 3 | 11 | Nice to have, can ship without |
| **P2** | 10 | 0 | 10 | Phase 2 — defer |
| **Totals** | **107** | **49** | **58** | — |

### P1-BLOCKER Summary (2 remaining of 15 original)

1. ~~BE-SFTP-01 — Entire SFTP backend module missing~~ ✅
2. ~~BE-AUDIT-01 — Audit events never triggered~~ ✅
3. ~~BE-MOD-01 — No SFTP module~~ ✅
4. ~~FE-WIRE-01 — VaultUnlock never rendered~~ ✅
5. ~~FE-WIRE-02 — CredentialManager never accessible~~ ✅
6. ~~FE-WIRE-05 — SessionEditor never opened~~ ✅
7. ~~FE-SSH-01 — No SSH terminal frontend integration~~ ✅
8. ~~FE-SSH-02 — No SSH connection dialog~~ ✅
9. ~~FE-SFTP-01 — SFTP uses mock data only~~ ✅
10. ~~FE-SESS-07 — Sessions not persisted to disk~~ ✅
11. ~~INT-01 — Session persistence missing~~ ✅
12. ~~INT-02 — Profile lifecycle not in UI~~ ✅
13. ~~INT-03 — SSH session lifecycle not wired~~ ✅
14. BE-SSH-01 — Jump host / ProxyJump not implemented ❌
15. BE-SSH-02 — SSH agent forwarding not implemented ❌

### Test Coverage Gap

| Area | Current Tests | Target Tests | Gap |
|------|:------------:|:------------:|:---:|
| Rust unit tests | 47 | 90 | 43 |
| Frontend unit tests | 49 | 53 | 4 |
| Integration tests | 0 | 21 | 21 |
| E2E tests | 0 | 20 | 20 |
| Security/fuzz tests | 0 | 10 | 10 |
| Performance tests | 0 | 7 | 7 |
| **Total** | **96** | **201** | **105** |

---

*End of gap analysis. This document should be updated as gaps are resolved.*

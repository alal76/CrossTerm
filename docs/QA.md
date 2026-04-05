# CrossTerm — QA Specification

| Field          | Value                                |
|----------------|--------------------------------------|
| Spec ID        | QA-CROSSTERM-001                     |
| Version        | 1.0                                  |
| Status         | Active                               |
| Last Updated   | 2026-04-05                           |
| Parent Spec    | SPEC-CROSSTERM-001 §17              |

---

## 1. Quality Goals

| Goal                 | Target                                                  |
|----------------------|---------------------------------------------------------|
| Unit test coverage   | ≥ 80% line coverage on all Rust backend modules         |
| Frontend coverage    | ≥ 70% line coverage on stores and utility functions     |
| Zero critical bugs   | No P0/P1 bugs in release candidates                     |
| Performance budget   | Per SPEC §16 targets (cold start <2s, SSH connect <1.5s)|
| Security baseline    | No known vulnerabilities in dependencies (cargo/npm audit clean) |
| Accessibility        | WCAG 2.1 AA on all non-terminal UI surfaces             |

---

## 2. Test Pyramid

```
        ┌──────────┐
        │   E2E    │  Playwright (desktop), Appium (Android)
        │  Tests   │  ~20 critical user journeys
        ├──────────┤
        │ Integr.  │  Docker test targets (OpenSSH, etc.)
        │  Tests   │  IPC round-trip tests
        ├──────────┤
        │  Unit    │  Rust: #[cfg(test)] in each module
        │  Tests   │  TS: Vitest for stores & utilities
        └──────────┘
        (Largest layer: unit tests)
```

---

## 3. Rust Backend Testing

### 3.1 Unit Tests

Each backend module (`vault`, `ssh`, `terminal`, `config`, `audit`) must have a `#[cfg(test)]` module with tests covering:

#### Vault Module (`vault/mod.rs`)

| Test Case                              | Category     | Description |
|----------------------------------------|-------------|-------------|
| `test_derive_key_deterministic`        | Crypto      | Same password + salt → same key |
| `test_derive_key_different_salt`       | Crypto      | Different salt → different key |
| `test_encrypt_decrypt_roundtrip`       | Crypto      | encrypt(plaintext) → decrypt → plaintext |
| `test_decrypt_wrong_key_fails`         | Crypto      | decrypt with wrong key → Decryption error |
| `test_decrypt_wrong_nonce_fails`       | Crypto      | decrypt with wrong nonce → Decryption error |
| `test_vault_create_unlock_lock`        | Lifecycle   | Create vault → unlock → verify unlocked → lock → verify locked |
| `test_vault_unlock_wrong_password`     | Auth        | Wrong password → InvalidPassword error |
| `test_credential_create_list_get`      | CRUD        | Create credential → list (summary only, no secrets) → get (with decrypted data) |
| `test_credential_update`              | CRUD        | Update credential fields → get → verify updated |
| `test_credential_delete`             | CRUD        | Delete credential → get → CredentialNotFound |
| `test_credential_list_excludes_secrets`| Security    | List operation never returns encrypted_data or nonce |
| `test_vault_locked_operations_fail`    | Security    | All CRUD operations fail when vault is locked |
| `test_key_zeroized_on_lock`           | Security    | After lock, encryption_key is None |

#### Config Module (`config/mod.rs`)

| Test Case                              | Category     | Description |
|----------------------------------------|-------------|-------------|
| `test_profile_create_list`             | CRUD        | Create profile → appears in list |
| `test_profile_create_duplicate_fails`  | Validation  | Duplicate name → ProfileAlreadyExists |
| `test_profile_update`                  | CRUD        | Update name → verify persisted |
| `test_profile_delete`                  | CRUD        | Delete → no longer in list |
| `test_session_create_list`             | CRUD        | Create session → appears in list |
| `test_session_search_by_name`          | Search      | Search by name substring → matches |
| `test_session_search_by_host`          | Search      | Search by host → matches |
| `test_session_search_by_tag`           | Search      | Search by tag → matches |
| `test_settings_defaults`              | Config      | Fresh profile → default settings values |
| `test_settings_update_persist`         | Config      | Update setting → restart → persisted |

#### Terminal Module (`terminal/mod.rs`)

| Test Case                              | Category     | Description |
|----------------------------------------|-------------|-------------|
| `test_create_terminal_default_shell`   | Lifecycle   | Create with no shell specified → uses default |
| `test_create_terminal_custom_shell`    | Lifecycle   | Create with specific shell → correct shell |
| `test_write_read_roundtrip`           | I/O         | Write to terminal → receive output event |
| `test_resize_terminal`                | I/O         | Resize → no error, dimensions updated |
| `test_close_terminal`                 | Lifecycle   | Close → exit event emitted, removed from map |
| `test_terminal_list`                  | State       | Create 3 → list → 3 entries |
| `test_close_nonexistent_fails`        | Error       | Close invalid ID → NotFound error |

#### SSH Module (`ssh/mod.rs`)

| Test Case                              | Category     | Description |
|----------------------------------------|-------------|-------------|
| `test_build_config_keepalive`          | Config      | verify keepalive_interval and keepalive_max |
| `test_port_forward_id_accessor`        | Type        | PortForward::id() returns correct ID for all variants |
| `test_ssh_auth_serde_password`         | Serde       | Password auth serializes/deserializes correctly |
| `test_ssh_auth_serde_private_key`      | Serde       | Key auth serializes/deserializes correctly |
| `test_ssh_connection_info_fields`      | Type        | SshConnectionInfo has all required fields |

#### Audit Module (`audit/mod.rs`)

| Test Case                              | Category     | Description |
|----------------------------------------|-------------|-------------|
| `test_append_and_read_event`           | I/O         | Append event → read → event present |
| `test_read_empty_log`                 | I/O         | No log file → empty vec (not error) |
| `test_filter_by_event_type`           | Filter      | Multiple event types → filter → only matching |
| `test_pagination_offset_limit`        | Filter      | 10 events → offset 5, limit 3 → events 6-8 |
| `test_csv_export_format`             | Export      | Events → CSV with correct headers and escaping |
| `test_csv_escapes_quotes_in_details`  | Export      | Details with quotes → properly escaped |

### 3.2 Integration Tests (Rust)

Run against Docker test infrastructure in CI:

| Test Target              | Docker Image                  | Tests |
|--------------------------|-------------------------------|-------|
| SSH password auth        | `linuxserver/openssh-server`  | Connect, auth, exec command, disconnect |
| SSH key auth             | `linuxserver/openssh-server`  | Connect with key, exec, verify output |
| SSH port forwarding      | `linuxserver/openssh-server`  | Local forward, verify data tunnel |
| PTY local shell          | Native (CI runner)            | Spawn shell, echo command, verify output |

### 3.3 Fuzz Testing

| Target                   | Tool             | Focus |
|--------------------------|------------------|-------|
| Vault encrypt/decrypt    | `cargo fuzz`     | Arbitrary plaintext → encrypt → decrypt must roundtrip |
| JSON config parsing      | `cargo fuzz`     | Malformed JSON → no panics, clean errors |
| SSH auth parsing         | `cargo fuzz`     | Arbitrary SshAuth input → no panics |

---

## 4. Frontend Testing

### 4.1 Test Framework

- **Unit/Component**: Vitest + React Testing Library
- **E2E**: Playwright (Tauri WebView)

### 4.2 Store Tests (Vitest)

Each Zustand store must have comprehensive unit tests:

#### appStore

| Test Case                              | Description |
|----------------------------------------|-------------|
| `initial state has correct defaults`   | Theme=Dark, sidebar expanded, bottom panel hidden |
| `setSidebarMode changes mode`          | Set to Snippets → mode is Snippets |
| `toggleSidebar flips collapsed`        | Toggle twice → back to original |
| `setTheme updates theme`              | Set Light → theme is Light |
| `setBottomPanelMode shows panel`       | Set mode → bottomPanelVisible is true |
| `setWindowDimensions auto-collapses`   | Width 800 → sidebar collapses |
| `setWindowDimensions no-collapse`      | Width 1200 → sidebar stays expanded |
| `addProfile appends to list`          | Add profile → profiles.length increased |

#### sessionStore

| Test Case                              | Description |
|----------------------------------------|-------------|
| `addSession adds to list`             | Add session → in sessions array |
| `removeSession cascades to tabs/favs`  | Remove session → tab closed, favorite removed |
| `openTab creates tab for new session`  | Open session → tab created, active |
| `openTab reuses existing tab`         | Open same session → no duplicate, just activate |
| `closeTab selects nearest tab`        | Close active → next or previous becomes active |
| `closeTab last tab clears active`     | Close only tab → activeTabId is null |
| `pinTab moves to front`              | Pin tab → pinned is true |
| `toggleFavorite adds and removes`      | Toggle once → added, toggle again → removed |
| `addRecentSession maintains max 25`    | Add 30 → only 25 remain, most recent first |
| `reorderTab updates order field`       | Reorder → order updated |

#### terminalStore

| Test Case                              | Description |
|----------------------------------------|-------------|
| `createTerminal adds to map`          | Create → Map contains terminal |
| `removeTerminal deletes from map`      | Remove → Map no longer contains |
| `updateTerminalDimensions updates`     | Update cols/rows → reflected in state |
| `getTerminalBySession finds match`     | Create with sessionId → findable |
| `getTerminalBySession returns undefined`| No match → undefined |

#### vaultStore

| Test Case                              | Description |
|----------------------------------------|-------------|
| `initial state is locked`             | vaultLocked is true |
| `unlockVault calls invoke and unlocks` | Mock invoke → vaultLocked becomes false |
| `unlockVault handles error`           | Mock invoke rejection → error is set, still locked |
| `lockVault clears credentials`        | Lock → credentials empty, vaultLocked true |
| `addCredential optimistic update`      | Add → immediately in credentials array |
| `deleteCredential removes from list`   | Delete → no longer in credentials |
| `clearError resets error to null`      | Set error → clearError → null |

### 4.3 Component Tests (React Testing Library)

| Component           | Key Test Cases                                                |
|---------------------|---------------------------------------------------------------|
| `StatusDot`         | Renders correct color class for each ConnectionStatus value   |
| `TitleBar`          | Displays profile name; theme toggle switches theme            |
| `TabBar`            | Renders tabs in correct order; pinned first; close button works |
| `Sidebar`           | Mode switching; collapse/expand; session list rendering       |
| `SessionCanvas`     | Empty state when no active tab; type icon for active tab      |
| `BottomPanel`       | Mode tabs switch content; close button hides panel            |
| `StatusBar`         | Shows profile, connection status, encoding                    |
| `CommandPalette`    | Opens on keyboard shortcut; filters actions; executes action  |
| `Toast`             | Renders with correct variant; auto-dismisses success/info     |
| `VaultUnlock`       | Form validation; calls unlockVault; shows errors              |
| `CredentialManager` | Lists credentials; add/edit/delete flows                      |
| `SessionEditor`     | Form validation for required fields; creates session          |
| `SettingsPanel`     | Renders categories; settings changes persist                  |

---

## 5. End-to-End Tests (Playwright)

### 5.1 Setup

- Run against `cargo tauri dev` or a built application.
- Use Playwright's WebView/Electron adapter for Tauri.
- Test data setup: pre-created profiles and sessions via config file seeding.

### 5.2 Critical User Journeys

| # | Journey                                | Steps | Assertions |
|---|----------------------------------------|-------|------------|
| 1 | **First launch → create profile**      | Open app → wizard → set password → create | Profile appears in title bar |
| 2 | **Create SSH session**                 | Sessions panel → New → fill form → save | Session in tree |
| 3 | **Open local terminal**                | Cmd+T → Local Shell → type `echo hello` | "hello" appears in terminal output |
| 4 | **Tab management**                     | Open 3 tabs → click tab 2 → close tab 3 | Correct tab active, tab 3 gone |
| 5 | **Sidebar collapse/expand**            | Click sidebar icon → collapsed → click again → expanded | Width transitions correctly |
| 6 | **Theme switching**                    | Click theme toggle → assert light class → toggle back | CSS variables change |
| 7 | **Vault unlock → add credential**      | Unlock vault → add password credential → verify in list | Credential name visible |
| 8 | **Vault lock → operations blocked**    | Lock vault → try list credentials → blocked | Error message or lock screen |
| 9 | **Command palette**                    | Cmd+Shift+P → type "toggle" → select "Toggle Sidebar" | Sidebar toggles |
| 10| **Quick Connect**                      | Cmd+Shift+N → type `ssh user@host` → enter | Tab created with SSH type |
| 11| **Bottom panel**                       | Ctrl+J → panel visible → switch modes → close | Panel shows/hides correctly |
| 12| **Session edit**                       | Right-click session → Edit → change name → save | Updated name in tree |
| 13| **Pin/unpin tab**                      | Right-click tab → Pin → icon-only → Unpin → full tab | Tab width changes |
| 14| **Middle-click close tab**             | Middle-click a tab | Tab closes |
| 15| **Window resize → sidebar collapse**   | Resize window < 900px | Sidebar auto-collapses |
| 16| **Settings panel**                     | Open settings → change font size → verify | Setting persisted |
| 17| **Audit log view**                     | Open bottom panel → Audit Log tab → verify events | Events listed |
| 18| **Multiple profiles**                  | Create second profile → switch → different session list | Isolated data |
| 19| **SFTP browser**                       | Connect to SSH session → open SFTP panel → browse | Files/directories listed |
| 20| **Keyboard navigation**               | Tab through UI elements → all reachable by keyboard | Focus ring visible |

---

## 6. Security Testing

### 6.1 Static Analysis

| Tool            | Scope    | CI Integration | Threshold |
|-----------------|----------|----------------|-----------|
| `cargo clippy`  | Rust     | Every PR       | Zero warnings (deny level) |
| `cargo audit`   | Rust deps| Daily + every PR | Zero known vulnerabilities |
| `npm audit`     | JS deps  | Daily + every PR | Zero high/critical |
| ESLint strict   | TypeScript| Every PR      | Zero errors |

### 6.2 Security Test Cases

| Category             | Test Case                                                    | Tool |
|----------------------|--------------------------------------------------------------|------|
| Credential storage   | Verify vault DB is not readable without password             | Manual + unit test |
| Memory safety        | Verify key material is zeroized after vault lock             | Unit test with memory inspection |
| IPC boundary         | Verify `encrypted_data` never appears in Tauri IPC responses | Integration test |
| Secret serialization | Verify `#[serde(skip_serializing)]` on sensitive fields      | Unit test (serde_json::to_string) |
| SSH host keys        | Verify host key verification (when implemented)              | Integration test |
| Audit completeness   | Verify all security events are logged                        | Integration test |
| Input validation     | Verify no SQL injection in credential queries                | Fuzz test |
| Dependency chain     | No known CVEs in transitive dependencies                     | cargo audit + npm audit |

### 6.3 Penetration Testing Checklist (Pre-Release)

- [ ] Vault DB cannot be decrypted with brute-force in reasonable time (Argon2id params enforced)
- [ ] No credentials leak into application logs
- [ ] No credentials leak into crash reports
- [ ] IPC commands validate all parameters (no injection)
- [ ] Plugin sandbox (when implemented) cannot access filesystem beyond scope
- [ ] Auto-updater verifies code signatures

---

## 7. Performance Testing

### 7.1 Performance Budgets (from SPEC §16)

| Metric                            | Target          | Test Method |
|-----------------------------------|-----------------|-------------|
| Cold start to usable UI           | < 2 seconds     | Playwright timing |
| Tab open to shell prompt (local)  | < 500 ms        | Event timing |
| SSH connect to shell prompt (LAN) | < 1.5 seconds   | Docker target |
| Terminal throughput                | ≥ 80 MB/s       | `cat` large file, measure render time |
| Memory per idle terminal tab      | < 15 MB         | Process monitor |
| Memory baseline (1 tab open)      | < 120 MB        | Process monitor |

### 7.2 Performance Test Scenarios

| Scenario                        | Setup                           | Metric |
|---------------------------------|---------------------------------|--------|
| Startup time                    | Cold launch, measure until `DOMContentLoaded + render` | < 2s |
| Tab switching latency           | 10 open tabs, rapidly switch    | < 50ms per switch |
| Terminal flood                  | `yes` command running for 5s    | No UI freeze, measured FPS > 30 |
| Large scrollback                | `cat` 100MB file                | Throughput ≥ 80 MB/s |
| Multiple terminals              | Open 20 simultaneous terminals  | Memory < 500 MB total |
| Long idle                       | Leave 5 sessions idle 1 hour    | No memory growth > 10% |

---

## 8. Accessibility Testing

### 8.1 Automated

| Tool                | Coverage                          | CI Integration |
|---------------------|----------------------------------|----------------|
| axe-core            | WCAG 2.1 AA on all rendered pages | Playwright + axe |
| Lighthouse          | Accessibility audit               | Periodic |

### 8.2 Manual Checklist

- [ ] All interactive elements reachable by keyboard (Tab/Shift+Tab)
- [ ] Focus ring (2px, `border-focus` token) visible on all focused elements
- [ ] No focus traps outside modal dialogs
- [ ] ARIA roles on all regions (`navigation`, `main`, `complementary`, `status`)
- [ ] Live regions for connection state changes and toast notifications
- [ ] Color is never the sole indicator of status (dots supplemented with shapes)
- [ ] UI text scales with configurable scale factor (75%–200%)
- [ ] `prefers-reduced-motion` disables all animations
- [ ] High Contrast theme achieves ≥ 7:1 text contrast, ≥ 3:1 interactive boundary contrast
- [ ] Screen reader announces tab switches, panel changes, connection events

---

## 9. CI/CD Pipeline

### 9.1 Pipeline Stages

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌──────────┐    ┌─────────┐
│  Lint   │ →  │  Build  │ →  │  Test   │ →  │ Security │ →  │ Package │
│ clippy  │    │ cargo   │    │  unit   │    │  audit   │    │  tauri  │
│ eslint  │    │ vite    │    │  integ  │    │  fuzz    │    │  build  │
│ fmt     │    │ tsc     │    │  e2e    │    │          │    │         │
└─────────┘    └─────────┘    └─────────┘    └──────────┘    └─────────┘
```

### 9.2 PR Checks (Required)

| Check               | Command                          | Pass Criteria |
|----------------------|----------------------------------|---------------|
| Rust lint            | `cargo clippy -- -D warnings`    | Zero warnings |
| Rust format          | `cargo fmt --check`              | No diff       |
| Rust tests           | `cargo test`                     | All pass      |
| TS type check        | `tsc -b --noEmit`                | Zero errors   |
| TS lint              | `npx eslint .`                   | Zero errors   |
| Frontend build       | `npx vite build`                 | Successful    |
| Dependency audit     | `cargo audit && npm audit`       | No high/critical |

### 9.3 Nightly Checks

| Check               | Command                          | Purpose |
|----------------------|----------------------------------|---------|
| Integration tests    | Docker-based SSH/SFTP tests      | Regression |
| E2E tests            | Playwright full suite            | User journey validation |
| Fuzz testing         | `cargo fuzz run` (30 min)        | Edge case discovery |
| Performance baseline | Benchmark suite                  | Detect regressions |

---

## 10. Bug Classification

| Priority | Label | Definition                                    | SLA         |
|----------|-------|-----------------------------------------------|-------------|
| P0       | 🔴    | Data loss, security breach, complete crash     | Fix within 24h |
| P1       | 🟠    | Feature broken, no workaround                  | Fix within 3 days |
| P2       | 🟡    | Feature broken, workaround exists              | Fix within sprint |
| P3       | 🟢    | Cosmetic, minor UX issue                       | Backlog     |

---

## 11. Release Criteria

A release candidate must meet ALL of the following:

- [ ] All P0/P1 bugs resolved
- [ ] Unit test coverage ≥ 80% (Rust), ≥ 70% (TypeScript stores)
- [ ] All 20 E2E critical journeys passing
- [ ] `cargo audit` and `npm audit` clean
- [ ] Performance budgets met (SPEC §16)
- [ ] Accessibility checklist complete
- [ ] SBOM (CycloneDX) generated
- [ ] Code signed (macOS notarized, Windows Authenticode)
- [ ] Changelog updated

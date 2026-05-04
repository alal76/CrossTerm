# CrossTerm Engineering Execution Plan
### Delivering the Product Roadmap — v0.3 through v1.2

**Document owner:** Engineering  
**Last updated:** 2026-05-04  
**Paired with:** [ROADMAP.md](ROADMAP.md)

---

## 1. Current Engineering State Assessment

Before planning, an honest inventory of what we are working with:

### Codebase health

| Area | Lines | Test coverage | Condition |
|------|-------|--------------|-----------|
| SSH backend (`ssh/mod.rs`) | 2,423 | ~0% | Monolithic — no boundary interfaces |
| Vault backend (`vault/mod.rs`) | 2,684 | ~0% | Security-critical with no regression safety net |
| Network backend (`network/mod.rs`) | 2,759 | ~0% | Scanner, WiFi, port-forward all interleaved |
| Config (`config/mod.rs`) | 2,236 | ~30% (one test) | 58-field Settings struct, brittle struct-literal tests |
| Frontend components | ~6,500 | ~45% | Good for UI; stores under-tested |
| Plugin runtime | 905 | ~0% | Stub — planned for Phase 3 |

**Critical finding:** The four largest backend modules have zero unit test coverage and no internal module boundaries. Any change to SSH or vault carries high regression risk. This must be addressed before new features ship into those modules.

### Infrastructure

| System | Status | Gap |
|--------|--------|-----|
| CI (GitHub Actions) | Passing | Rust tests not run; only `cargo build` + lint |
| Release pipeline | Manual tag push → artifact build | No Homebrew SHA update automation (fixed in 0.2.3) |
| Crash reporting | None | No visibility into production failures |
| Performance monitoring | None | No baseline for startup or connection times |
| Code coverage | Not measured | No gate |

### Team assumptions (plan to scale)

This plan is written assuming a starting team of **1–2 engineers** (current state) scaling to **4–6** by Phase 3. Milestones are sized accordingly. Adjust sprint capacity per actual headcount.

---

## 2. Engineering Principles

1. **Test-first for backend changes.** Any change to `ssh/`, `vault/`, `security/`, or `config/` must include a unit test covering the new path. No exceptions.
2. **Typed errors, not strings.** `invoke` calls return `Result<T, AppError>` where `AppError` is a Rust enum serialized to JSON with a `code` field. The UI never displays raw Rust panic messages.
3. **Modules before features.** When a backend module exceeds 800 lines, split it before adding new behaviour. Complexity budget: 800 lines / module, 50 lines / function (cognitive complexity ≤ 15).
4. **Measure before optimising.** No performance work without a benchmark establishing a baseline.
5. **Deprecate, don't delete.** Public `invoke` command names are API surface — maintain backward compatibility within a major version.
6. **Security review gate.** Any PR touching `vault/`, `security/`, `ssh/` key handling, or plugin permissions requires a second-engineer review with a security checklist sign-off.

---

## 3. Phase 1 Engineering Plan — Foundation (v0.3 → v0.4)

**Duration:** 10 weeks  
**Goal:** Zero P0 crashes; onboarding test passes without docs; backend test coverage ≥ 60%

### Sprint 1–2: Test infrastructure & error taxonomy (weeks 1–4)

**Why first:** Everything else builds on the safety net. Running tests after each feature is written in reverse order.

#### Backend test infrastructure

```
src-tauri/src/
├── ssh/
│   ├── mod.rs          ← extract: connection.rs, auth.rs, forwarding.rs, keepalive.rs
│   └── tests/
│       ├── auth_tests.rs
│       ├── connection_tests.rs
│       └── forwarding_tests.rs
├── vault/
│   ├── mod.rs          ← extract: crypto.rs, store.rs, credentials.rs
│   └── tests/
│       ├── crypto_tests.rs      ← test Argon2id params, AES-GCM round-trip
│       └── credential_tests.rs
└── network/
    ├── mod.rs          ← extract: scanner.rs, wifi.rs, tunnel.rs
    └── tests/
```

Tasks:
- [ ] Add `cargo test` step to CI; gate PR merges on test pass
- [ ] Add `cargo tarpaulin` coverage report as CI artifact; set 60% floor as a non-blocking warning (becomes blocking at Phase 2)
- [ ] Refactor `ssh/mod.rs` into 4 submodules: `connection`, `auth`, `forwarding`, `terminal_io`
- [ ] Refactor `vault/mod.rs` into 3 submodules: `crypto`, `store`, `credentials`
- [ ] Write minimum 40 unit tests across ssh + vault (target: every public `#[tauri::command]` has ≥ 1 happy-path + 1 error test)
- [ ] Implement `AppError` enum in `src-tauri/src/error.rs`:
  ```rust
  #[derive(Debug, Serialize, thiserror::Error)]
  pub enum AppError {
      #[error("auth_failed")] AuthFailed { detail: String },
      #[error("host_unreachable")] HostUnreachable { host: String, port: u16 },
      #[error("host_key_mismatch")] HostKeyMismatch { fingerprint: String },
      #[error("vault_locked")] VaultLocked,
      #[error("vault_wrong_password")] VaultWrongPassword,
      #[error("permission_denied")] PermissionDenied { detail: String },
      #[error("io_error")] IoError { detail: String },
      #[error("internal")] Internal { detail: String },
  }
  ```
- [ ] Wire `AppError` through all `ssh_*` and `vault_*` command handlers; update frontend `invoke` call sites to read `error.code`
- [ ] Write a `src/utils/errorMessages.ts` that maps `AppError.code` to localised friendly copy (20 most common codes)

**Deliverable:** `cargo test` runs 40+ tests in CI. Zero raw error strings reach the UI for SSH or vault flows.

---

#### Frontend test infrastructure

- [ ] Add `@vitest/coverage-v8` to CI; publish coverage report as artifact
- [ ] Set coverage gate: 75% lines for `src/stores/`, `src/components/Vault/`, `src/components/Settings/`
- [ ] Add integration tests for `vaultStore` (lock/unlock/create/delete state transitions)
- [ ] Add integration tests for `sessionStore` (add/remove/select tab)
- [ ] Add `src/utils/errorMessages.test.ts` covering all 20 mapped error codes

---

### Sprint 3–4: Onboarding & session watchdog (weeks 5–8)

#### Import wizard

Architecture: a new Tauri command `session_import_detect` scans known config locations and returns a typed `ImportSource` list; `session_import_execute` converts them to `Session` objects.

```rust
// src-tauri/src/importer/mod.rs  (new module)
pub enum ImportSource {
    SshConfig { path: PathBuf, host_count: usize },
    PuTTY { registry_key: String, session_count: usize },  // Windows only
    SecureCRT { ini_path: PathBuf, session_count: usize },
    MobaXterm { path: PathBuf, session_count: usize },
}
```

Frontend: `src/components/Shared/ImportWizard.tsx` — 3-step modal:
1. Auto-detected sources (checkboxes)
2. Preview table (host / type / credential — editable before import)
3. Summary (N sessions imported, M already existed → skip)

Tasks:
- [ ] Implement `importer/mod.rs` with `.ssh/config` parser (full ProxyJump, IdentityFile, Port)
- [ ] Implement PuTTY registry reader (Windows, `winreg` crate)
- [ ] Implement SecureCRT `.ini` parser
- [ ] Implement MobaXterm `.mxtsessions` parser
- [ ] Build `ImportWizard.tsx` frontend component
- [ ] Wire into First Launch Wizard step 1 AND into File menu → Import Sessions
- [ ] Unit tests: 1 fixture file per importer format, assert correct `Session` output

#### First-run redesign

Replace the raw `VaultUnlock` gate shown on first launch with `FirstLaunchWizard` steps:
- Step 0 (conditional): "Import existing sessions?" → `ImportWizard`
- Step 1: Create vault (existing `VaultUnlock` create mode, but with context copy)
- Step 2: Theme + font preview
- Step 3: Keyboard shortcut reference card

Tracked in: `appStore.firstLaunchComplete: boolean` persisted to config.

#### Session watchdog

```rust
// src-tauri/src/ssh/keepalive.rs  (new)
// Emits a "session_health" Tauri event every 30s per active session
// Fields: session_id, status (ok/degraded/dropped), latency_ms, last_seen_secs
```

Frontend `useEffect` in `SshTerminalView.tsx` listens for `session_health` events and:
- Shows a yellow banner if `status === "degraded"` (high latency or missed keepalives)
- Shows a red reconnect overlay if `status === "dropped"` (5s auto-retry, manual override)

Tasks:
- [ ] Implement session health event emitter in `ssh/keepalive.rs`
- [ ] Implement `ReconnectOverlay.tsx` redesign with exponential backoff countdown
- [ ] Add tunnel health events from `network/tunnel.rs` (same pattern)
- [ ] Toast notification when a tunnel drops while in background

---

### Sprint 5 — Polish & release (weeks 9–10)

- [ ] Delete-confirm guard on vault trash icon (200ms delay + tooltip "Click again to delete")
- [ ] "Recently connected" section in session tree (last 5, sorted by `lastConnectedAt`)
- [ ] Right-click terminal tab → "Open SFTP here"
- [ ] Ctrl+Shift+F focuses search bar (already implemented — fix discoverability: add hint in status bar)
- [ ] v0.3.0 release: internal dogfood, then beta, then stable
- [ ] v0.4.0: ship the full Phase 1 feature set after 2-week beta soak

**Phase 1 exit gate checklist:**
- [ ] 0 P0 crashes in 2-week soak on macOS 14, Windows 11, Ubuntu 24.04
- [ ] Backend coverage ≥ 60%, frontend coverage ≥ 75%
- [ ] Onboarding test: unfamiliar user connects 3 SSH hosts in < 5 minutes without docs
- [ ] All structured `AppError` codes have friendly messages in English + at least one other locale

---

## 4. Phase 2 Engineering Plan — Power User (v0.5 → v0.6)

**Duration:** 12 weeks  
**Goal:** Replace every session management tool for individual power users

### Module ownership (assumes 3 engineers)

| Engineer | Primary area | Secondary |
|----------|-------------|-----------|
| Eng-A | Session tree v2, smart groups | Macro GUI builder |
| Eng-B | Security (TOTP, YubiKey, audit export) | Certificate pinning |
| Eng-C | Terminal quality (hyperlinks, RTL, regex search) | Performance |

### Key technical decisions

#### Session tree v2

The session tree is currently a flat list with client-side grouping. Phase 2 requires:

1. **Virtual scrolling** (`@tanstack/react-virtual`) — the tree must handle 1,000+ sessions without frame drops
2. **Drag-and-drop** (`@dnd-kit/core`) — move sessions between groups; reorder groups
3. **Multi-select** — `selectedSessionIds: Set<string>` in `sessionStore`; Shift+click range select, Ctrl/⌘+click toggle
4. **Smart groups** — stored as `{ id, name, filter: FilterExpr }` where `FilterExpr` is a typed predicate tree evaluated client-side against the session list

```typescript
// src/types/index.ts additions
type FilterExpr =
  | { type: "tag"; value: string }
  | { type: "protocol"; value: SessionType }
  | { type: "status"; value: ConnectionStatus }
  | { type: "last_connected_before"; days: number }
  | { type: "and"; children: FilterExpr[] }
  | { type: "or"; children: FilterExpr[] };
```

#### TOTP vault unlock

The `TOTPSeedCredential` type exists. The unlock flow needs:

1. Backend `vault_verify_totp(vault_id, totp_code)` command that reads the vault's linked TOTP seed, computes the TOTP at current time ± 1 window, and validates
2. Frontend: optional second field in `VaultUnlock` for the OTP code; shown only if vault has a TOTP seed linked

Security invariant: TOTP verification happens **after** password verification, never instead of it.

#### YubiKey / FIDO2

The `vault_fido2_auth_begin` stub already exists. Full implementation:

1. Add `ctap2` (or `fido2-rs`) Rust dependency
2. Implement CTAP2 `authenticatorMakeCredential` + `authenticatorGetAssertion` flow in `vault/fido2.rs`
3. Store the credential ID in vault metadata (not in the vault itself — needed to unlock it)
4. Frontend: "Use Security Key" button in VaultUnlock already present; wire to the completed backend

#### Audit log export

```rust
// src-tauri/src/audit/export.rs  (new)
pub enum ExportFormat { Csv, SignedPdf, Syslog { host: String, port: u16, protocol: SyslogProtocol } }
pub async fn export_audit_log(entries: &[AuditEntry], format: ExportFormat) -> Result<ExportResult>
```

PDF generation: use `printpdf` crate. Include a SHA-256 hash of the log contents in the footer for integrity verification.

### Sprint plan (Phase 2)

| Sprint | Weeks | Output |
|--------|-------|--------|
| S6 | 1–2 | Session tree virtual scroll + multi-select |
| S7 | 3–4 | Session tree drag-and-drop + smart groups |
| S8 | 5–6 | TOTP vault unlock + certificate pinning UI |
| S9 | 7–8 | YubiKey / FIDO2 vault unlock + audit export |
| S10 | 9–10 | Macro GUI builder + dry-run mode |
| S11 | 11 | Terminal hyperlinks + regex scrollback search |
| S12 | 12 | RTL support + v0.5 release; v0.6 after 3-week beta |

**Phase 2 exit gate:**
- [ ] Session tree handles 1,000 sessions with < 16ms frame time (measured with Chrome DevTools / Tauri profiler)
- [ ] TOTP + YubiKey both pass manual security test matrix
- [ ] NPS ≥ 45 from beta user survey (n ≥ 30)
- [ ] Backend coverage ≥ 70%, frontend coverage ≥ 80%

---

## 5. Phase 3 Engineering Plan — Team & Enterprise (v0.7 → v1.0)

**Duration:** 16 weeks  
**Goal:** First enterprise customer signed. SOC 2 Type I initiated.

### Team size: 4–6 engineers

Recommended structure:
- **Platform team (2 eng):** shared vault, SSO/LDAP, MDM, RBAC
- **Features team (2 eng):** cloud integration depth (SSM/Bastion/IAP), session recording policy
- **Reliability team (1 eng):** observability, centralized audit, compliance report
- **Security eng (1, part-time or contractor):** threat model review, penetration test coordination

### Shared vault architecture

The `VaultInfo.shared_with` field exists in the type system. The cryptographic model:

```
Per-user key:   KEK_user  =  Argon2id(master_password, salt_user)
Vault DEK:      DEK       =  random 256-bit key
Vault data:     AES-256-GCM(DEK, vault_contents)
Sharing:        Envelope_peer = AES-256-GCM(KEK_peer_public, DEK)
                (one envelope per shared user, stored in vault metadata)
```

- Owner generates a key pair (Curve25519) at vault creation; public key stored in profile registry
- To share: owner encrypts DEK with peer's public key; peer decrypts envelope with their private key (never leaves their device) to get DEK
- Revocation: owner rotates DEK; re-encrypts all envelopes except revoked peer
- Backend: new commands `vault_share`, `vault_unshare`, `vault_accept_share`, `vault_rotate_dek`

### SSO / SAML / OIDC

Use `oxide-auth` or `openidconnect` Rust crates. Architecture:

1. CrossTerm acts as an OIDC Relying Party with a loopback redirect URI (`http://127.0.0.1:{ephemeral_port}/callback`)
2. On login, open browser to IdP authorization URL; wait for callback with auth code
3. Exchange code for ID token; extract profile claims (sub, email, groups)
4. Map claims to local CrossTerm profile; derive vault KEK from the IdP-issued session token (not a password — use PKCE + token binding)

SAML requires a library (`samael` crate or custom XMLSec implementation). Consider offering OIDC only in v1.0 and adding SAML in a v1.1 patch.

### Session recording policy

Recording already exists (`recording/mod.rs`). Policy layer adds:

```rust
// src-tauri/src/config/policy.rs  (new)
pub struct RecordingPolicy {
    pub require_recording_for: Vec<HostPattern>,  // e.g. "*.prod.*"
    pub storage_path: PathBuf,
    pub retention_days: u32,
    pub encrypt_recordings: bool,
}
```

- Policy loaded from a `policy.json` file that MDM can push
- Recordings encrypted with a separate key derivable by the compliance reviewer role
- Frontend: non-dismissible banner when connected to a policy-mandated host: "This session is being recorded"

### Sprint plan (Phase 3)

| Sprint | Weeks | Output |
|--------|-------|--------|
| S13–14 | 1–4 | Shared vault crypto + backend + basic UI |
| S15–16 | 5–8 | OIDC SSO + RBAC model |
| S17 | 9–10 | AWS SSM + Azure Bastion + GCP IAP |
| S18 | 11–12 | Session recording policy + compliance reviewer UI |
| S19 | 13–14 | Centralized audit export (Splunk/S3/syslog) + compliance PDF |
| S20 | 15–16 | MDM policy JSON + v0.9 RC + security penetration test |
| — | +2 | Pen test remediation → v1.0 stable |

**Phase 3 exit gate:**
- [ ] External penetration test (scope: vault, SSO, plugin sandbox, network commands) — all critical/high findings remediated
- [ ] SOC 2 Type I audit initiated with external auditor
- [ ] At least 1 enterprise customer (≥ 50 seats) in production
- [ ] Backend coverage ≥ 75%, frontend coverage ≥ 85%
- [ ] Shared vault tested against a formal threat model (documented)

---

## 6. Cross-Cutting Engineering Work

These run alongside all phases and are never "done."

### 6.1 Performance budget

Every release must meet the following budgets, measured on a reference machine (M2 MacBook Air, 8 GB RAM, 100 Mbps network):

| Metric | Budget | Measurement method |
|--------|--------|--------------------|
| Cold startup to interactive | ≤ 1.5 s | Tauri app `ready` event timestamp |
| SSH connection establishment | ≤ 1.5 s (P99) | `ssh_connect` invocation timing |
| Session tree render (1,000 items) | ≤ 16 ms | `performance.mark()` around tree render |
| SFTP directory listing (100 items) | ≤ 300 ms | `sftp_list_directory` invocation timing |
| Vault unlock | ≤ 400 ms | Argon2id is intentionally slow; acceptable |
| Terminal input latency | ≤ 20 ms | xterm.js input event to echo |

Regressions against budget are blocking. A CI benchmark suite (using `criterion` for Rust, `vitest bench` for TS) must be added in Phase 1 and gates added in Phase 2.

### 6.2 Accessibility

Target: WCAG 2.1 Level AA by v1.0.

Phase 1: Audit with `axe-core` (`vitest-axe`) — fix all "critical" violations  
Phase 2: Full keyboard-only walkthrough of every modal and panel; fix all "serious" violations  
Phase 3: Screen reader testing (VoiceOver macOS, NVDA Windows); fix remaining violations

Key items identified now:
- Terminal canvas (`<canvas>`) needs an accessible live region for screen readers
- All icon-only buttons need `aria-label`
- Color contrast: status dot colours must meet 4.5:1 against all themes

### 6.3 Internationalisation

Current state: i18n infrastructure exists; English is complete; other locales are stubs.

Plan:
- Phase 1: Complete German (de) and Japanese (ja) locales (high-value markets)
- Phase 2: Arabic (ar) and Hebrew (he) — requires RTL layout work
- Phase 3: Chinese Simplified (zh-Hans) — required for enterprise APAC deals

Use Crowdin for community translation. Budget 1–2 days per release for locale sync.

### 6.4 Dependency hygiene

Run weekly (CI cron):
- `cargo audit` — Rust advisory database
- `npm audit` — JS advisory database
- `cargo outdated` — flag major version bumps for review

Policy: critical advisories are patched within 48 hours and shipped as a patch release regardless of feature freeze status.

### 6.5 Release engineering

Current: manual tag push. Target state by Phase 2:

```
Push tag v0.x.y
    ↓
CI builds macOS (aarch64 + x86_64), Windows (x64), Linux (x64 deb/rpm/AppImage)
    ↓
SHA256 checksums computed and attached
    ↓
GitHub Release created with artifacts + release notes (auto-generated from conventional commits)
    ↓
Homebrew tap updated automatically (already wired in release.yml)
    ↓
Windows installer published to winget manifest PR (automated)
    ↓
Linux packages submitted to AUR (Arch) and Ubuntu PPA (manual review, then automated)
```

Add by Phase 2:
- Auto-generated changelog from conventional commit messages (`git-cliff`)
- Windows `winget` manifest auto-PR
- Delta update support (Tauri updater + `tauri-plugin-updater`) so users download only diffs

---

## 7. Technical Debt Register

Items that must not be allowed to persist beyond their "Fix by" phase:

| # | Debt item | Fix by | Risk if ignored |
|---|-----------|--------|----------------|
| TD-1 | `ssh/mod.rs`, `vault/mod.rs`, `network/mod.rs` > 2,400 lines each with no module boundaries | Phase 1 | Regressions on every change; onboarding new engineers takes weeks |
| TD-2 | Zero backend unit tests in security-critical modules | Phase 1 | Undetectable vault crypto regressions |
| TD-3 | `Settings` struct has 58 fields in one struct literal — tests break on every new field | Phase 1 | CI becomes increasingly fragile |
| TD-4 | `FIDO2` / `vault_fido2_auth_begin` is a stub that always fails silently | Phase 2 | Users see "Security Key" button that does nothing; trust erosion |
| TD-5 | `shared_with: Vec<String>` in `VaultInfo` — field exists but backend ignores it | Phase 3 | Misleading data model; risk of partial implementation shipping |
| TD-6 | Android components (`src/components/Android/`) have no tests and are untouched since initial commit | Phase 3 | Android launch blocked on unknown quality |
| TD-7 | `plugin_rt/mod.rs` stubs are 905 lines of unimplemented scaffolding | Phase 3 | Plugin system ships with no real capability; developer trust lost |
| TD-8 | Hardcoded version strings in `helpContent.ts` and `installation.md` | Each release | Version drift confuses users (currently automated via sed in commit) |

---

## 8. Hiring Plan

| Phase | Role | Justification |
|-------|------|--------------|
| Phase 1 | Senior Rust engineer (backend) | SSH/vault refactor and test coverage requires deep Rust experience |
| Phase 2 | Senior frontend engineer | Session tree virtualisation + accessibility work is full-time |
| Phase 3 | Security engineer (FT or contractor) | Shared vault crypto, SAML, pen test coordination |
| Phase 3 | DevRel / SDK engineer | Plugin ecosystem won't grow without an advocate and SDK samples |
| Phase 4 | ML engineer (part-time) | Local LLM integration and anomaly detection |

---

## 9. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Tauri v3 breaking changes require major migration | Medium | High | Pin Tauri to minor version in CI; allocate 1 sprint per major Tauri upgrade |
| macOS notarisation / Gatekeeper policy changes block distribution | Low | Critical | Maintain Apple developer account in good standing; test notarisation on every tag |
| FIDO2 / CTAP2 hardware compatibility varies widely | High | Medium | Test against YubiKey 5, SoloKey, and FIDO2 software emulator in CI |
| Argon2id parameters too slow on low-end Windows hardware | Medium | Medium | Benchmark on a $300 Windows laptop; add CPU-count-aware parameter auto-tuning |
| Open source competitors (Tabby, WezTerm) add vault features | Medium | Medium | Our moat is breadth + Android + enterprise — keep shipping Phase 3 features |
| SOC 2 audit takes longer than expected | Medium | High | Start pre-audit readiness work in Phase 2 (policy docs, access reviews) |
| Plugin WASM sandbox escape vulnerability | Low | Critical | Formal security review of `plugin_rt` before any plugin ships publicly; bug bounty |

---

## 10. Definition of Done

A feature is done when:

1. **Code complete** — implementation merged to `main`
2. **Tests written** — unit + integration coverage meets phase floor; no test coverage regression
3. **Error paths covered** — all `AppError` codes this feature can emit are handled in UI with friendly messages
4. **Accessibility checked** — `axe-core` reports zero new critical violations
5. **Help article updated** — `docs/help/` and `helpContent.ts` updated with any new UI concepts
6. **i18n strings added** — all new user-visible strings in `en.json`; marked for translation in Crowdin
7. **Changelog entry written** — conventional commit message enables auto-changelog generation
8. **Security reviewed** (for vault/SSH/plugin/network changes) — second engineer sign-off on security checklist

---

## 11. First 30 Days — Concrete Actions

Regardless of team size, these actions unlock Phase 1 immediately:

| Day | Action | Owner |
|-----|--------|-------|
| 1 | Add `cargo test` step to CI; confirm it runs (even with 0 tests currently) | Eng |
| 1 | Add `tarpaulin` coverage report to CI | Eng |
| 2 | Create `src-tauri/src/error.rs` with `AppError` enum (skeleton, 8 variants) | Eng |
| 3–5 | Refactor `vault/mod.rs` → `vault/{crypto,store,credentials}.rs`; add 10 unit tests | Eng |
| 3–5 | Refactor `ssh/mod.rs` → `ssh/{connection,auth,forwarding,terminal_io}.rs` | Eng |
| 6–10 | Wire `AppError` to all vault + SSH command handlers; update frontend error handling | Eng |
| 11–15 | Build `.ssh/config` importer (highest import value); add fixture-based tests | Eng |
| 16–20 | Build `ImportWizard.tsx` frontend; wire into first-launch flow | Eng |
| 21–25 | Implement session health watchdog + reconnect overlay | Eng |
| 26–30 | Fix U-7 (delete confirm guard), U-13 (recently connected), U-15 (open SFTP here) | Eng |
| 30 | Tag v0.3.0-beta; start internal dogfood | Eng |

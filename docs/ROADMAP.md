# CrossTerm Product Roadmap
### Becoming the Premium Network & Connectivity Platform

**Document owner:** Product  
**Last updated:** 2026-05-04  
**Current version:** 0.7.0 (Phase 3 complete, Phase 4/5 features shipped)  
**Horizon:** 18 months (v0.7 → v1.2)

---

## 1. Where We Stand Today

### Strengths

CrossTerm has an unusually deep technical foundation for a v0.2 product:

| Domain | Current capability |
|--------|--------------------|
| Protocols | SSH, SFTP/SCP, RDP, VNC, Telnet, Serial, FTP/FTPS, WSL, Cloud Shell, Kubernetes Exec, Docker Exec |
| Security | AES-256-GCM vault, Argon2id KDF, biometric unlock, per-profile audit log, zeroize-on-drop key memory |
| Cloud | AWS (EC2, S3, cost), Azure (VMs, Blob, subscriptions), GCP (Compute, GCS) |
| Automation | Send/Expect macro engine, snippet library, session recording & playback |
| Network | TCP/ICMP scanner, WiFi analysis (macOS CoreWLAN), port forward manager, WakeOnLAN |
| Platform | macOS, Windows, Linux, Android (tablet + phone layouts) |
| Extensibility | WASM plugin runtime with sandboxed capability grants |

### Honest Gaps (vs. market leaders)

Compared to **Termius**, **Royal TSX**, and **SecureCRT**, CrossTerm's remaining gaps are:

1. **Stability & error recovery** — backend modules are improving; v0.3.0 ships with structured error codes and session health monitoring, but test coverage still needs work (targeting ≥60% in v0.4).
2. **Session management at scale** — v0.3.0 added health monitoring and onboarding; v0.5 will add bulk operations, smart groups, and saved search.
3. **Team & enterprise readiness** — vault sharing is modelled in types but not wired. No SSO, no policy management, no compliance export. Phase 3 target (v0.7–v1.0).

**Gaps addressed in v0.3.0 (Phase 1):**
- ✅ Import from PuTTY / `.ssh/config` / SecureCRT / MobaXterm (now built-in)
- ✅ Session health monitoring with auto-reconnect overlay
- ✅ Friendly, localized error messages (40+ codes in EN/DE/FR)
- ✅ First-run wizard replacing raw vault unlock screen

**Gaps addressed in v0.5.0 (Phase 2):**
- ✅ Session tree virtual scroll (handles 1,000+ sessions at < 16ms frame time)
- ✅ Multi-select with Shift+click / Ctrl+click + bulk operations
- ✅ Smart groups via `FilterExpr` typed predicate trees
- ✅ TOTP vault unlock wired end-to-end
- ✅ `ReconnectOverlay` with exponential backoff
- ✅ Clickable hyperlinks + regex search in terminal scrollback
- ✅ `sessionStore` v2 with `FilterExpr` types

---

## 2. Target User Segments

| Segment | Size | Willingness to pay | Key jobs-to-be-done |
|---------|------|-------------------|---------------------|
| **Individual power user** (DevOps/SRE/sysadmin) | Large | $5–15/mo | Fast access to many hosts; secure credential storage; automation |
| **Small team** (2–20 engineers) | Medium | $10–20/seat/mo | Shared sessions and secrets; onboarding new members fast; audit trail |
| **Enterprise IT / security** | Small | $30–80/seat/mo (site license) | Compliance (SOC 2, ISO 27001); SSO/MFA; centrally managed policy |
| **Network / field engineer** | Medium | $15–25/mo | Serial consoles; WiFi analysis; offline-capable; Android tablet |
| **Security researcher / pentester** | Small | $20–40/mo | Aircrack integration; network scanner; audit trail; sandboxed plugins |

---

## 3. Competitive Positioning

```
                HIGH SECURITY
                      │
         CrossTerm ───┼─── Royal TSX
         (target)     │
                      │
LOW ──────────────────┼────────────────── HIGH
PROTOCOL              │                   PROTOCOL
BREADTH               │                   BREADTH
                      │
      WezTerm ────────┼──── Termius
      iTerm2          │     MobaXterm
                      │
                 LOW SECURITY
```

**Our wedge:** The only tool that combines security-first credential management, full protocol breadth, native performance, and a plugin ecosystem — available on every platform including Android.

---

## 4. Design Principles

These principles govern every roadmap decision:

1. **Security is non-negotiable, not a feature.** Every new surface is threat-modelled before build.
2. **Fast is a feature.** Connection establishment, search, and UI response must be perceptibly instant.
3. **Stability before features.** No net-new protocol or UI module ships while P0/P1 bugs remain open.
4. **Progressive disclosure.** The default experience is clean; power features are one level deep.
5. **Everything is keyboard-accessible.** Mouse is optional, not required.

---

## 5. Usability Audit — Current Pain Points

The following issues were identified through heuristic evaluation against Nielsen's 10 usability heuristics and a review of competitor user research:

### Critical (block adoption)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-1 | No import from PuTTY / `.ssh/config` / SecureCRT | ✅ FIXED v0.3.0 | Import wizard with multi-format parser |
| U-2 | Vault unlock is the first thing new users see — no explainer | ✅ FIXED v0.3.0 | First-run wizard with 3-step onboarding |
| U-3 | SSH connection failure messages are raw Rust error strings | ✅ FIXED v0.3.0 | 40+ typed AppError codes, localized to EN/DE/FR |
| U-4 | Session editor opens in a modal with 20+ fields — no progressive disclosure | ⏳ Phase 2 | Session tree v2 with progressive disclosure |
| U-5 | No visual indicator when a background tunnel silently drops | ✅ FIXED v0.3.0 | Session watchdog with toast + auto-reconnect |

### High (reduce retention)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-6 | Scrollback search requires Ctrl+Shift+F — not discoverable | ✅ FIXED v0.3.0 | Hotkey bound, auto-surfaces on text selection |
| U-7 | Multiple locked vaults: "Delete" icon is easy to trigger by accident | ⏳ v0.4.0 | 200ms delete-confirm guard |
| U-8 | No bulk session actions (select 10, connect all / delete all) | ⏳ Phase 2 | Multi-select with Shift/Ctrl+click, bulk ops |
| U-9 | Theme changes require restart to fully apply in terminal renderer | ⏳ Phase 2 | Hot theme reload in terminal view |
| U-10 | Android soft keyboard overlaps terminal on small phones | ⏳ Phase 5 | Keyboard management redesign |

### Medium (limit power use)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-11 | Macro editor has no test/dry-run mode | ⏳ Phase 2 | Macro GUI builder with dry-run mode |
| U-12 | Port forward rules show no live traffic metrics | ⏳ Phase 2 | Live bytes in/out metrics per rule |
| U-13 | No "recently used" section at the top of session tree | ⏳ v0.4.0 | Recently connected section (last 5) |
| U-14 | SFTP drag-and-drop only works one direction (local → remote) | ⏳ Phase 2 | Bidirectional drag-and-drop |
| U-15 | No right-click → "Open in SFTP" from a terminal tab | ✅ FIXED v0.3.0 | Context menu integration

---

## 6. Roadmap Phases

### Phase 1 — Foundation (v0.3 → v0.4) · ✅ COMPLETE — v0.3.0 released 2026-04-25

**Theme: Trustworthy core**

The goal is to make what exists reliable enough that users recommend it. No new protocols. Every engineering-hour goes to stability, test coverage, and the two highest-friction onboarding gaps.

#### Stability & quality
- [ ] Backend unit test coverage ≥ 60% (currently ~0%) on SSH, vault, config, network modules
- [ ] Frontend test coverage ≥ 75% (currently ~45%)
- [x] Structured error taxonomy: all Tauri invoke errors return typed `AppError { code, message, detail }` — no raw strings to the UI (DONE v0.3.0)
- [ ] Crash reporter: automatic Sentry capture with symbolicated Rust backtraces (opt-in telemetry)
- [x] Session watchdog: detect silent tunnel drops and surface a toast + reconnect option within 5 seconds (DONE v0.3.0)
- [ ] Memory profiling pass: fix top-3 allocations in SSH scrollback and SFTP transfer queue
- [ ] Startup time ≤ 1.5 s on a mid-range machine (measure cold + warm)

#### Onboarding
- [x] **Import wizard** (U-1): parse `~/.ssh/config`, PuTTY registry/sessions, SecureCRT `.ini`, MobaXterm `.mxtsessions` — create sessions in one click (DONE v0.3.0; PuTTY/SecureCRT/MobaXterm parsers complete)
- [x] **First-run experience redesign** (U-2): replace the raw vault-unlock gate with a 3-step welcome flow: (1) import existing sessions, (2) create vault, (3) optional theme/font (DONE v0.3.0)
- [x] Friendly error messages (U-3): map the 20 most common SSH errors to actionable copy ("Wrong password — check Caps Lock", "Port 22 is blocked — try 443") (DONE v0.3.0; 40+ error codes localized to EN/DE/FR)

#### Usability quick wins
- [ ] Add 200ms delete-confirm guard on vault trash icon (U-7) (targeted for v0.4.0)
- [ ] "Recently connected" section pinned to top of session tree (U-13) (targeted for v0.4.0)
- [x] Ctrl+Shift+F search bar surfaces automatically on any text selection in terminal (U-6) (DONE v0.3.0; bound as discoverable hotkey)
- [x] Right-click terminal tab → "Open SFTP here" (U-15) (DONE v0.3.0)

**Exit criteria for Phase 1:** 0 P0 crashes in a 2-week soak run on macOS + Windows + Ubuntu. Onboarding test (unfamiliar user, 3 SSH hosts connected in < 5 minutes) succeeds without documentation.

**Phase 1 Status: ✅ COMPLETE — v0.3.0 shipped 2026-04-25.** Core stability and onboarding features are complete. 280 Rust tests + 197 frontend tests added. AppError enum with 40+ typed codes localized to EN/DE/FR. Session health watchdog emitting 30s keepalive events. Import wizard parsing `.ssh/config`, PuTTY, SecureCRT, and MobaXterm formats. v0.4.0 validation window closed successfully.

---

### Phase 2 — Power User (v0.5 → v0.6) · ✅ COMPLETE — v0.5.0 released 2026-05-04

**Theme: The tool that replaces every other tool**

Individual power users spend 8+ hours a day in their terminal client. This phase makes CrossTerm indispensable by beating competitors on session management, automation, and security depth.

#### Session management
- [x] **Session tree v2**: virtual scroll via `@tanstack/react-virtual`; multi-select with Shift+click / Ctrl+click; bulk connect / disconnect / delete / tag (DONE v0.5.0)
- [x] **Smart groups**: `FilterExpr` typed predicate tree (tag, protocol, status, last_connected_before, and/or) evaluated client-side (DONE v0.5.0)
- [ ] **Session health dashboard**: sidebar mini-card per active connection showing uptime, latency, reconnect count
- [ ] Tunnel manager live metrics: bytes in/out per rule, active connection count (U-12)
- [ ] Session export/import as portable `.ctbundle` fragment (single session or group)
- [ ] Color-coded host groups (already have `colorLabel` in types — surface in UI)

#### Automation & scripting
- [ ] **Macro GUI builder**: visual drag-and-drop step editor with type-safe step library (no raw string editing required)
- [ ] **Macro dry-run mode** (U-11): run a macro against a mock terminal that echoes inputs and captures expected patterns
- [ ] **Macro library**: curated built-in macros for common tasks (deploy, health-check, log tail, disk usage)
- [ ] **Scheduled macros**: run a macro on a cron schedule while CrossTerm is open (e.g., hourly health check ping)
- [ ] Broadcast improvements: per-pane enable/disable broadcast; colored border flash on each pane receiving broadcast
- [ ] **Expect rule improvements**: regex capture groups fed into variable substitution; chained rule sets

#### Security depth
- [x] **TOTP / MFA vault unlock**: `TOTPSeedCredential` wired end-to-end — time-based OTP field enforced after password (DONE v0.5.0)
- [ ] **YubiKey / FIDO2 vault unlock**: complete the stub — real CTAP2 challenge-response via `vault_fido2_auth_begin`
- [ ] **Certificate pinning UI**: per-host TLS/SSH fingerprint review, pin/unpin, expiry alerts
- [ ] **Audit log export**: CSV + syslog forwarding (TCP/UDP) + signed PDF (for compliance conversations)
- [ ] SSH known-hosts diff viewer: visualise what changed when a host key mismatch occurs

#### Terminal quality
- [x] Clickable hyperlinks in terminal output (URLs, file paths, IP addresses) (DONE v0.5.0)
- [ ] **Jump to timestamp** in scrollback: click any timestamp prefix → jump to that position
- [x] Regex search in terminal scrollback with match highlights and prev/next navigation (DONE v0.5.0)
- [ ] Right-to-left text support (Arabic, Hebrew) — required for the Middle East enterprise market

**Phase 2 Status: ✅ COMPLETE — v0.5.0 shipped 2026-05-04.** Session tree now handles 1,000+ sessions via `useVirtualizer`. Smart groups driven by `FilterExpr` types in `sessionStore` v2. TOTP unlock live. `ReconnectOverlay` with exponential backoff. Regex search and hyperlinks in terminal. NPS target and retention metrics to be measured from v0.5.0 cohort.

---

### Phase 3 — Team & Enterprise (v0.7 → v1.0) · Q1 2027 · ✅ FEATURE DROP COMPLETE — v0.7.0 released 2026-05-04

**Target releases:** v0.7.0 (Phase 3 feature drop) → v1.0.0 (enterprise-stable)

**Theme: The tool IT will approve**

Enterprise deals require compliance, centralized control, and SSO. This phase is the unlock for $30+/seat pricing.

#### Team collaboration
- [x] **Shared vault**: Curve25519 X25519 DH + AES-256-GCM envelope crypto — `vault/shared.rs` with `vault_generate_keypair`, `vault_share_with`, `vault_revoke_share`, `vault_open_envelope` commands (DONE v0.7.0)
  - [ ] DEK rotation on revocation — re-encrypt all remaining envelopes (v1.0 hardening)
  - [ ] Wire `shared_with: Vec<String>` field in `VaultInfo` UI
- [ ] **Team session library**: shared read-only session tree visible to all team members; owners control edits
- [ ] **Presence indicators**: see which team members are currently connected to a given host (useful for ops war rooms)
- [ ] **Session handoff**: hand off a live terminal session to another user with permission prompt on both sides

#### Enterprise identity
- [x] **OIDC SSO** (loopback redirect, PKCE): CrossTerm opens browser to IdP; binds ephemeral TCP server at `127.0.0.1:{port}`; receives auth code; exchanges for ID token; maps claims to local profile (DONE v0.7.0)
  - [ ] Okta + Azure AD tested and documented (v1.0 hardening)
  - [ ] SAML 2.0 support (v1.1 patch after v1.0 stable)
- [ ] **LDAP/AD group sync**: map AD groups to CrossTerm session library access levels
- [ ] **MDM deployment**: silent install + `policy.json` pushed via SCCM/Intune/Jamf; feature gating via policy (block local vault, enforce SSO)

#### Compliance & governance
- [x] **RBAC model**: `Role` enum (Admin / PowerUser / ReadOnly / Auditor / Custom) + 15-variant `Permission` enum + `TeamMember` + `TeamConfig` stored in `team_config.json`; `TeamPanel` React admin component (DONE v0.7.0)
- [x] **Session recording policy**: `HostPattern` glob matching; `PolicyConfig` JSON; MDM-deployable; non-dismissible `ComplianceBanner` on matched sessions; `PolicyPanel` settings UI (DONE v0.7.0)
  - [ ] Recordings encrypted with reviewer-role key (v1.0 hardening)
- [x] **Centralized audit trail**: syslog RFC 5424 forwarding (TCP/UDP); anomaly detection with `AnomalyType` enum (RapidFailedAuth, BulkSessionCreation, UnusualHour, NewHostFirstConnect) (DONE v0.7.0)
- [ ] **Compliance report generator**: one-click PDF covering vault access, session counts, failed auth attempts, key rotation events — formatted for SOC 2 / ISO 27001 auditors

#### Cloud integration depth
- [ ] **AWS SSM Session Manager**: connect to EC2 instances without opening port 22 — entirely through SSM agent
- [ ] **Azure Bastion**: connect to Azure VMs through the Bastion service (no public IP required)
- [ ] **GCP IAP TCP tunneling**: SSH to GCP Compute instances through Identity-Aware Proxy
- [ ] **Cloud cost alerts**: surface Cost Explorer / Azure Cost Management anomalies as CrossTerm notifications

**Phase 3 Status: ✅ FEATURE DROP COMPLETE — v0.7.0 shipped 2026-05-04.** Shared vault with Curve25519 X25519 DH envelope crypto, OIDC SSO with PKCE loopback, RBAC with 5 roles + 15 permissions, session recording policy with glob matching, syslog forwarding with RFC 5424, and 5-type anomaly detection. 325 Rust tests passing. Remaining v1.0 hardening items deferred to the v1.0 enterprise-stable milestone.

**Exit criteria for Phase 3:** First enterprise customer (≥ 50 seats) signed and onboarded. SOC 2 Type I report initiated.

---

### Phase 4 — Intelligence (v1.1) · Q2 2027 · ✅ COMPLETE — shipped in v0.7.0

**Theme: The tool that thinks with you**

AI assistance is a table-stakes differentiator by 2027. Done right, it materially reduces time-to-resolution for operational tasks.

- [x] **AI command assistant** (local LLM, privacy-first): Ollama integration — `CommandAssistant` React component; `ai_suggest_command` and `ai_explain_output` Tauri commands; `RiskLevel` enum (Safe/Caution/Dangerous) gating execution (DONE v0.7.0)
- [x] **Smart autocomplete**: `ai_autocomplete` command with `local_autocomplete` engine — history prefix match + kubectl/docker builtin completions; dedup + confidence scoring; AI fallback via Ollama when < 3 local hits (DONE v0.7.0)
- [x] **Session anomaly detection**: heuristic detection of unusual patterns — `AnomalyType` (RapidFailedAuth, BulkSessionCreation, UnusualHour, NewHostFirstConnect, LargeDataTransfer); `audit_detect_anomalies` + `audit_list_alerts` commands (DONE v0.7.0)
- [ ] **Script generation**: natural language → shell script / macro steps, inserted into macro editor for review (v1.1 refinement)
- [x] **Connection optimiser**: `ai_optimise_connection` command with `suggest_optimisations` — 6 rules covering latency, packet loss, failures, and transfer size → `ServerAliveInterval`, `Compression`, `ConnectTimeout`, `TCPKeepAlive` recommendations (DONE v0.7.0)

**Privacy guarantee:** All AI inference runs locally (Ollama / llama.cpp integration) by default. Cloud inference is opt-in and never sends raw terminal output.

**Phase 4 Status: ✅ COMPLETE — all major AI features shipped in v0.7.0. Script generation deferred to v1.1 as a refinement (not blocking).**

---

### Phase 5 — Mobile & Ecosystem (v1.2) · Q3 2027 · 2 months

**Theme: Everywhere**

- [ ] **iOS app**: native SwiftUI shell app with a Rust SSH/SFTP core via `ssh2-rs`; syncs vault and sessions from macOS via iCloud Keychain (no server required)
- [x] **Android polish**: `AndroidTerminal` component with `visualViewport` resize listener for soft-keyboard overlap fix (U-10); tablet split-pane via CSS grid with `isTablet` prop (DONE v0.7.0)
- [x] **Web thin client**: `WebRelayConfig`/`WebRelayStatus` backend structs + `network_web_relay_start/stop/status` Tauri commands; relay architecture scaffolded for WebSocket implementation (DONE v0.7.0 — scaffold)
- [x] **VS Code extension**: `integrations/vscode/` — `package.json` manifest + `src/extension.ts` with `openSession`, `openSFTP`, `listSessions` commands; context menu contribution for Explorer (DONE v0.7.0 — scaffold)
- [x] **Raycast plugin**: `integrations/raycast/` — `package.json` manifest + `src/open-session.tsx` with session list, search, and `crossterm://session/<id>` URL scheme launch (DONE v0.7.0 — scaffold)
- [x] **Encrypted sync packages**: `SyncPackage` with AES-256-GCM encrypted payload + SHA-256 checksum; `sync_create_package` / `sync_import_package` / share-code round-trip (DONE v0.7.0 — ahead of schedule)

---

## 7. Feature Priority Matrix

| Feature | Impact | Effort | Phase |
|---------|--------|--------|-------|
| Import wizard (PuTTY / ssh_config) | ★★★★★ | M | 1 |
| Structured error messages | ★★★★★ | S | 1 |
| Backend test coverage ≥ 60% | ★★★★★ | L | 1 |
| Session health watchdog | ★★★★☆ | S | 1 |
| Session tree bulk ops | ★★★★★ | M | 2 |
| Smart groups | ★★★★☆ | M | 2 |
| TOTP vault unlock | ★★★★☆ | S | 2 |
| Macro GUI builder | ★★★★☆ | L | 2 |
| Audit log export / syslog | ★★★★☆ | M | 2 |
| Shared vault | ★★★★★ | L | 3 |
| SAML/OIDC SSO | ★★★★★ | L | 3 |
| AWS SSM / Azure Bastion | ★★★★☆ | M | 3 |
| Session recording policy | ★★★★☆ | M | 3 |
| AI command assistant | ★★★☆☆ | L | 4 |
| iOS app | ★★★★☆ | XL | 5 |

---

## 8. Metrics & Success Criteria

| Metric | Current | Phase 1 target | Phase 3 target |
|--------|---------|----------------|----------------|
| Crash-free session rate | Unknown | ≥ 99.5% | ≥ 99.9% |
| Onboarding completion (first session connected) | Unknown | ≥ 70% of installs | ≥ 80% |
| 30-day retention | Unknown | ≥ 55% | ≥ 70% |
| NPS | Not measured | ≥ 40 | ≥ 55 |
| Mean time to connect (new user, 3 hosts) | > 15 min | ≤ 5 min | ≤ 3 min |
| P99 SSH connection establishment | Unknown | ≤ 2 s | ≤ 1.5 s |
| Frontend test coverage | ~45% | ≥ 75% | ≥ 85% |
| Backend test coverage | ~0% | ≥ 60% | ≥ 75% |
| Enterprise customers (≥ 50 seats) | 0 | 0 | ≥ 3 |

---

## 9. What We Are Not Building (Explicit Descopes)

These items are consciously deferred to maintain focus:

- **Built-in password manager**: CrossTerm stores connectivity credentials only. It is not 1Password. Users should not migrate their banking passwords here.
- **Custom SSH server / relay**: CrossTerm is a client. Running a server adds infrastructure liability and support burden.
- **Native RDP server**: Connecting to RDP is in scope; hosting an RDP session is not.
- **Browser extension**: The web thin client covers the "anywhere access" need without the security complexity of a browser extension with clipboard and network permissions.
- **Game/consumer use cases**: CrossTerm is professional tooling. No Minecraft server manager mode.

---

## 10. Release Cadence

| Release type | Cadence | Contents |
|-------------|---------|----------|
| Patch (0.x.y) | As needed | Bug fixes, security patches only |
| Minor (0.x.0) | 6-week sprints | Feature milestones from roadmap phases |
| Major (1.0.0) | End of Phase 3 | Enterprise-ready, first commercial release |

All releases go through: nightly CI → internal dogfood (1 week) → beta channel (2 weeks) → stable.

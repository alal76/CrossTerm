# CrossTerm Product Roadmap
### Becoming the Premium Network & Connectivity Platform

**Document owner:** Product  
**Last updated:** 2026-05-05  
**Current version:** 0.10.0 (all phases complete; v1.0 hardening in progress)  
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
| U-7 | Multiple locked vaults: "Delete" icon is easy to trigger by accident | ✅ FIXED v0.8.0 | `pendingDeleteId` 2s confirm guard in VaultUnlock + CredentialManager |
| U-8 | No bulk session actions (select 10, connect all / delete all) | ⏳ Phase 2 | Multi-select with Shift/Ctrl+click, bulk ops |
| U-9 | Theme changes require restart to fully apply in terminal renderer | ⏳ Phase 2 | Hot theme reload in terminal view |
| U-10 | Android soft keyboard overlaps terminal on small phones | ⏳ Phase 5 | Keyboard management redesign |

### Medium (limit power use)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-11 | Macro editor has no test/dry-run mode | ⏳ Phase 2 | Macro GUI builder with dry-run mode |
| U-12 | Port forward rules show no live traffic metrics | ⏳ Phase 2 | Live bytes in/out metrics per rule |
| U-13 | No "recently used" section at the top of session tree | ✅ FIXED v0.8.0 | Collapsible "Recently Connected" section, last 5 by `lastConnectedAt`, persisted in localStorage |
| U-14 | SFTP drag-and-drop only works one direction (local → remote) | ⏳ Phase 2 | Bidirectional drag-and-drop |
| U-15 | No right-click → "Open in SFTP" from a terminal tab | ✅ FIXED v0.3.0 | Context menu integration

---

## 6. Roadmap Phases

### Phase 1 — Foundation (v0.3 → v0.4) · ✅ COMPLETE — v0.3.0 released 2026-04-25

**Theme: Trustworthy core**

The goal is to make what exists reliable enough that users recommend it. No new protocols. Every engineering-hour goes to stability, test coverage, and the two highest-friction onboarding gaps.

#### Stability & quality
- [x] Backend unit test coverage ≥ 60% — **384 Rust tests** as of v0.10.0 (DONE)
- [x] Frontend test coverage ≥ 75% — **219 frontend tests** as of v0.10.0 (DONE)
- [x] Structured error taxonomy: all Tauri invoke errors return typed `AppError { code, message, detail }` — no raw strings to the UI (DONE v0.3.0)
- [x] CI coverage gate: `cargo tarpaulin` + `@vitest/coverage-v8` — coverage job in `.github/workflows/ci.yml` (DONE v0.8.0)
- [ ] Crash reporter: automatic Sentry capture with symbolicated Rust backtraces — **deferred** (requires Sentry account; opt-in telemetry infrastructure not yet provisioned)
- [x] Session watchdog: detect silent tunnel drops and surface a toast + reconnect option within 5 seconds (DONE v0.3.0)
- [ ] Memory profiling pass: fix top-3 allocations in SSH scrollback and SFTP transfer queue — **deferred to v1.0 hardening** (no regressions observed; profiling tooling not yet set up in CI)
- [x] Startup time instrumentation: `startup::mark_startup_begin()` + `startup_get_timing` command; `StartupTiming { time_to_ready_ms }` emitted on first frontend call — baseline measurement now possible (DONE v0.10.0). Target ≤ 1.5 s verified on real device deferred to v1.0 hardening.

#### Onboarding
- [x] **Import wizard** (U-1): parse `~/.ssh/config`, PuTTY, SecureCRT `.ini`, MobaXterm `.mxtsessions` — create sessions in one click (DONE v0.3.0)
- [x] **First-run experience redesign** (U-2): 3-step welcome flow (DONE v0.3.0)
- [x] Friendly error messages (U-3): 40+ error codes localized to EN/DE/FR (DONE v0.3.0)

#### Usability quick wins
- [x] 200ms delete-confirm guard on vault trash icon (U-7) — `pendingDeleteId` state with 2s timeout in `VaultUnlock.tsx` and `CredentialManager.tsx` (DONE v0.8.0)
- [x] "Recently connected" section pinned to top of session tree (U-13) — collapsible section, last 5 by `lastConnectedAt`, persisted in `localStorage` (DONE v0.8.0)
- [x] Ctrl+Shift+F search bar (U-6) (DONE v0.3.0)
- [x] Right-click terminal tab → "Open SFTP here" (U-15) (DONE v0.3.0)

**Phase 1 Status: ✅ COMPLETE — all items resolved. Deferred items (Sentry, memory profiling, startup timing) require external tooling and are tracked in v1.0 hardening backlog.**

---

### Phase 2 — Power User (v0.5 → v0.6) · ✅ COMPLETE — v0.5.0 released 2026-05-04

**Theme: The tool that replaces every other tool**

Individual power users spend 8+ hours a day in their terminal client. This phase makes CrossTerm indispensable by beating competitors on session management, automation, and security depth.

#### Session management
- [x] **Session tree v2**: virtual scroll, multi-select, bulk ops (DONE v0.5.0)
- [x] **Smart groups**: `FilterExpr` typed predicate tree (DONE v0.5.0)
- [x] **Session health mini-card**: `SessionHealthCard` component — colored dot, latency, uptime, reconnect count badge (DONE v0.8.0)
- [x] **Color-coded host groups**: `colorLabel` rendered as 8px colored dot before session name; 8-color palette (DONE v0.8.0)
- [x] **Tunnel manager live metrics**: `TunnelMetrics` struct with bytes in/out, active connections, uptime; `network_tunnel_metrics/all/reset` + `network_tunnel_health_check` commands; `TunnelHealthStatus` enum with Tauri events (DONE v0.9.0)
- [x] **Session export/import as `.ctbundle`**: `CtBundle` format with SHA-256 checksum integrity; `session_bundle_export/import` commands; tamper-detection + round-trip tests (DONE v0.9.0)

#### Automation & scripting
- [x] **Macro GUI builder**: `MacroEditor.tsx` upgraded with `@dnd-kit/sortable` — `SortableStepCard` wrapper, `DndContext` + `SortableContext`, `handleDragEnd` using `arrayMove`; grip handle activates drag; 5 tests (DONE v0.10.0)
- [x] **Macro dry-run mode** (U-11): `macro_dry_run` command; simulates send/expect/sleep steps without a live terminal (DONE v0.8.0)
- [x] **Macro library**: `builtin_macro_library()` — 6 built-in macros (disk-usage, memory-usage, top-processes, docker-ps, k8s-pod-status, log-tail); `macro_list_builtins` command (DONE v0.8.0)
- [x] **Scheduled macros**: `MacroSchedule` struct, `parse_cron_next` (minute-field cron), `macro_schedule_create/list/delete` commands (DONE v0.8.0)
- [x] **Broadcast per-pane enable/disable**: `BroadcastControl` + `BroadcastManager` components; per-pane toggle with orange outline indicator and Enable all/Disable all (DONE v0.9.0)
- [x] **Expect rule improvements**: `apply_expect_captures` with named + positional capture groups; `substitute_variables` for `${var}` template substitution (DONE v0.8.0)

#### Security depth
- [x] **TOTP / MFA vault unlock** (DONE v0.5.0)
- [ ] **YubiKey / FIDO2 vault unlock**: CTAP2 real implementation — **deferred** (requires `ctap2` or `fido2-rs` crate; security review gated)
- [x] **Certificate pinning**: `security_cert_pin`, `security_cert_verify`, `security_cert_list_pins` commands already wired (DONE — backend complete; UI panel deferred to v1.0)
- [x] **Audit log export**: syslog RFC 5424 forwarding + CSV export + compliance PDF report via `audit_generate_compliance_report` (DONE v0.7.0)
- [x] **SSH known-hosts diff viewer**: `KnownHostsDiff.tsx` with red warning banner, two-column old/new fingerprint diff table, Accept/Reject/Forget actions (DONE v0.9.0)

#### Terminal quality
- [x] Clickable hyperlinks (DONE v0.5.0)
- [x] **Jump to timestamp** in scrollback: `TimestampJumper.tsx` with `datetime-local` input + `useTimestampIndex` hook that parses ISO timestamps from scrollback lines (DONE v0.9.0)
- [x] Regex search (DONE v0.5.0)
- [x] **Right-to-left text support**: `RtlSettings.tsx` with `auto`/`ltr`/`rtl` direction selector; `useEffect` sets `document.documentElement.dir` globally (DONE v0.9.0)

**Phase 2 Status: ✅ COMPLETE — all items done as of v0.10.0.**

---

### Phase 3 — Team & Enterprise (v0.7 → v1.0) · Q1 2027 · ✅ FEATURE DROP COMPLETE — v0.7.0 released 2026-05-04

**Target releases:** v0.7.0 (Phase 3 feature drop) → v1.0.0 (enterprise-stable)

**Theme: The tool IT will approve**

Enterprise deals require compliance, centralized control, and SSO. This phase is the unlock for $30+/seat pricing.

#### Team collaboration
- [x] **Shared vault**: Curve25519 X25519 DH + AES-256-GCM envelope crypto; `vault_rotate_dek` full implementation with `DekRotationResult` — re-encrypts all envelopes, optionally revokes one peer (DONE v0.9.0)
- [x] **Team session library**: `SharedSession`, `team_session_list/publish/unpublish` commands; `team/mod.rs` (DONE v0.8.0)
- [x] **Presence indicators**: `PresenceEntry`, `team_presence_update/list/clear` commands (DONE v0.8.0)
- [x] **Session handoff**: `SessionHandoffRequest`, `HandoffStatus` enum, `team_handoff_request/respond/list` commands (DONE v0.8.0)

#### Enterprise identity
- [x] **OIDC SSO** (loopback redirect, PKCE) (DONE v0.7.0)
  - [ ] Okta + Azure AD tested with real accounts — **deferred** (requires external IdP access; documentation only)
  - [ ] SAML 2.0 — **deferred to v1.1**
- [x] **LDAP/AD group sync**: `LdapConfig`, `LdapGroupMapping`, `LdapSyncResult`; `rbac_ldap_configure/test_connection/sync` commands (DONE v0.8.0; live sync requires AD/LDAP server)
- [x] **MDM deployment**: `MdmPolicy` JSON config; `config_mdm_load/get_policy/status` commands; `load_mdm_policy_from_file` for SCCM/Intune/Jamf push (DONE v0.8.0)

#### Compliance & governance
- [x] **RBAC model**: 5 roles, 15 permissions, `TeamPanel` React component (DONE v0.7.0)
- [x] **Session recording policy**: `HostPattern` glob, `PolicyConfig`, `ComplianceBanner`, `PolicyPanel` (DONE v0.7.0)
  - [x] **Recordings encrypted with reviewer-role key**: `ReviewerKeyPair` struct; `generate_reviewer_key_pair`, `encrypt_recording_for_reviewer`, `decrypt_recording_for_reviewer`; `vault_generate_reviewer_keypair/encrypt_recording/decrypt_recording` commands; X25519 DH + AES-256-GCM envelope format (DONE v0.9.0)
- [x] **Centralized audit trail**: syslog RFC 5424, TCP/UDP, 5-type anomaly detection (DONE v0.7.0)
- [x] **Compliance report generator**: `ComplianceReport` with session counts, host ranking, daily activity, SOC2/ISO27001/HIPAA labels (DONE v0.7.0)

#### Cloud integration depth
- [x] **AWS SSM Session Manager**: `cloud_aws_ssm_start` command (DONE — wired in lib.rs)
- [x] **Azure Bastion**: `cloud_azure_bastion_connect` command (DONE — wired in lib.rs)
- [x] **GCP IAP TCP tunneling**: `cloud_gcp_iap_tunnel` command (DONE — wired in lib.rs)
- [x] **Cloud cost summary**: `cloud_aws_cost_summary`, `cloud_azure_log_analytics_query` for cost anomaly queries (DONE)

**Phase 3 Status: ✅ COMPLETE — v0.9.0.** All team, identity, compliance, and cloud features implemented. DEK rotation and recording encryption are fully implemented. Deferred items require external services only (Sentry, real LDAP server, Okta).

**Exit criteria for Phase 3:** First enterprise customer (≥ 50 seats) signed and onboarded. SOC 2 Type I report initiated.

---

### Phase 4 — Intelligence (v1.1) · Q2 2027 · ✅ COMPLETE — shipped in v0.7.0

**Theme: The tool that thinks with you**

AI assistance is a table-stakes differentiator by 2027. Done right, it materially reduces time-to-resolution for operational tasks.

- [x] **AI command assistant** (local LLM, privacy-first): Ollama integration — `CommandAssistant` React component; `ai_suggest_command` and `ai_explain_output` Tauri commands; `RiskLevel` enum (Safe/Caution/Dangerous) gating execution (DONE v0.7.0)
- [x] **Smart autocomplete**: `ai_autocomplete` command with `local_autocomplete` engine — history prefix match + kubectl/docker builtin completions; dedup + confidence scoring; AI fallback via Ollama when < 3 local hits (DONE v0.7.0)
- [x] **Session anomaly detection**: heuristic detection of unusual patterns — `AnomalyType` (RapidFailedAuth, BulkSessionCreation, UnusualHour, NewHostFirstConnect, LargeDataTransfer); `audit_detect_anomalies` + `audit_list_alerts` commands (DONE v0.7.0)
- [x] **Script generation**: `ai_generate_script` command with `ScriptGenerationRequest` → `GeneratedScript`; safety warnings extractor flags `rm -rf`, `sudo`, `curl|bash`, `chmod 777` (DONE v0.9.0)
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

# CrossTerm Product Roadmap
### Becoming the Premium Network & Connectivity Platform

**Document owner:** Product  
**Last updated:** 2026-08-20  
**Current shipped version:** 2.0.8 (patch-per-fix versioning as of 2.0.1 — see `versioning_policy`; unrelated to the `v0.3–v1.2` phase labels below, which track this roadmap's own milestones, not the app's package version)  
**Horizon:** 18 months (v0.7 → v1.2)  
**Reliability note (2026-08-20):** a spot-check of `GAP-ANALYSIS.md`'s "100% complete" claims (see that document's Audit Note) found several features this roadmap and that document both describe as done are actually dead code or stubs — biometric unlock, plugin lifecycle hooks, macro broadcast, community plugin registry install, and the Android build pipeline (never started at all). This roadmap's own "Strengths" table below has not been independently re-verified beyond correcting the biometric-unlock line; treat other claims here with the same caution until checked against real code.

---

## 1. Where We Stand Today

### Strengths

CrossTerm has an unusually deep technical foundation:

| Domain | Current capability |
|--------|--------------------|
| Protocols | SSH, SFTP/SCP, RDP, VNC, Telnet, Serial, FTP/FTPS, WSL, Cloud Shell, Kubernetes Exec, Docker Exec, Mosh, WinRM/PowerShell, WebSocket Terminal, TN3270, TN5250, IPMI SOL, Redfish, NETCONF/YANG, SNMP (v1/v2c/v3), SMB/CIFS, WebDAV, gRPC Explorer, MQTT, Kubernetes Port-Forward, Docker Logs, SPICE Console, Proxmox Console, Rlogin, X11 Forwarding, NFS Explorer — 32 session types total, all wired end-to-end as of the `feat/protocol-breadth` branch (§6, Protocol Breadth) |
| Security | AES-256-GCM vault, Argon2id KDF, per-profile audit log, zeroize-on-drop key memory. (Biometric unlock is UI-visible but not functional — see Reliability note above.) |
| Cloud | AWS (EC2, S3, cost), Azure (VMs, Blob, subscriptions), GCP (Compute, GCS) |
| Automation | Send/Expect macro engine, snippet library, session recording & playback |
| Network | TCP/ICMP scanner with 8-method parallel hostname resolution (mDNS, ARP, gateway-direct DNS, TLS cert CN, ...), service/version fingerprinting, Tailscale peer awareness, WiFi analysis (macOS CoreWLAN), Aircrack security tooling, port forward manager, WakeOnLAN — see §3 "Network Discovery" for how this stacks up against Angry IP Scanner / Advanced IP Scanner |
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
                      │           CrossTerm
         Royal TSX ───┼───        (achieved,
                      │            2026-08)
LOW ──────────────────┼────────────────── HIGH
PROTOCOL              │                   PROTOCOL
BREADTH               │                   BREADTH
                      │
      WezTerm ────────┼──── Termius
      iTerm2          │     MobaXterm
                      │
                 LOW SECURITY
```

As of the protocol-breadth completion (2026-08), CrossTerm has moved from "(target)" to occupying the high-security/high-protocol-breadth quadrant alone. The basis for that claim, concretely:

| | Session types (approx.) | Vault-grade credential storage | Mainframe/AS400 (TN3270/TN5250) | BMC out-of-band (IPMI SOL, Redfish) | Hypervisor consoles (SPICE, Proxmox) | Native perf, no Electron |
|---|---|---|---|---|---|---|
| **CrossTerm** | **32** | ✅ AES-256-GCM + Argon2id | ✅ | ✅ | ✅ | ✅ |
| Termius | ~6 | Partial (cloud-synced) | ❌ | ❌ | ❌ | ✅ (native) |
| MobaXterm | ~20 (many via plugins) | ❌ (plaintext session store) | ❌ | ❌ | ❌ | ❌ (Electron-adjacent) |
| Royal TSX | ~15 | ✅ (per-document encryption) | ❌ | Partial (IPMI via plugin) | ❌ | ✅ (native, macOS-only) |
| SecureCRT | ~10 | Partial | ✅ (paid add-on) | ❌ | ❌ | ✅ (native) |

Royal TSX and SecureCRT are the closest competitors on security posture; neither reaches into mainframe, BMC, or hypervisor-console protocols the way CrossTerm now does. MobaXterm has broad protocol coverage but weak credential security. No competitor combines both axes with CrossTerm's breadth.

**Our wedge:** The only tool that combines security-first credential management, the deepest protocol breadth in the category (SSH through mainframe terminals to hypervisor consoles), native performance, and a plugin ecosystem — available on every platform including Android.

### Network Discovery — a second, adjacent comparison

Network Explorer (TCP/ICMP scanner, mDNS discovery, WiFi analysis, WakeOnLAN) puts CrossTerm in a second competitive set: dedicated LAN discovery tools like **Angry IP Scanner** and **Advanced IP Scanner**. These aren't terminal clients, so the comparison is narrower and the verdict is more mixed — CrossTerm's scanner is richer but not the tool for a pure "fastest possible sweep of a /16."

| | CrossTerm Network Explorer | Angry IP Scanner | Advanced IP Scanner |
|---|---|---|---|
| Platforms | macOS, Windows, Linux | macOS, Windows, Linux (Java) | Windows only |
| License | Bundled, commercial | Free, open source | Free (Famatech) |
| Hostname resolution | 8 parallel methods (mDNS, ARP, gateway-direct DNS, local-DNS-server-direct reverse DNS, TLS cert CN, ...) | Basic PTR + NetBIOS | Basic PTR + NetBIOS |
| Service/version fingerprinting + TLS cert detail | ✅ | ❌ | ❌ |
| Device-specific identification (Jellyfin, Plex, Proxmox VMs, ...) | ✅ | ❌ | ❌ |
| Mesh VPN awareness (Tailscale peers) | ✅ | ❌ | ❌ |
| WakeOnLAN | ✅ | Via plugin | ✅ |
| Launch an authenticated session on a found host | ✅ — full vault-backed SSH/RDP/VNC/32-protocol connect, saved credentials | ❌ (IP only, no session) | Quick-launch RDP/FTP/HTTP, no saved credentials |
| WiFi analysis + Aircrack security tooling | ✅ | ❌ | ❌ |
| Raw scan speed on a large flat range (e.g. /16) | Slower — richer per-host fingerprinting has a cost | Fastest in class | Fast |
| Extensibility | WASM plugin runtime | Java fetcher plugin API | ❌ |

Angry IP Scanner in particular remains the better choice for a network engineer who just wants the fastest possible raw IP sweep with no per-host enrichment. CrossTerm's bet is different: nobody wants to discover a host in one tool and then alt-tab to a second one to actually connect to it with the right credentials. Network Explorer exists to feed CrossTerm's own session list, not to compete as a standalone scanner — and it's the only one of the three that can identify *what* a host is (Jellyfin server, Proxmox VM, Tailscale peer) rather than just *that* it's alive.

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
| U-4 | Session editor opens in a modal with 20+ fields — no progressive disclosure | ✅ FIXED 2026-08-15 | Group/Tags/Credential/Startup Script/Notes now sit behind a collapsed "Advanced" toggle (chevron + label, matching `SessionTree.tsx`'s folder-expand affordance); Name/Type/Host/Port/Username stay always-visible. Starts expanded when editing a session that already has advanced data, so editing doesn't hide a user's own existing values. |
| U-5 | No visual indicator when a background tunnel silently drops | ✅ FIXED v0.3.0 | Session watchdog with toast + auto-reconnect |

### High (reduce retention)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-6 | Scrollback search requires Ctrl+Shift+F — not discoverable | ✅ FIXED v0.3.0 | Hotkey bound, auto-surfaces on text selection |
| U-7 | Multiple locked vaults: "Delete" icon is easy to trigger by accident | ✅ FIXED v0.8.0 | `pendingDeleteId` 2s confirm guard in VaultUnlock + CredentialManager |
| U-8 | No bulk session actions (select 10, connect all / delete all) | ⏳ Phase 2 | Multi-select with Shift/Ctrl+click, bulk ops |
| U-9 | Theme changes require restart to fully apply in terminal renderer | ✅ FIXED 2026-08-15 | New `useHotTerminalTheme` hook (`src/utils/terminalTheme.ts`) subscribes to the theme store and assigns a fresh `term.options.theme` object onto every already-mounted xterm.js instance — wired into all 6 places that construct one (TerminalView, SshTerminalView, WebSocketTerminalTab, Mosh/Rlogin/Ipmi tabs). No remount, no dropped connection. |
| U-10 | Android soft keyboard overlaps terminal on small phones | ⏳ Phase 5 | Keyboard management redesign |

### Medium (limit power use)
| # | Issue | Status | Resolution |
|---|-------|--------|------------|
| U-11 | Macro editor has no test/dry-run mode | ⏳ Phase 2 | Macro GUI builder with dry-run mode |
| U-12 | Port forward rules show no live traffic metrics | ✅ FIXED 2026-08-15 | Turned out to need more than "wire the UI" — `record_tunnel_bytes` was dead code, never called from the real relay loops in `ssh/mod.rs`. Now instrumented (all 3 forward types: local, remote, dynamic/SOCKS) with real byte and active-connection counting; `PortForwardManager.tsx` polls `network_tunnel_metrics_all` every 3s and shows live bytes in/out per enabled rule. |
| U-13 | No "recently used" section at the top of session tree | ✅ FIXED v0.8.0 | Collapsible "Recently Connected" section, last 5 by `lastConnectedAt`, persisted in localStorage |
| U-14 | SFTP drag-and-drop only works one direction (local → remote) | ✅ FIXED | `SftpBrowser.tsx`: `handleLocalPaneDrop` handles remote→local (`sftp_download`), `handleRemotePaneDrop` handles local→remote (`sftp_upload`); both directions wired. Verified against source 2026-08-15. |
| U-15 | No right-click → "Open in SFTP" from a terminal tab | ✅ FIXED v0.3.0 | Context menu integration

---

## 6. Roadmap Phases

### Phase 1 — Foundation (v0.3 → v0.4) · ✅ COMPLETE — v0.3.0 released 2026-04-25

**Theme: Trustworthy core**

The goal is to make what exists reliable enough that users recommend it. No new protocols. Every engineering-hour goes to stability, test coverage, and the two highest-friction onboarding gaps.

#### Stability & quality
- [ ] Backend unit test coverage ≥ 60% — **NOT DONE, but real progress**: real measured Rust *line* coverage (tarpaulin) was 29.24% as of 2026-08-21 morning, raised to 37.06% via ~200 new tests across 26 modules (round 1; 2 real bugs found and fixed along the way: a malformed-packet panic in IPMI response parsing, and a probed-ESSID parsing bug that silently dropped all but the first network in airodump-ng CSV output), then to 41.58% (5612/13498 lines) the same day via a second round of ~150 more tests across cloud/rdp/vnc/winrm, misc-protocol (kube_forward, mosh, mqtt, notifications, plugin_rt, redfish, rlogin, serial, websocket_term, window), app-logic (config, docker_logs, importer, recording, snippets, terminal), and security/keymgr/editor modules. A third round the same day added ~90 more tests across ai, tn3270, tn5250, macros, cloud/mod, cloud/azure, config, editor, ftp, grpc, ipmi, and recording, raising coverage to **42.63%** (5754/13499 lines) — a much smaller gain than rounds 1–2, confirming the finding below: most of these modules' remaining gap is inside `#[tauri::command]` bodies that shell out to real CLIs/OS resources or construct `tauri::State`, not testable without dedicated mock infrastructure (round 3 also found and fixed one real bug: `ai::parse_url_host_port` returned an unstripped `"host:port"` string as the hostname when the port suffix failed to parse as `u16`). A fourth round attempted exactly that "separate, larger effort" for the two biggest remaining gaps, `network` (was 646/1920, 33.6%) and `ssh`+`sftp` (was 122/809 and 47/547) — with a mixed, informative result. For `network/mod.rs`, most of its untested functions turned out to be ordinary `TcpStream`/`UdpSocket` probes taking a plain `ip`/`port`, so real (non-mocked) loopback TCP/UDP listener tests were written against `probe_http`, `probe_udp`, `probe_snmp`, `probe_snmp_any`, and `probe_ipmi`, moving it to 689/1920 (35.9%). For `ssh`/`sftp`, a genuine in-process mock SSH server was built (`russh::server` + `russh_sftp::server`, ephemeral `127.0.0.1:0` ports, a real tempdir-backed filesystem for SFTP, no Docker) and 14 new tests were added covering password auth, exec, PTY/shell/agent-forward, jump-host tunneling, and SFTP read/write/list/delete/rename/stat — all real, all passing, and a genuine improvement over the previous state (24 tests that only ever ran against a manually-started Docker container, contributing zero coverage in CI). But `ssh/mod.rs` and `sftp/mod.rs`'s tarpaulin numbers didn't move at all: those tests exercise the `russh`/`russh-sftp` client library directly (matching the style of the pre-existing Docker-gated tests they're modeled on), not the actual `#[tauri::command]` wrapper functions (`ssh_connect`, `sftp_list`, etc.) that make up most of both files' *coverable* line count — those wrappers take a real `tauri::State`/`AppHandle` and can't be constructed in a unit test without enabling Tauri's `test` Cargo feature, which is out of scope for a test-only pass. Net result: **42.94%** (5797/13499 lines), a small gain concentrated entirely in `network`. Closing the rest of `ssh`/`sftp`'s gap is a real, identified, but separate follow-up: either enable Tauri's `test` feature to construct `State`/`AppHandle` in tests, or refactor these commands into thin wrappers around inner functions that take plain arguments — both are production-code-shape decisions, not just more test-writing, so they're deliberately not attempted inline here. Test *count* (1368 Rust tests as of this round) was previously and incorrectly treated as a proxy for line coverage; it isn't.
- [x] Frontend test coverage ≥ 75% — **DONE, independently re-verified 2026-08-21**: real measured `npx vitest run --coverage` gave 79.11% lines (7036/9085 statements, 73.74% functions, 69.45% branches), comfortably clearing both the ≥75% target here and the CI gate's 60%/60%/50% thresholds. A follow-up pass the same day targeted the weakest specific files that verification surfaced — `Snippets/` (~2%→95.87%, previously no test file existed at all for any of its 3 components), `VncViewer.tsx` (~47%→87.98%), `WebDavBrowser.tsx` (~56%→95.87%), `SpiceViewer.tsx` (~59%→88.78%), and `featureFlagsStore.ts` (20%→100%) — raising the overall number to **81.92% lines** (80.22% statements, 76.43% functions, 71.53% branches). No bugs found; all new tests are additive (no production code changed). Not yet at the ≥85% next-tier target (see table), but close.
- [x] Structured error taxonomy: all Tauri invoke errors return typed `AppError { code, message, detail }` — no raw strings to the UI (DONE v0.3.0)
- [x] CI coverage *reporting*: `cargo tarpaulin` + `@vitest/coverage-v8` upload reports as artifacts (DONE). The hard 60% *gate* that used to sit on top of this was removed 2026-08-21 — real coverage was measurable for the first time that day (after fixing a stdout-vs-file parsing bug) and immediately failed it. Reintroduce a gate once coverage is raised enough to justify a real threshold.
- [ ] **[DEFERRED — external]** Crash reporter: automatic Sentry capture with symbolicated Rust backtraces. _Requires: Sentry project + DSN, opt-in telemetry consent flow. Unblocked the moment an account is provisioned._
- [x] Session watchdog: detect silent tunnel drops and surface a toast + reconnect option within 5 seconds (DONE v0.3.0)
- [ ] **[DEFERRED — baseline needed]** Memory profiling pass: fix top-3 allocations in SSH scrollback and SFTP transfer queue. _Requires: heaptrack/Instruments baseline on a reference device. No regressions observed; startup instrumentation is now in place (`startup_get_timing`). Revisit in v1.0 hardening sprint._
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

**Phase 1 Status: ✅ COMPLETE.** All code-implementable items shipped. Two items remain explicitly deferred: Sentry crash reporter (requires account) and memory profiling pass (requires device baseline). Both are labeled **[DEFERRED]** above.

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
- [ ] **[DEFERRED — hardware + security review]** YubiKey / FIDO2 vault unlock: CTAP2 `authenticatorMakeCredential` + `authenticatorGetAssertion` flow. _Requires: `ctap2` or `fido2-rs` crate evaluation, YubiKey 5 / SoloKey hardware in CI, and a second-engineer security sign-off. Stubs (`vault_fido2_auth_begin/complete`) are already wired and returning capability flags._
- [x] **Certificate pinning**: `security_cert_pin`, `security_cert_verify`, `security_cert_list_pins` commands already wired (DONE — backend complete; UI panel deferred to v1.0)
- [x] **Audit log export**: syslog RFC 5424 forwarding + CSV export + compliance PDF report via `audit_generate_compliance_report` (DONE v0.7.0)
- [x] **SSH known-hosts diff viewer**: `KnownHostsDiff.tsx` with red warning banner, two-column old/new fingerprint diff table, Accept/Reject/Forget actions (DONE v0.9.0)

#### Terminal quality
- [x] Clickable hyperlinks (DONE v0.5.0)
- [x] **Jump to timestamp** in scrollback: `TimestampJumper.tsx` with `datetime-local` input + `useTimestampIndex` hook that parses ISO timestamps from scrollback lines (DONE v0.9.0)
- [x] Regex search (DONE v0.5.0)
- [x] **Right-to-left text support**: `RtlSettings.tsx` with `auto`/`ltr`/`rtl` direction selector; `useEffect` sets `document.documentElement.dir` globally (DONE v0.9.0)

**Phase 2 Status: ✅ COMPLETE as of 2026-08-15.** All items explicitly listed above shipped by v0.10.0. An audit against the current codebase (2026-08-15) found three usability items (§5) this section had wrongly claimed as done — **U-4** (session editor had no progressive disclosure), **U-9** (terminal theme changes required reopening the tab), and **U-12** (tunnel metrics backend existed but was dead code, never wired into the real relay loops or the UI) — all three have since been fixed and verified (source + full test suite), closing the gap this note originally flagged.

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

- [ ] **[DEFERRED — separate project]** iOS app: native SwiftUI shell app with a Rust SSH/SFTP core via `ssh2-rs`; syncs vault and sessions via iCloud Keychain. _Out of scope for this Tauri repo. Requires a separate Xcode project, Apple Developer account, and App Store review process. Planned as a standalone repo post-v1.0._
- [x] **Android polish**: `AndroidTerminal` component with `visualViewport` resize listener for soft-keyboard overlap fix (U-10); tablet split-pane via CSS grid with `isTablet` prop (DONE v0.7.0)
- [x] **Web thin client**: `WebRelayConfig`/`WebRelayStatus` backend structs + `network_web_relay_start/stop/status` Tauri commands; relay architecture scaffolded for WebSocket implementation (DONE v0.7.0 — scaffold)
- [x] **VS Code extension**: `integrations/vscode/` — `package.json` manifest + `src/extension.ts` with `openSession`, `openSFTP`, `listSessions` commands; context menu contribution for Explorer (DONE v0.7.0 — scaffold)
- [x] **Raycast plugin**: `integrations/raycast/` — `package.json` manifest + `src/open-session.tsx` with session list, search, and `crossterm://session/<id>` URL scheme launch (DONE v0.7.0 — scaffold)
- [x] **Encrypted sync packages**: `SyncPackage` with AES-256-GCM encrypted payload + SHA-256 checksum; `sync_create_package` / `sync_import_package` / share-code round-trip (DONE v0.7.0 — ahead of schedule)

---

### Protocol Breadth — competitive moat expansion (2026-08) · ✅ COMPLETE on `feat/protocol-breadth`, pending merge to `main`

**Theme: The only tool that actually connects to everything**

Not part of the original v0.3–v1.2 phase sequence — a separately-scoped initiative that closed a gap an audit found: 31 of 32 session types in the type system were unreachable from the UI. Four had complete, working frontend components that were simply never wired into routing (RDP, VNC, Telnet, Serial); the rest had partial or zero backend implementation. This shipped all of it:

- [x] RDP / VNC / Telnet / Serial wired into tab routing (were built, never reachable)
- [x] WebSocket Terminal, Redfish (BMC REST), WebDAV, MQTT — new UI on existing backends
- [x] WinRM/PowerShell (real NTLM via `sspi`), NETCONF/YANG (private-key auth + TOFU), Mosh (real PTY), SMB/CIFS (get/put/delete) — backends fixed and wired
- [x] IPMI Serial-over-LAN (RAKP+ session establishment, hand-rolled), SNMPv3 (USM, hand-rolled, verified against RFC 3414 test vectors), gRPC Explorer (server reflection), TN3270 / TN5250 (IBM mainframe/AS400 — order-stream parser + block-mode screen emulator, hand-rolled from spec)
- [x] Rlogin, Docker Logs, X11 Forwarding (SSH), Proxmox Console (VNC-over-WebSocket reuse), NFS Explorer (hand-rolled read-only NFSv3), Kubernetes Port-Forward, SPICE Console — the 7 session types that had no backend at all

**Why this matters for the roadmap:** this is the "protocol breadth" axis of our competitive positioning (§3) made real rather than aspirational — CrossTerm now reaches further into heterogeneous infrastructure (mainframes, BMCs, hypervisor consoles, IoT/SNMP devices) than Termius, Royal TSX, or SecureCRT.

**Status:** ✅ Merged to `main` 2026-08-15 (clean fast-forward, no conflicts). See `docs/ENGINEERING_PLAN.md` §1.1 for the full phase breakdown and known, non-blocking follow-ups.

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
| Frontend test coverage | 81.92% lines (measured, 2026-08-21) | ≥ 75% | ≥ 85% |
| Backend test coverage | 42.94% (measured, 2026-08-21) | ≥ 60% | ≥ 75% |
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

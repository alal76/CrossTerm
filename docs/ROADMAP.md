# CrossTerm Product Roadmap
### Becoming the Premium Network & Connectivity Platform

**Document owner:** Product  
**Last updated:** 2026-05-04  
**Current version:** 0.2.5  
**Horizon:** 18 months (v0.3 → v1.2)

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

Compared to **Termius**, **Royal TSX**, and **SecureCRT**, CrossTerm's current gaps fall into four categories:

1. **Stability & error recovery** — backend modules average 2,400–2,700 lines with no backend unit test coverage. Silent failures reach users as blank screens.
2. **Onboarding friction** — no import from PuTTY / `.ssh/config` / SecureCRT / MobaXterm. New users must re-enter every host manually.
3. **Session management at scale** — no bulk operations, no smart groups, no saved search, no health monitoring for always-on tunnels.
4. **Team & enterprise readiness** — vault sharing is modelled in types but not wired. No SSO, no policy management, no compliance export.

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
| # | Issue | Heuristic violated | Impacted flow |
|---|-------|--------------------|---------------|
| U-1 | No import from PuTTY / `.ssh/config` / SecureCRT | Recognition over recall | First-run |
| U-2 | Vault unlock is the first thing new users see — no explainer | Help & docs | First-run |
| U-3 | SSH connection failure messages are raw Rust error strings | Error prevention | Connect |
| U-4 | Session editor opens in a modal with 20+ fields — no progressive disclosure | Aesthetic & minimalist | Connect |
| U-5 | No visual indicator when a background tunnel silently drops | Visibility of system status | Always-on tunnels |

### High (reduce retention)
| # | Issue | Heuristic violated | Impacted flow |
|---|-------|--------------------|---------------|
| U-6 | Scrollback search requires Ctrl+Shift+F — not discoverable | Recognition over recall | Terminal |
| U-7 | Multiple locked vaults: "Delete" icon is easy to trigger by accident (proximity to Select) | Error prevention | Vault |
| U-8 | No bulk session actions (select 10, connect all / delete all) | Efficiency of use | Session tree |
| U-9 | Theme changes require restart to fully apply in terminal renderer | Consistency | Settings |
| U-10 | Android soft keyboard overlaps terminal on small phones | Flexibility | Android |

### Medium (limit power use)
| # | Issue | Impact |
|---|-------|--------|
| U-11 | Macro editor has no test/dry-run mode | Automation |
| U-12 | Port forward rules show no live traffic metrics | Network |
| U-13 | No "recently used" section at the top of session tree | Navigation |
| U-14 | SFTP drag-and-drop only works one direction (local → remote) | File transfer |
| U-15 | No right-click → "Open in SFTP" from a terminal tab | File transfer |

---

## 6. Roadmap Phases

### Phase 1 — Foundation (v0.3 → v0.4) · Q2–Q3 2026 · 3 months

**Theme: Trustworthy core**

The goal is to make what exists reliable enough that users recommend it. No new protocols. Every engineering-hour goes to stability, test coverage, and the two highest-friction onboarding gaps.

#### Stability & quality
- [ ] Backend unit test coverage ≥ 60% (currently ~0%) on SSH, vault, config, network modules
- [ ] Frontend test coverage ≥ 75% (currently ~45%)
- [ ] Structured error taxonomy: all Tauri invoke errors return typed `AppError { code, message, detail }` — no raw strings to the UI
- [ ] Crash reporter: automatic Sentry capture with symbolicated Rust backtraces (opt-in telemetry)
- [ ] Session watchdog: detect silent tunnel drops and surface a toast + reconnect option within 5 seconds
- [ ] Memory profiling pass: fix top-3 allocations in SSH scrollback and SFTP transfer queue
- [ ] Startup time ≤ 1.5 s on a mid-range machine (measure cold + warm)

#### Onboarding
- [ ] **Import wizard** (U-1): parse `~/.ssh/config`, PuTTY registry/sessions, SecureCRT `.ini`, MobaXterm `.mxtsessions` — create sessions in one click
- [ ] **First-run experience redesign** (U-2): replace the raw vault-unlock gate with a 3-step welcome flow: (1) import existing sessions, (2) create vault, (3) optional theme/font
- [ ] Friendly error messages (U-3): map the 20 most common SSH errors to actionable copy ("Wrong password — check Caps Lock", "Port 22 is blocked — try 443")

#### Usability quick wins
- [ ] Add 200ms delete-confirm guard on vault trash icon (U-7)
- [ ] "Recently connected" section pinned to top of session tree (U-13)
- [ ] Ctrl+Shift+F search bar surfaces automatically on any text selection in terminal (U-6)
- [ ] Right-click terminal tab → "Open SFTP here" (U-15)

**Exit criteria for Phase 1:** 0 P0 crashes in a 2-week soak run on macOS + Windows + Ubuntu. Onboarding test (unfamiliar user, 3 SSH hosts connected in < 5 minutes) succeeds without documentation.

---

### Phase 2 — Power User (v0.5 → v0.6) · Q3–Q4 2026 · 3 months

**Theme: The tool that replaces every other tool**

Individual power users spend 8+ hours a day in their terminal client. This phase makes CrossTerm indispensable by beating competitors on session management, automation, and security depth.

#### Session management
- [ ] **Session tree v2**: drag-and-drop reorder, multi-select with Shift+click / Ctrl+click, bulk connect / disconnect / delete / tag
- [ ] **Smart groups**: saved filter by tag, protocol, last-connected date, connection status — auto-populated
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
- [ ] **TOTP / MFA vault unlock**: wire `TOTPSeedCredential` type to the vault unlock flow — add a time-based OTP field that must match before password is accepted
- [ ] **YubiKey / FIDO2 vault unlock**: complete the Phase 3 stub — real CTAP2 challenge-response via the `vault_fido2_auth_begin` command
- [ ] **Certificate pinning UI**: per-host TLS/SSH fingerprint review, pin/unpin, expiry alerts
- [ ] **Audit log export**: CSV + syslog forwarding (TCP/UDP) + signed PDF (for compliance conversations)
- [ ] SSH known-hosts diff viewer: visualise what changed when a host key mismatch occurs

#### Terminal quality
- [ ] Clickable hyperlinks in terminal output (URLs, file paths, IP addresses)
- [ ] **Jump to timestamp** in scrollback: click any timestamp prefix → jump to that position
- [ ] Regex search in terminal scrollback with match highlights and prev/next navigation
- [ ] Right-to-left text support (Arabic, Hebrew) — required for the Middle East enterprise market

**Exit criteria for Phase 2:** NPS survey of 50 beta users scores ≥ 45. At least 5 users report CrossTerm replaced their previous primary tool.

---

### Phase 3 — Team & Enterprise (v0.7 → v1.0) · Q1 2027 · 4 months

**Theme: The tool IT will approve**

Enterprise deals require compliance, centralized control, and SSO. This phase is the unlock for $30+/seat pricing.

#### Team collaboration
- [ ] **Shared vault**: complete the `shared_with` field wire-up — vault owner grants read-only or read-write access to named profile IDs; encrypted re-key per shared user
- [ ] **Team session library**: shared read-only session tree visible to all team members; owners control edits
- [ ] **Presence indicators**: see which team members are currently connected to a given host (useful for ops war rooms)
- [ ] **Session handoff**: hand off a live terminal session to another user with permission prompt on both sides

#### Enterprise identity
- [ ] **SAML 2.0 / OIDC SSO**: federate vault unlock and profile identity to Okta, Azure AD, Ping — replaces master password for enterprise deployments
- [ ] **LDAP/AD group sync**: map AD groups to CrossTerm session library access levels
- [ ] **MDM deployment**: silent install + policy JSON pushed via SCCM/Intune/Jamf; disables features by policy (e.g. block local vault, enforce SSO)

#### Compliance & governance
- [ ] **Centralized audit trail**: ship audit events to a customer-managed endpoint (syslog, Splunk HEC, Datadog, S3)
- [ ] **Session recording policy**: IT can mandate recording for all sessions to compliance hosts; recordings stored encrypted, accessible to authorized reviewers
- [ ] **Compliance report generator**: one-click PDF covering vault access, session counts, failed auth attempts, key rotation events — formatted for SOC 2 / ISO 27001 auditors
- [ ] **Role-based access control (RBAC)**: admin / operator / viewer roles; admins manage vault and sessions, viewers read-only

#### Cloud integration depth
- [ ] **AWS SSM Session Manager**: connect to EC2 instances without opening port 22 — entirely through SSM agent
- [ ] **Azure Bastion**: connect to Azure VMs through the Bastion service (no public IP required)
- [ ] **GCP IAP TCP tunneling**: SSH to GCP Compute instances through Identity-Aware Proxy
- [ ] **Cloud cost alerts**: surface Cost Explorer / Azure Cost Management anomalies as CrossTerm notifications

**Exit criteria for Phase 3:** First enterprise customer (≥ 50 seats) signed and onboarded. SOC 2 Type I report initiated.

---

### Phase 4 — Intelligence (v1.1) · Q2 2027 · 2 months

**Theme: The tool that thinks with you**

AI assistance is a table-stakes differentiator by 2027. Done right, it materially reduces time-to-resolution for operational tasks.

- [ ] **AI command assistant** (local LLM, privacy-first): highlight an error message → "Explain and fix" → suggests corrective command, user approves before execution
- [ ] **Smart autocomplete**: command suggestions based on session history, session type (e.g. `kubectl` completions from the running cluster's API)
- [ ] **Session anomaly detection**: ML model detects unusual output patterns (spike in error rate, unexpected login, new sudo invocations) and raises an alert
- [ ] **Script generation**: natural language → shell script / macro steps, inserted into macro editor for review
- [ ] **Connection optimiser**: suggest SSH keepalive / compression settings based on observed packet loss and latency

**Privacy guarantee:** All AI inference runs locally (Ollama / llama.cpp integration) by default. Cloud inference is opt-in and never sends raw terminal output.

---

### Phase 5 — Mobile & Ecosystem (v1.2) · Q3 2027 · 2 months

**Theme: Everywhere**

- [ ] **iOS app**: native SwiftUI shell app with a Rust SSH/SFTP core via `ssh2-rs`; syncs vault and sessions from macOS via iCloud Keychain (no server required)
- [ ] **Android polish**: complete the existing Android component set — fix soft-keyboard overlap (U-10), add Bluetooth keyboard support, tablet split-pane
- [ ] **Web thin client**: browser-accessible terminal (WebSocket → Tauri relay) for jump-server scenarios where desktop install is not possible
- [ ] **VS Code extension**: open a CrossTerm SSH session from a VS Code remote project in one click
- [ ] **Raycast / Alfred plugin**: ⌘-space → type a hostname → open CrossTerm session

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

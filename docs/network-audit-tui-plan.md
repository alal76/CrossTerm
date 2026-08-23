# network-audit-tui-plan

Design and roadmap for `crossterm-audit-tui`, a standalone terminal UI for
auditing network reach from a container or headless server — for example, a
Proxmox LXC/VM with no display, used to check what's reachable on a subnet
and then open a session into whatever's found. It's a separate binary, not
a mode of the GUI app, and not a Tauri/webview process at all.

Source: `crossterm-audit-tui/`. Built as a sibling workspace member to
`src-tauri` (see the root `Cargo.toml`), specifically so it has none of the
GUI app's Tauri/GTK/WebKit dependency tree — its own `Cargo.toml` pulls in
only `ratatui`, `crossterm`, `serde`/`serde_json`, and `clap`.

## Why this exists, and why it's separate from the GUI

The GUI's Network Explorer and `network-explore-cli` (the existing headless
scan tool — see `docs/network-explore-cli.md`) already cover running a scan
without a display. What was missing was a way to *browse* those results and
*act on them* (open a session) without a display either — `network-explore-cli`
only writes JSON, it has no interactive UI. `crossterm-audit-tui` is that
missing piece: an MC/nano-style full-screen UI (persistent function-key
legend, modal popups, no free-form command line) that's comfortable to use
over a bare SSH connection into a box with nothing else installed.

It's a genuinely separate UI, not an embedding of the GUI's own components,
because the GUI is a Tauri/webview app — its UI layer fundamentally isn't
something you can run in a terminal. Sharing *state* with the GUI (see
Phase 4 below) is a separate concern from sharing UI code, and is planned;
sharing UI code is not, since there is no shared UI layer to begin with.

## Roadmap

- **Phase 1 — static host browser (done).** Reads a JSON dump produced by
  `network-explore-cli` (passed as a CLI argument) and lets you page through
  discovered hosts: IP, hostname, OS guess, suggested session type, open
  ports, with a detail popup for the full record (TLS SANs, mDNS records,
  evidence). No scanning of its own yet.
- **Phase 2 — live scanning (done).** Press `n` (or `F2`) from the browser
  to type a CIDR and scan it directly, instead of having to run
  `network-explore-cli` separately first. Implementation: shells out to the
  `network-explore-cli` binary (found beside this one, or on `PATH`) in a
  background thread, so the UI stays responsive while the scan runs; the
  browser refreshes automatically when it finishes. See `src/scan.rs` and
  the `Screen::ScanPrompt` / `Screen::Scanning` / `Screen::ScanError` states
  in `src/app.rs`. Deliberately shells out rather than linking `app_lib` (the
  GUI app's own crate) directly — that would pull in the whole Tauri/GTK/
  WebKit dependency tree at compile time regardless of which code paths
  actually run, defeating the point of a small, standalone tool.
- **Phase 3 — Telnet sessions.** Open an interactive Telnet session into a
  selected host directly from the Host Detail view, in a small embedded
  terminal (not a full PTY passthrough yet — that's Phase 4/5).
- **Phase 4 — SSH sessions.** Same, for SSH. Per an explicit design decision
  (see below), this shells out to the real system `ssh` binary when one is
  present, rather than always using CrossTerm's built-in `russh` client —
  so `~/.ssh/config`, agent forwarding, and any local SSH setup the user
  already has just work, the same way they would running `ssh` directly.
  Falls back to the built-in client only if no system `ssh` is found.
- **Phase 5 — SSH parity features.** Host-key TOFU (trust-on-first-use)
  verification with a real interactive Yes/No prompt (not the GUI's
  automatic-accept behavior — a bare terminal tool auditing unfamiliar
  networks should not silently trust new host keys), and sharing known-hosts
  and vault state with the GUI app rather than keeping fully separate state.
  This needs a real answer for *how* the two share that state (the GUI's
  vault/known-hosts handling is coupled to `crate::config`/`crate::audit`
  inside `app_lib`, which this tool deliberately doesn't depend on) —
  likely a shared on-disk format/location the GUI already uses, read
  directly rather than through `app_lib`'s API surface. Not yet designed in
  detail.
- **Phase 6 — polish.** Static `musl` build target (for dropping a single
  self-contained binary into a minimal container with no matching glibc),
  plus whatever rough edges the earlier phases surface in real use.

## Design decisions already made

Answered explicitly when this was scoped, rather than left as open
questions for whoever picks up later phases:

1. **Shell out to the real system SSH/Telnet client when present**, rather
   than always using the built-in `russh` client. Falls back to the
   built-in client if no system binary is found.
2. **Share known-hosts/vault state with the GUI app**, rather than being a
   fully self-contained tool with its own separate trust store. Mechanism
   not yet designed (see Phase 5).
3. **Real interactive Yes/No host-key TOFU prompt**, not the GUI's
   automatic-accept behavior.
4. **Static build target: `musl`**, for portability into minimal containers.

## Relationship to `network-explore-cli`

`network-explore-cli` and `crossterm-audit-tui` are companion tools, built
from the same workspace and typically deployed together: the former does
the scanning (and can be used entirely on its own, scripted, piped to `jq`,
etc. — see its own doc), the latter is the interactive browser/session
launcher on top of its output. Phase 2 makes that pairing automatic — you
no longer have to run them as two separate steps — but `network-explore-cli`
remains a fully independent, scriptable tool in its own right.

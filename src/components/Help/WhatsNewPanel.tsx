import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, Sparkles } from "lucide-react";

const APP_VERSION = "2.0.4";
const DISMISSED_VERSION_KEY = "crossterm:whats-new-dismissed";

const RELEASE_NOTES = `
## What's New in CrossTerm ${APP_VERSION}

### Bug Fixes
- Fixed pressing Enter in the CIDR field while a scan was already running launching a second, fully overlapping scan: the Scan button correctly disabled itself mid-scan, but the CIDR field's Enter-key shortcut had no such check. Two overlapping scans of the same subnet split the shared probe pool from 2.0.3 between them, so results could look dramatically worse than a single scan of the exact same subnet moments later — this was mistakable for a detection bug when it was really just two scans quietly fighting each other. Pressing Enter (or clicking Scan) while one is already in progress is now a no-op until it finishes.

## CrossTerm 2.0.3

### Bug Fixes
- Fixed scan results getting worse the more subnets you searched at once: each concurrently-running scan created its own independent pool of up to 25 simultaneous probes, so scanning several subnets together (e.g. a local LAN plus a couple of Tailscale-routed ones) could mean 100+ probes competing for the system at the same time — degrading every scan's results, including ones that would otherwise have found plenty of hosts. All scans now share one bounded pool, so scanning more subnets at once no longer starves the others.

## CrossTerm 2.0.2

### Bug Fixes
- Fixed Network Explorer missing real, online devices on the local subnet: detection relied only on open TCP ports and ICMP ping, so any device with a firewall blocking ping and none of the scanned ports open was invisible — even though it was clearly present and reachable (confirmed against a live ARP table showing resolved hardware addresses for several devices the scan never once reported). The scan now also checks the ARP cache, which can't be blocked without breaking basic IP connectivity, as a third presence signal.

## CrossTerm 2.0.1

### Bug Fixes
- Fixed the real reason settings (including the new diagnostic logging toggle) could silently fail to save on some installs: the app's cached "active profile" ID defaults to a placeholder that was never a real profile, and only ever got replaced by the first-launch setup wizard. If that wizard was skipped, or the profile it pointed to was later removed, every settings and session read/write failed silently from then on — no error, just nothing being saved. The app now recovers automatically by switching to the most recently used real profile instead of getting stuck permanently.

Versioning changed here: bug-fix releases now bump the patch number (2.0.x) instead of the minor number. The entries below predate that change and use the old scheme — they're all still bug fixes/features from the same 2.0 line, just numbered inconsistently with what's above.

## CrossTerm 2.5.0

### Bug Fixes
- Settings that fail to save now show an error instead of silently keeping the on-screen value with nothing actually persisted — a toggle could look "on" while the backend write had failed for an unrelated reason (e.g. a stale active profile), with no indication anything was wrong.
- The active profile's diagnostic-logging preference is now also applied on every settings read, not just on profile switch/settings save — closes a startup-timing gap where a saved "on" preference could fail to take effect on a fresh launch.

## CrossTerm 2.4.0

### Bug Fixes
- Fixed the diagnostic logging toggle added in 2.3.0 not actually doing anything: the logging plugin bakes its on/off level into its internal dispatcher once, at startup, so flipping the setting afterward silently had no effect. It now uses a live-checked switch instead, so enabling or disabling it takes effect immediately, as originally intended.

## CrossTerm 2.3.0

### New Features
- **Optional diagnostic logging** — Settings > Advanced now has a toggle to write scan and activity logs to disk, off by default in every build (release included) and takes effect immediately, no restart needed. Makes it possible to actually diagnose a report like "the scan only found N hosts" after the fact.

### Bug Fixes
- Fixed Network Explorer scanning and reporting a subnet's network (.0) and broadcast (.255) addresses as if they were real, connectable devices — neither is an actual host, and the broadcast address in particular tended to answer a ping, showing up as a bogus "found" entry with no real device behind it.

## CrossTerm 2.2.0

### New Features
- **Custom Network Explorer scan settings** — the "extra ports" field now remembers what you type between scans, and two new fields let you add SNMP community strings (for switches/UPSes/printers configured off the default "public") and vendor keywords (for camera/NAS/router brands not in the built-in device-guessing list) that scans will also try.
- **Free-form serial baud rate** — the baud rate field now accepts any value you type (e.g. 230400, 921600 for ESP32/ESP8266 flashing or 3D-printer firmware), not just the fixed preset list; the presets are still one click away.

### Bug Fixes
- Fixed session types added since 1.5.1 (Proxmox Console, Redfish, SMB, WebDAV, SNMP, IPMI, Rlogin, NFS, and about a dozen others) failing to save with an error — the backend was only aware of the original 13 session types and silently rejected the rest.
- Fixed Network Explorer under-reporting hosts on larger scans (e.g. showing 4 out of ~19 real devices): a race between the scan-progress counter and each host's own "found" event could retire the scan before its last few in-flight hosts' results arrived, silently dropping them.

## CrossTerm 2.1.0

### Bug Fixes
- Fixed the real reason "More Session Types" never showed extra protocols for some users: the native macOS/Windows/Linux menu bar's own "Connect" menu is a separate surface from the in-window "+" button and never got a "More Session Types" entry at all — it now has the same submenu the in-window menu does.
- Fixed a Network Explorer scan slowdown introduced by the new SNMP/IPMI UDP probes: since most hosts don't run either service and UDP never sends back a "closed" response the way TCP does, those two probes were reusing the same ~1.5s timeout as the TCP/ping checks, adding that floor to every single host's scan time regardless of how fast it would otherwise have finished. They now use a much shorter, dedicated timeout.

## CrossTerm 2.0.0

### New Features
- **Faster session-type picker** — the "+" new-tab menu's "More Session Types…" now expands inline into a submenu listing every remaining protocol, instead of opening a full dialog and hunting through a dropdown.
- **Real UDP scanning in Network Explorer** — SNMP and IPMI are now detected with genuine protocol probes (an SNMP GetRequest and an IPMI RMCP Presence Ping), not just a port guess.
- **Redfish and WebDAV detection** — identified by their actual HTTP signature (Redfish's ServiceRoot JSON, WebDAV's DAV/PROPFIND header), not assumed from a shared generic web port.
- **Broader scan coverage** — SMB, FTP, NFS, Rlogin, and Proxmox hosts are now suggested as one-click connections when discovered.

### Bug Fixes
- Fixed a port-mismatch bug affecting Kubernetes, Docker, WinRM, and MQTT-over-TLS discoveries: the suggested connection type was right, but a naming inconsistency meant it silently connected on the wrong port (a hardcoded fallback) instead of the real one the scan found.
- SMB was being detected during a scan but never offered as a connectable session — fixed.

## CrossTerm 1.5.1

### New Features
- **Macro Editor & Expect Rules** — Automate terminal workflows with recordable macros and pattern-triggered auto-responses.
- **Session Recording** — Record and play back terminal sessions, with policy-driven auto-recording and a compliance banner.
- **Cloud Dashboard** — Manage AWS, Azure, and GCP resources and Kubernetes clusters from one place.
- **Plugin Manager & Registry** — Install, enable, and browse plugins for CrossTerm.
- **Smart Groups** — Build dynamic session groups filtered by tag, protocol, status, or host.
- **Single Sign-On** — Configure OIDC providers for SSO authentication.
- **FTP support** — Connect to FTP servers alongside SFTP.
- **Standalone Code Editor & Diff Viewer** — Edit and compare local files without leaving the app.
- **Network Explorer** — Quick Scan and Wake-on-LAN, alongside existing discovery and Wi-Fi tools.

### Improvements
- Profile Sync now encrypts with AES-256-GCM and actually transfers your sessions, snippets, and settings.
- Session health indicators reflect real connection activity, so reconnect prompts trigger when a session actually drops.
- SFTP gained file preview and folder sync.
- A host key change now shows the real old/new fingerprint diff instead of a generic error.

### Bug Fixes
- Session rename, move, and delete no longer fail silently — errors surface and the change is rolled back.
- The vault no longer offers a biometric unlock option that could never succeed.
- Export Audit Log and locale installation now actually save instead of silently doing nothing.
`;

export default function WhatsNewPanel() {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
    if (dismissed !== APP_VERSION) {
      setVisible(true);
    }
  }, []);

  function handleDismiss() {
    setVisible(false);
  }

  function handleDontShowAgain() {
    localStorage.setItem(DISMISSED_VERSION_KEY, APP_VERSION);
    setVisible(false);
  }

  if (!visible) return null;

  return (
    <div className="fixed inset-0 z-[8500] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={handleDismiss}
        onKeyDown={(e) => e.key === "Escape" && handleDismiss()}
        aria-hidden="true"
      />
      <div
        className="relative w-full max-w-lg max-h-[80vh] bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] flex flex-col overflow-hidden"
        style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-border-subtle shrink-0">
          <div className="flex items-center gap-2">
            <Sparkles size={16} className="text-accent-primary" />
            <h2 className="text-sm font-semibold text-text-primary">{t("whatsNew.title")}</h2>
          </div>
          <button
            onClick={handleDismiss}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 text-xs text-text-secondary leading-relaxed">
          {RELEASE_NOTES.trim()
            .split("\n")
            .map((line, i) => {
              const key = `line-${String(i)}`;
              if (line.startsWith("## ")) {
                return (
                  <h3 key={key} className="text-sm font-semibold text-text-primary mt-3 mb-2">
                    {line.replace("## ", "")}
                  </h3>
                );
              }
              if (line.startsWith("### ")) {
                return (
                  <h4 key={key} className="text-xs font-medium text-text-primary mt-3 mb-1.5">
                    {line.replace("### ", "")}
                  </h4>
                );
              }
              if (line.startsWith("- ")) {
                const content = line.replace("- ", "");
                const boldMatch = /^\*\*(.+?)\*\*\s*—\s*(.+)$/.exec(content);
                if (boldMatch) {
                  return (
                    <p key={key} className="ml-3 mb-1">
                      <span className="text-text-primary font-medium">{boldMatch[1]}</span>
                      {" — "}
                      {boldMatch[2]}
                    </p>
                  );
                }
                return (
                  <p key={key} className="ml-3 mb-1">
                    • {content}
                  </p>
                );
              }
              if (line.trim() === "") return <div key={key} className="h-1" />;
              return (
                <p key={key} className="mb-1">
                  {line}
                </p>
              );
            })}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border-subtle shrink-0">
          <button
            onClick={handleDontShowAgain}
            className="px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("whatsNew.dontShowAgain")}
          </button>
          <button
            onClick={handleDismiss}
            className="px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("whatsNew.dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}

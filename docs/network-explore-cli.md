# network-explore-cli

A standalone command-line binary that runs CrossTerm's network discovery
pipeline — the same one behind the GUI's Network Explorer — with no Tauri
app, no vault, no UI, and no login. It scans a CIDR range and writes a full
JSON result set to disk.

It exists for headless/scripted use: auditing a server subnet from a
terminal (including over SSH into a jump host with no display), feeding scan
results into other tooling, or CI-style inventory checks — anywhere running
the full GUI app isn't practical or wanted.

Source: `src-tauri/src/bin/network-explore-cli.rs`. The scan logic itself
lives in `src-tauri/src/network/mod.rs`'s `run_explore_and_dump` — see that
function's own doc comment for how it relates to the GUI's
`network_explore_start`.

## Building and running

From the repository root:

```bash
cd src-tauri
cargo build --release --bin network-explore-cli
# binary at target/release/network-explore-cli

# or run directly without a separate build step:
cargo run --release --bin network-explore-cli -- <CIDR> [OPTIONS]
```

A debug build (`cargo run --bin network-explore-cli --`, no `--release`)
works fine for occasional use; `--release` matters once you're scanning
larger ranges, since the per-host probing is CPU-bound enough that a debug
build's lack of optimization is noticeable across hundreds of hosts.

## Usage

```text
network-explore-cli [OPTIONS] <CIDR>
```

Run `network-explore-cli --help` at any time for the authoritative, in-sync
flag reference — everything below is a walkthrough of the same flags with
more context.

### Arguments

| Argument | Description |
|---|---|
| `<CIDR>` | Required. The range to scan, e.g. `192.168.1.0/24`, `10.0.0.0/16`. Every host address in the range is probed. |

### Options

| Flag | Default | Description |
|---|---|---|
| `-o, --out <PATH>` | `network_scan.json` | Output JSON file path. |
| `-t, --timeout-ms <MS>` | `1500` | Per-host TCP connect / ping timeout, in milliseconds. See [Timeouts](#timeouts) below for how this interacts with the shorter UDP timeout. |
| `--services <LIST>` | the app's standard set (see below) | Comma-separated well-known services to scan, e.g. `--services ssh,winrm,ipmi`. Run `--list-services` to see every valid name and its default port. |
| `--extra-ports <LIST>` | none | Comma-separated arbitrary TCP ports to scan in addition to `--services`, e.g. `--extra-ports 8006,9100`. |
| `--snmp-community <LIST>` | none (`public` is always tried) | Extra SNMP v1/v2c community strings to try against each host's UDP/161. |
| `--vendor-hint <LIST>` | none | Extra vendor keywords (e.g. `HIKVISION`, `MIKROTIK`) to match against a discovered TLS certificate's subject/issuer organization, beyond the app's built-in vendor list. |
| `--list-services` | — | Print every valid `--services` name with its default port, then exit without scanning. |
| `--table` | off | Also print a human-readable summary table to stdout after scanning. The JSON file is always written regardless of this flag. |
| `-q, --quiet` | off | Suppress the `scanning...`/`wrote N host(s)...` status lines normally printed to stderr. Doesn't affect `--table`'s output or the JSON file. |
| `-h, --help` | — | Print the flag reference and exit. |

### Default service set

When `--services` is omitted, the same default set the GUI uses is scanned:
`ssh`, `vnc`, `rdp`, `http`, `https`, `telnet`, `ftp`, `smb`, `mysql`,
`postgresql`, `redis`, `mongodb`, plus the protocol-breadth additions
(`win_rm`, `win_rm_tls`, `mqtt`, `mqtt_tls`, `netconf`, `grpc`, `kube_api`,
`docker_api`, `docker_api_tls`, `ws_terminal`, `rtsp`).

Three ports are **always** probed regardless of `--services`/`--extra-ports`:

- **53** (DNS) — feeds local-DNS-server discovery, used internally as one of
  several hostname-resolution methods.
- **8096** (Jellyfin) and **32400** (Plex) — cheap to always check, the only
  way these media-server VMs get named in results.

SNMP (UDP/161) and IPMI (UDP/623) are **always** probed too — they're not
gated by `--services` at all, since they're UDP protocol probes rather than
simple TCP connect checks. Use `--snmp-community` to reach devices configured
off the default `public` community.

### Timeouts

`--timeout-ms` governs TCP connect attempts and ICMP ping. UDP probes (SNMP,
IPMI) use their own shorter timeout — `min(300ms, --timeout-ms)` — internally,
regardless of what you pass. This is deliberate: a TCP port that isn't
listening fails fast with a real RST packet, but a UDP port with no SNMP/IPMI
agent behind it never replies at all, so it always burns its *full* timeout.
Reusing a generous TCP timeout for UDP would make every host without SNMP/IPMI
take that much longer to scan, for no benefit — a real reply, when a device
does answer, arrives in low milliseconds on a LAN.

## Output schema

The JSON file's top-level shape is `ExploreDump`:

```jsonc
{
  "cidr": "192.168.1.0/24",
  "bound_interface": "en0",        // network interface probe traffic was pinned to, if resolved
  "dns_servers_used": ["192.168.1.1"], // local DNS servers discovered/used for hostname resolution
  "ports_scanned": [22, 53, 80, ...],  // full deduplicated port list actually probed
  "host_count": 3,                 // length of "results"
  "results": [ /* ExploreResult, one per host found present — see below */ ],
  "unmerged_mdns": {                // mDNS records whose address never answered a scanned port or ping
    "192.168.1.50": [ /* MdnsRecord */ ]
  }
}
```

Each entry in `results` is an `ExploreResult`:

| Field | Type | Meaning |
|---|---|---|
| `ip` | string | The host's IPv4 address. |
| `hostname` | string \| null | Best-effort resolved hostname (reverse DNS, mDNS, NetBIOS, ARP-vendor fallback, etc. — see `resolve_hostname_aggressive` in `network/mod.rs` for the full method chain). |
| `mac_address` | string \| null | MAC address from the local ARP cache, if resolved. |
| `mac_vendor` | string \| null | OUI vendor lookup against `mac_address` (e.g. `"Apple, Inc."`). |
| `open_ports` | `OpenPort[]` | Every port that answered, enriched with banner/version/TLS detail — see below. Also includes synthetic entries for SNMP (port 161) and IPMI (port 623) when those UDP probes get a real reply. |
| `os_guess` | string \| null | Best-effort OS/platform guess from open ports, ICMP TTL, and vendor hints. |
| `response_time_ms` | number | Currently always `0.0` in this CLI's output (populated by the GUI path but not timed here — see the field's use in `network_explore_start` if you need real timing). |
| `suggested_session_type` | string \| null | The single highest-priority connectable session type CrossTerm would suggest (e.g. `"ssh"`, `"vnc"`). |
| `candidate_session_types` | string[] | Every connectable session type detected, in priority order — for hosts running more than one service (e.g. both SSH and a Proxmox API). Always starts with `suggested_session_type` when that's non-null. |
| `ttl` | number \| null | Raw ICMP TTL from the ping reply — a fallback OS-family signal (`64` typically Linux/macOS/BSD, `128` typically Windows, `255` often network gear). |
| `mdns` | `MdnsRecord[]` | mDNS/Bonjour service records observed for this IP. |
| `evidence` | string[] | Human-readable notes explaining the `hostname`/`os_guess`/`mac_vendor` findings — useful for understanding *why* a guess was made, not just what it was. |

`OpenPort`:

| Field | Type | Meaning |
|---|---|---|
| `port` | number | The port number. |
| `service_name` | string | Short identifier, e.g. `"ssh"`, `"http"`, `"snmp"`, `"ipmi"`. |
| `protocol` | string | `"tcp"` or `"udp"`. |
| `banner` | string \| null | Raw first-line banner, for server-speaks-first protocols (SSH, FTP, SMTP, POP3, IMAP, MySQL, VNC). |
| `version` | string \| null | Parsed product+version summary, e.g. `"OpenSSH 10.0p2 (Debian)"`. For the synthetic SNMP entry, this holds the raw `sysDescr` string instead. |
| `http_title` | string \| null | `<title>` extracted from an HTTP(S)-shaped response. |
| `tls` | object \| null | TLS certificate detail (subject, issuer, validity, SANs), present for any port where a TLS handshake succeeded. |

`MdnsRecord`:

| Field | Type | Meaning |
|---|---|---|
| `service_type` | string | e.g. `"_home-assistant._tcp.local."` |
| `instance_name` | string | e.g. `"Home"` |
| `hostname` | string \| null | |
| `txt` | object | Raw TXT record key/value pairs. |

## Examples

Scan a home/office LAN with the default service set:

```bash
network-explore-cli 192.168.1.0/24
```

Audit a server subnet: SSH, WinRM (Windows remote management), and IPMI/BMC
out-of-band management, plus a couple of ports commonly used by
hypervisors/monitoring agents, printing a quick table as well as writing JSON:

```bash
network-explore-cli 10.0.0.0/24 \
  --services ssh,win_rm,win_rm_tls \
  --extra-ports 8006,9100 \
  --table
```

(IPMI and SNMP are always probed regardless of `--services` — see
[Default service set](#default-service-set) — so they don't need to be
listed explicitly.)

Reach SNMP-managed gear (switches, UPSes, printers) that isn't on the
default `public` community, and tag a niche camera vendor's TLS certs:

```bash
network-explore-cli 10.0.0.0/24 \
  --snmp-community private --snmp-community monitoring \
  --vendor-hint HIKVISION
```

Quiet, scriptable JSON-only output (no stderr status lines) for piping into
other tooling:

```bash
network-explore-cli 192.168.1.0/24 --quiet --out /tmp/scan.json
jq '.results[] | select(.suggested_session_type == "ssh") | .ip' /tmp/scan.json
```

## Relationship to the GUI's Network Explorer

This tool and the GUI's Network Explorer (`network_explore_start`, the Tauri
command backing it) share the same underlying probing primitives — TCP
connect, ping, ARP, mDNS, SNMP, IPMI, OS/vendor guessing — in
`network/mod.rs`, and are kept in sync on a best-effort basis. They differ in
shape, not capability:

- The GUI streams incremental progress and per-host result events for a
  responsive UI, and supports cancelling a scan mid-flight.
- This CLI runs one batch pass and writes one JSON file at the end — no
  progress events, no cancellation (use Ctrl-C).

If you notice a probe or capability the GUI has that this CLI doesn't (or
vice versa), that's a real gap, not an intentional difference — please file
an issue.

## Security & ethics

This uses the same class of traffic as an ordinary client: TCP connection
attempts, ICMP probes, and standard-protocol handshakes (HTTP GET, TLS
handshake, SNMP GetRequest, IPMI RMCP presence ping). It never sends exploit
payloads. Only scan networks you own or have explicit written permission to
scan — unauthorized port scanning may be illegal in your jurisdiction and
violates the terms of service of most cloud providers.

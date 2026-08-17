/// Analyze WiFi details for a given network using helper functions.
#[allow(dead_code)]
#[tauri::command]
pub fn network_analyze_wifi_details(
    ssid: String,
    bssid: String,
    channel_raw: String,
    signal_noise_raw: Option<String>,
    security_raw: Option<String>,
) -> serde_json::Value {
    let (channel, channel_width, freq_hint) = parse_channel_info(&channel_raw);
    let band = parse_band_from_channel(channel, freq_hint);
    let (signal_dbm, noise_dbm) = signal_noise_raw
        .as_deref()
        .map(parse_signal_noise)
        .unwrap_or((None, None));
    let security = security_raw
        .as_deref()
        .map(parse_macos_security)
        .unwrap_or(WifiSecurity::Unknown("unknown".to_string()));

    serde_json::json!({
        "ssid": ssid,
        "bssid": bssid,
        "channel": channel,
        "channel_width_mhz": channel_width,
        "band": format!("{:?}", band),
        "signal_dbm": signal_dbm,
        "noise_dbm": noise_dbm,
        "security": format!("{:?}", security),
    })
}
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Semaphore;
use uuid::Uuid;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Scan not found: {0}")]
    ScanNotFound(String),
    #[error("Invalid CIDR: {0}")]
    InvalidCidr(String),
    #[error("Tunnel not found: {0}")]
    TunnelNotFound(String),
    #[error("Server not found: {0}")]
    ServerNotFound(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid MAC address: {0}")]
    InvalidMac(String),
    #[error("Server already running: {0}")]
    ServerAlreadyRunning(String),
    #[error("Port in use: {0}")]
    PortInUse(u16),
}

impl Serialize for NetworkError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> Self {
        NetworkError::Io(err.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanTarget {
    pub cidr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub ip: String,
    pub hostname: Option<String>,
    pub mac_address: Option<String>,
    pub mac_vendor: Option<String>,
    pub open_ports: Vec<OpenPort>,
    pub os_guess: Option<String>,
    pub response_time_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenPort {
    pub port: u16,
    pub service_name: String,
    pub protocol: String,
    /// Raw first-line banner for server-speaks-first protocols (SSH/FTP/SMTP/POP3/IMAP/MySQL/VNC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
    /// Parsed product+version summary, e.g. "nginx 1.28.1", "OpenSSH 10.0p2 (Debian)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `<title>` extracted from an HTTP(S)-shaped response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_title: Option<String>,
    /// TLS certificate detail, present for any port where a TLS handshake succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsCertInfo>,
}

/// Certificate detail captured from a native TLS handshake (subject/issuer/SAN/validity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsCertInfo {
    pub subject_cn: Option<String>,
    pub subject_org: Option<String>,
    pub issuer_org: Option<String>,
    pub san: Vec<String>,
    /// RFC3339 "not after" (expiry) timestamp, if parseable.
    pub not_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WolTarget {
    pub mac_address: String,
    pub broadcast_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRule {
    pub id: String,
    pub name: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub tunnel_type: TunnelType,
    pub ssh_session_ref: Option<String>,
    pub auto_start: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelType {
    Local,
    Remote,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelStatus {
    Active,
    Inactive,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileServerConfig {
    pub directory: String,
    pub port: u16,
    pub server_type: FileServerType,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileServerType {
    Http,
    Tftp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileServerInfo {
    pub id: String,
    pub directory: String,
    pub port: u16,
    pub server_type: FileServerType,
    pub running: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub scan_id: String,
    pub hosts_scanned: u32,
    pub total_hosts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanHostFound {
    pub scan_id: String,
    pub result: ScanResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatusEvent {
    pub rule_id: String,
    pub status: TunnelStatus,
}

// ── Network Explore Types ───────────────────────────────────────────────

/// Well-known service ports for the network explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFilter {
    Ssh,
    Vnc,
    Rdp,
    Http,
    Https,
    Telnet,
    Ftp,
    Smb,
    Mysql,
    Postgresql,
    Redis,
    Mongodb,
    // New protocol breadth additions
    WinRm,
    WinRmTls,
    Mqtt,
    MqttTls,
    Netconf,
    Grpc,
    KubeApi,
    DockerApi,
    DockerApiTls,
    WsTerminal,
    Rtsp,
    Custom(u16),
}

impl ServiceFilter {
    fn port(&self) -> u16 {
        match self {
            ServiceFilter::Ssh => 22,
            ServiceFilter::Vnc => 5900,
            ServiceFilter::Rdp => 3389,
            ServiceFilter::Http => 80,
            ServiceFilter::Https => 443,
            ServiceFilter::Telnet => 23,
            ServiceFilter::Ftp => 21,
            ServiceFilter::Smb => 445,
            ServiceFilter::Mysql => 3306,
            ServiceFilter::Postgresql => 5432,
            ServiceFilter::Redis => 6379,
            ServiceFilter::Mongodb => 27017,
            ServiceFilter::WinRm => 5985,
            ServiceFilter::WinRmTls => 5986,
            ServiceFilter::Mqtt => 1883,
            ServiceFilter::MqttTls => 8883,
            ServiceFilter::Netconf => 830,
            ServiceFilter::Grpc => 50051,
            ServiceFilter::KubeApi => 6443,
            ServiceFilter::DockerApi => 2375,
            ServiceFilter::DockerApiTls => 2376,
            ServiceFilter::WsTerminal => 7681,
            ServiceFilter::Rtsp => 554,
            ServiceFilter::Custom(p) => *p,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreTarget {
    pub cidr: String,
    /// Well-known service filters to scan. If empty, defaults to SSH/VNC/RDP/HTTP/HTTPS.
    pub services: Vec<ServiceFilter>,
    /// Additional arbitrary ports supplied by the user.
    pub extra_ports: Vec<u16>,
    /// Per-host TCP connect timeout in milliseconds (default 1500).
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreResult {
    pub ip: String,
    pub hostname: Option<String>,
    pub mac_address: Option<String>,
    pub mac_vendor: Option<String>,
    pub open_ports: Vec<OpenPort>,
    pub os_guess: Option<String>,
    pub response_time_ms: f64,
    /// Quick-connect session type derived from the highest-priority open port.
    pub suggested_session_type: Option<String>,
    /// ICMP TTL from the ping reply, used as a fallback OS-family signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u8>,
    /// mDNS/Bonjour service records observed for this IP.
    #[serde(default)]
    pub mdns: Vec<MdnsRecord>,
    /// Human-readable notes explaining the hostname/OS/vendor findings above.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// A single mDNS/Bonjour service instance advertised by a host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdnsRecord {
    /// e.g. "_home-assistant._tcp.local."
    pub service_type: String,
    /// e.g. "Home"
    pub instance_name: String,
    pub hostname: Option<String>,
    #[serde(default)]
    pub txt: HashMap<String, String>,
}

/// Best-effort human-readable name for a host from its mDNS records, for
/// callers (like [`run_explore_and_dump`]) that have the full record set
/// in hand and want a single display name rather than the raw list. Mirrors
/// `deriveMdnsHostname` in `NetworkExplorer.tsx`, which the live streaming
/// UI applies at merge time instead — its host and mDNS results arrive as
/// two separate async event streams with no guaranteed order, so that
/// derivation has to happen client-side wherever the two are joined.
/// Priority: Google Cast/Chromecast TXT "fn" (the most human-readable label
/// a device self-reports) → advertised `hostname` (real DNS-style name, but
/// Cast devices often set this to an opaque device UUID) → instance name.
fn derive_mdns_hostname(records: &[MdnsRecord]) -> Option<String> {
    if let Some(fn_name) = records.iter().find_map(|r| r.txt.get("fn")) {
        return Some(fn_name.clone());
    }
    if let Some(host) = records.iter().find_map(|r| r.hostname.clone()) {
        return Some(host);
    }
    records.first().map(|r| r.instance_name.clone())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreProgress {
    pub scan_id: String,
    pub hosts_scanned: u32,
    pub total_hosts: u32,
    pub hosts_found: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreHostFound {
    pub scan_id: String,
    pub result: ExploreResult,
}

/// Emitted once the concurrent mDNS/Bonjour browse for a scan completes,
/// since records may resolve after some hosts have already been reported.
/// The frontend merges these into already-rendered rows by IP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreMdnsUpdate {
    pub scan_id: String,
    pub records: HashMap<String, Vec<MdnsRecord>>,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Parse a CIDR notation string into a list of IPv4 addresses.
fn parse_cidr(cidr: &str) -> Result<Vec<Ipv4Addr>, NetworkError> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return Err(NetworkError::InvalidCidr(cidr.to_string()));
    }

    let base_ip: Ipv4Addr = parts[0]
        .parse()
        .map_err(|_| NetworkError::InvalidCidr(cidr.to_string()))?;
    let prefix_len: u32 = parts[1]
        .parse()
        .map_err(|_| NetworkError::InvalidCidr(cidr.to_string()))?;

    if prefix_len > 32 {
        return Err(NetworkError::InvalidCidr(cidr.to_string()));
    }

    let ip_u32 = u32::from(base_ip);
    let host_bits = 32 - prefix_len;

    if host_bits == 0 {
        return Ok(vec![base_ip]);
    }

    // Reject prefixes broader than /16 (65 536 hosts) — prevents OOM and runaway scans.
    if host_bits > 16 {
        return Err(NetworkError::InvalidCidr(format!(
            "/{prefix_len} is too broad; maximum scan range is /16 (65 536 hosts)"
        )));
    }

    let mask = !((1u32 << host_bits) - 1);
    let network = ip_u32 & mask;
    let count = 1u32 << host_bits;

    let mut addrs = Vec::new();
    for i in 0..count {
        addrs.push(Ipv4Addr::from(network + i));
    }

    Ok(addrs)
}

/// Parse a MAC address string (xx:xx:xx:xx:xx:xx or xx-xx-xx-xx-xx-xx) into 6 bytes.
fn parse_mac(mac: &str) -> Result<[u8; 6], NetworkError> {
    let cleaned = mac.replace(['-', '.'], ":");
    let parts: Vec<&str> = cleaned.split(':').collect();
    if parts.len() != 6 {
        return Err(NetworkError::InvalidMac(mac.to_string()));
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] =
            u8::from_str_radix(part, 16).map_err(|_| NetworkError::InvalidMac(mac.to_string()))?;
    }
    Ok(bytes)
}

/// Build a Wake-on-LAN magic packet (6x 0xFF + 16x MAC address = 102 bytes).
pub fn build_wol_packet(mac_bytes: &[u8; 6]) -> Vec<u8> {
    let mut packet = vec![0xFFu8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(mac_bytes);
    }
    packet
}

/// Guess service name from port number.
///
/// These strings are also what `suggest_session_type` returns, so an
/// `OpenPort.service_name` from a real scan always matches the suggested
/// type string exactly — the frontend looks up the actual detected port via
/// `open_ports.find(p => p.service_name === suggested_session_type)`, and a
/// naming mismatch here would silently fall back to a wrong hardcoded port.
fn guess_service(port: u16) -> String {
    match port {
        21 => "ftp".to_string(),
        22 => "ssh".to_string(),
        23 => "telnet".to_string(),
        25 => "smtp".to_string(),
        53 => "dns".to_string(),
        80 => "http".to_string(),
        110 => "pop3".to_string(),
        143 => "imap".to_string(),
        161 => "snmp".to_string(),
        443 => "https".to_string(),
        445 => "smb".to_string(),
        513 => "rlogin".to_string(),
        554 => "rtsp".to_string(),
        623 => "ipmi".to_string(),
        830 => "netconf".to_string(),
        993 => "imaps".to_string(),
        995 => "pop3s".to_string(),
        1883 => "mqtt".to_string(),
        2049 => "nfs".to_string(),
        2375 => "docker-api".to_string(),
        2376 => "docker-api-tls".to_string(),
        3306 => "mysql".to_string(),
        3389 => "rdp".to_string(),
        5432 => "postgresql".to_string(),
        5900 => "vnc".to_string(),
        5985 => "winrm".to_string(),
        5986 => "winrm-tls".to_string(),
        6379 => "redis".to_string(),
        6443 => "kube-api".to_string(),
        7681 => "wsterm".to_string(),
        8006 => "proxmox".to_string(),
        8080 => "http-alt".to_string(),
        8443 => "https-alt".to_string(),
        8883 => "mqtt-tls".to_string(),
        8096 => "jellyfin".to_string(),
        27017 => "mongodb".to_string(),
        32400 => "plex".to_string(),
        50051 => "grpc".to_string(),
        _ => format!("port-{}", port),
    }
}

/// Ports checked for a suggested session type, in priority order. The
/// returned string is always `guess_service(port)` for whichever port wins
/// (see `suggest_session_type`) — listing ports here instead of
/// hand-writing each name is what guarantees that: hand-writing them
/// previously drifted out of sync for the "-tls" port variants (2376,
/// 5986, 8883 all fell through to their plain sibling's name, e.g. port
/// 2376 returned "docker-api" instead of guess_service's "docker-api-tls",
/// so the frontend's port lookup by service_name silently missed and fell
/// back to a hardcoded, wrong port).
const SUGGEST_TYPE_PRIORITY_PORTS: &[u16] = &[
    22, 3389, 5900, 5985, 5986, 6443, 2375, 2376, 1883, 8883,
    50051, 830, 7681, 445, 2049, 513, 8006, 623, 161, 23, 21,
];

/// Suggest the best session type from open ports (priority order).
///
/// Returns the exact same string `guess_service` uses for the winning
/// port (see its doc comment for why that matters) by construction —
/// derived directly from `guess_service`, not a separately hand-written copy.
fn suggest_session_type(open_ports: &[OpenPort]) -> Option<String> {
    let ports: Vec<u16> = open_ports.iter().map(|p| p.port).collect();
    SUGGEST_TYPE_PRIORITY_PORTS
        .iter()
        .find(|&&p| ports.contains(&p))
        .map(|&p| guess_service(p))
}

/// Upgrade a port-only suggestion once enrichment has actually confirmed
/// Redfish or WebDAV via their HTTP-level signature (see `probe_redfish`/
/// `probe_webdav`) — those can't be told apart from a generic web server by
/// port number alone, so `suggest_session_type` can't offer them on its
/// own. Only overrides when the current suggestion is empty or was just a
/// generic https/http guess-by-port, never a real protocol match like SSH.
fn refine_suggested_type(current: Option<String>, open_ports: &[OpenPort]) -> Option<String> {
    if matches!(
        current.as_deref(),
        Some(
            "ssh" | "rdp" | "vnc" | "winrm" | "winrm-tls" | "kube-api" | "docker-api"
                | "docker-api-tls" | "mqtt" | "mqtt-tls" | "grpc" | "netconf" | "wsterm"
                | "smb" | "nfs" | "rlogin" | "proxmox" | "ipmi" | "snmp"
        )
    ) {
        return current;
    }
    let redfish = open_ports
        .iter()
        .find(|p| REDFISH_PROBE_PORTS.contains(&p.port) && p.version.as_deref().is_some_and(|v| v.starts_with("Redfish")));
    if redfish.is_some() {
        return Some("redfish".to_string());
    }
    let webdav = open_ports
        .iter()
        .find(|p| WEBDAV_PROBE_PORTS.contains(&p.port) && p.version.as_deref() == Some("WebDAV"));
    if webdav.is_some() {
        return Some("webdav".to_string());
    }
    current
}

/// Common ports to scan by default.
const DEFAULT_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 143, 443, 445,
    513, 830, 993, 995, 1883, 2049, 2375, 2376, 3306, 3389,
    5432, 5900, 5985, 5986, 6379, 6443, 7681, 8006, 8080, 8443,
    8883, 27017, 50051,
];

// ── Interface-bound connect ────────────────────────────────────────────
// A VPN client (Tailscale, in the case that motivated this) can install a
// competing route for the exact same LAN prefix we're scanning via its
// tunnel interface (e.g. because some peer on the tailnet advertises that
// subnet as a route). When our probe traffic loses that race and goes out
// the tunnel instead of the physical NIC, the connection can still succeed
// (the VPN forwards it), but no ARP entry is ever created for the
// destination on the physical interface — so MAC/vendor lookup fails
// *permanently* for that host, not just intermittently, and retrying the
// ARP-cache read (see `resolve_arp_mac`) can never fix it. Forcing our
// scan sockets onto the interface that actually owns the target subnet
// closes that race deterministically.
tokio::task_local! {
    static BOUND_INTERFACE: Option<String>;
}

fn current_bound_interface() -> Option<String> {
    BOUND_INTERFACE.try_with(|v| v.clone()).unwrap_or(None)
}

// ── Local-DNS-server-direct reverse DNS ─────────────────────────────────
// Routers/gateways commonly run a tiny local DNS zone populated from DHCP
// client hostnames (e.g. "jellyfin2.home", "device-70.home") — and so does
// anything else acting as the LAN's DNS server (a Pi-hole, a NAS, a
// Windows AD box) — but each only answers PTR queries for its own zone if
// asked *directly*. The system's configured resolver (which might be a
// VPN's DNS, an upstream ISP resolver, or anything else `/etc/resolv.conf`/
// scutil points at) has no idea any of these zones exist. Confirmed on a
// real LAN: `dig -x <ip>` via the default resolver came back empty for 8
// hosts (VMs, phones, cameras) that `dig -x <ip> @<router>` resolved
// instantly.
//
// Rather than hardcode "the gateway" as the one local DNS server worth
// asking, this scan discovers candidates the same way it discovers
// everything else — by finding hosts with port 53 open — and grows the
// candidate set as the scan progresses, so hosts resolved later benefit
// from DNS servers a concurrent host task found earlier. Seeded up front
// with the default-route gateway and the CIDR's conventional `.1`, since
// both are free/instant and cover the common case before any host has
// even been probed yet.
type DnsServerRegistry = Arc<tokio::sync::RwLock<HashSet<IpAddr>>>;

fn new_dns_server_registry() -> DnsServerRegistry {
    Arc::new(tokio::sync::RwLock::new(HashSet::new()))
}

/// Seeds a registry with the default-route gateway and the CIDR's
/// conventional `.1` address (a near-universal home-router convention,
/// cheap to try even when wrong — a bad guess just times out like any
/// other closed port).
async fn seed_dns_server_registry(cidr: &str) -> DnsServerRegistry {
    let registry = new_dns_server_registry();
    let mut servers = registry.write().await;
    if let Some(gw) = detect_default_gateway().await {
        servers.insert(gw);
    }
    if let Some(dot_one) = conventional_dot_one(cidr) {
        servers.insert(dot_one);
    }
    drop(servers);
    registry
}

/// The `.1` address of `cidr`'s network (e.g. `192.168.0.1` for
/// `192.168.0.0/24`) — the overwhelmingly common convention for a home
/// router/gateway's own address, worth trying even before any host in the
/// range has actually been probed. `None` for prefixes too small to have a
/// meaningful "first host" (/31, /32).
fn conventional_dot_one(cidr: &str) -> Option<IpAddr> {
    let (net_str, prefix_str) = cidr.split_once('/')?;
    let network: Ipv4Addr = net_str.parse().ok()?;
    let prefix: u32 = prefix_str.parse().ok()?;
    if prefix >= 31 {
        return None;
    }
    let mask: u32 = u32::MAX << (32 - prefix);
    let network_addr = u32::from(network) & mask;
    Some(IpAddr::V4(Ipv4Addr::from(network_addr | 1)))
}

/// The system's default-route gateway (e.g. `192.168.0.1`), however this
/// scan's own outbound traffic actually gets there — independent of and
/// complementary to `interface_for_cidr`, which only identifies a subnet
/// this machine is *directly attached to*.
async fn detect_default_gateway() -> Option<IpAddr> {
    #[cfg(target_os = "macos")]
    {
        let output = tokio::process::Command::new("route").args(["-n", "get", "default"]).output().await.ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .find_map(|l| l.trim().strip_prefix("gateway:"))
            .and_then(|g| g.trim().parse().ok())
    }
    #[cfg(target_os = "linux")]
    {
        let output = tokio::process::Command::new("ip").args(["route", "show", "default"]).output().await.ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        // "default via 192.168.0.1 dev eth0 ..."
        let mut tokens = text.split_whitespace();
        while let Some(tok) = tokens.next() {
            if tok == "via" {
                return tokens.next().and_then(|g| g.parse().ok());
            }
        }
        None
    }
    #[cfg(target_os = "windows")]
    {
        let output = tokio::process::Command::new("route").args(["print", "0.0.0.0"]).output().await.ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().find_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3 && cols[0] == "0.0.0.0" && cols[1] == "0.0.0.0" {
                cols[2].parse().ok()
            } else {
                None
            }
        })
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    { None }
}

/// Registers `ip` as a candidate local DNS server for the rest of this
/// scan, if `open_ports` shows port 53 open on it. Called for every host
/// as its own port scan completes — this is the "search for ... any
/// router/DHCP-adjacent DNS server" discovery step: no vendor/brand
/// assumptions, just "does something answer on port 53".
async fn register_if_dns_server(ip: IpAddr, open_ports: &[OpenPort], registry: &DnsServerRegistry) {
    if open_ports.iter().any(|p| p.port == 53) {
        registry.write().await.insert(ip);
    }
}

/// Method 9 — reverse DNS against every currently-known local DNS server
/// directly (concurrently), bypassing whatever DNS server the system
/// resolver is actually configured to use. Takes the first hit.
async fn reverse_dns_via_known_servers(ip: String, registry: &DnsServerRegistry) -> Option<String> {
    let servers: Vec<IpAddr> = registry.read().await.iter().copied().collect();
    let queries = servers.into_iter().map(|server| {
        let ip = ip.clone();
        async move {
            let output = tokio::time::timeout(
                Duration::from_secs(3),
                tokio::process::Command::new("dig")
                    .args(["+short", "+time=2", "+tries=1", "-x", &ip, &format!("@{server}")])
                    .output(),
            ).await.ok()?.ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            let name = text.lines()
                .find(|l| !l.starts_with(';') && !l.trim().is_empty())?
                .trim()
                .trim_end_matches('.')
                .to_string();
            if name.is_empty() { None } else { Some(name) }
        }
    });
    futures::future::join_all(queries).await.into_iter().flatten().next()
}

/// The local interface whose directly-connected subnet matches `cidr`
/// exactly (e.g. "en0" for "192.168.0.0/24" when that's this machine's own
/// LAN segment). `None` when no local interface owns this subnet (routing
/// a scan through a gateway rather than a directly-attached LAN), in which
/// case binding isn't meaningful and default OS routing is used.
fn interface_for_cidr(cidr: &str) -> Option<String> {
    let (net_str, prefix_str) = cidr.split_once('/')?;
    let network: Ipv4Addr = net_str.parse().ok()?;
    let prefix: u32 = prefix_str.parse().ok()?;
    let mask: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    let target_net = u32::from(network) & mask;

    let interfaces = if_addrs::get_if_addrs().ok()?;
    interfaces.into_iter().find_map(|iface| {
        if iface.is_loopback() {
            return None;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            let iface_mask = u32::from(v4.netmask);
            let iface_net = u32::from(v4.ip) & iface_mask;
            if iface_mask == mask && iface_net == target_net {
                return Some(iface.name);
            }
        }
        None
    })
}

#[cfg(target_os = "macos")]
fn bind_socket_to_interface(socket: &socket2::Socket, iface: &str) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let cstr = std::ffi::CString::new(iface)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let index = unsafe { libc::if_nametoindex(cstr.as_ptr()) };
    if index == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let ret = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_BOUND_IF,
            &index as *const _ as *const libc::c_void,
            std::mem::size_of::<u32>() as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn bind_socket_to_interface(socket: &socket2::Socket, iface: &str) -> std::io::Result<()> {
    socket.bind_device(Some(iface.as_bytes()))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn bind_socket_to_interface(_socket: &socket2::Socket, _iface: &str) -> std::io::Result<()> {
    // No portable equivalent wired up on other targets (e.g. Windows would
    // need IP_UNICAST_IF with an interface *index*, not name); scans there
    // fall back to default OS routing, same as before this fix.
    Ok(())
}

/// `TcpStream::connect`, but bound to `BOUND_INTERFACE` (set for the
/// duration of a scan's per-host task) when one applies to the target
/// subnet — see the module note above `BOUND_INTERFACE`.
async fn connect_bound(ip: IpAddr, port: u16, timeout: Duration) -> std::io::Result<TcpStream> {
    let bound_if = current_bound_interface();
    let addr = SocketAddr::new(ip, port);
    let std_stream = tokio::task::spawn_blocking(move || -> std::io::Result<std::net::TcpStream> {
        let domain = if addr.is_ipv4() { socket2::Domain::IPV4 } else { socket2::Domain::IPV6 };
        let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
        if let Some(iface) = bound_if.as_deref() {
            bind_socket_to_interface(&socket, iface)?;
        }
        socket.connect_timeout(&addr.into(), timeout)?;
        socket.set_nonblocking(true)?;
        Ok(socket.into())
    })
    .await
    .map_err(std::io::Error::other)??;
    TcpStream::from_std(std_stream)
}

/// Try to connect to a TCP port with a timeout.
async fn check_port(ip: IpAddr, port: u16, timeout: Duration) -> Option<OpenPort> {
    match connect_bound(ip, port, timeout).await {
        Ok(_stream) => Some(OpenPort {
            port,
            service_name: guess_service(port),
            protocol: "tcp".to_string(),
            ..Default::default()
        }),
        _ => None,
    }
}

/// Probe a host using the system `ping` command (ICMP echo).
/// Returns `(alive, ttl)` — `ttl` is parsed from the reply when present and
/// is used as a low-cost, no-privileges-required OS-family signal (64 ~
/// Linux/macOS/BSD, 128 ~ Windows, 255 ~ network gear/embedded Linux).
/// Uses the OS `ping` binary so it works without raw-socket privileges.
async fn ping_host(ip: IpAddr, timeout: Duration) -> (bool, Option<u8>) {
    let ip_str = ip.to_string();
    // See the `BOUND_INTERFACE` note above `connect_bound`: same VPN-route
    // race applies to ping's ICMP socket, so bind it the same way. Windows'
    // `ping` argv below has no interface-binding equivalent, so it's unused
    // on that platform.
    #[cfg_attr(target_os = "windows", allow(unused_variables))]
    let bound_if = current_bound_interface();

    // Platform-specific argv for a single-shot ping.
    #[cfg(target_os = "windows")]
    let args: Vec<String> = {
        let timeout_ms = timeout.as_millis().max(100).to_string();
        vec!["-n".into(), "1".into(), "-w".into(), timeout_ms, ip_str]
    };
    #[cfg(target_os = "macos")]
    let args: Vec<String> = {
        // BSD ping: -W is in milliseconds. Apple's `-b boundif` must precede the host.
        let timeout_ms = timeout.as_millis().max(100).to_string();
        let mut a = vec!["-c".into(), "1".into(), "-W".into(), timeout_ms];
        if let Some(iface) = &bound_if {
            a.push("-b".into());
            a.push(iface.clone());
        }
        a.push(ip_str);
        a
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let args: Vec<String> = {
        // Linux ping: -W is in seconds; -I binds to an interface.
        let timeout_secs = timeout.as_secs().max(1).to_string();
        let mut a = vec!["-c".into(), "1".into(), "-W".into(), timeout_secs];
        if let Some(iface) = &bound_if {
            a.push("-I".into());
            a.push(iface.clone());
        }
        a.push(ip_str);
        a
    };

    let result = tokio::time::timeout(
        timeout + Duration::from_millis(500),
        tokio::process::Command::new("ping")
            .args(&args)
            .stderr(std::process::Stdio::null())
            .output(),
    ).await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            (true, parse_ping_ttl(&text))
        }
        _ => (false, None),
    }
}

/// Extract the ICMP TTL from `ping` output text (matches `ttl=NN` or `TTL=NN`).
fn parse_ping_ttl(text: &str) -> Option<u8> {
    let lower = text.to_ascii_lowercase();
    let pos = lower.find("ttl=")?;
    let rest = &text[pos + 4..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u8>().ok()
}

/// Aggressively resolve a hostname for an IP using all available methods in parallel.
/// open_ports is used to attempt TLS certificate hostname extraction.
/// Returns the first non-None result from 8 concurrent methods.
async fn resolve_hostname_aggressive(ip: IpAddr, open_ports: Vec<OpenPort>, dns_servers: &DnsServerRegistry) -> Option<String> {
    let ip_str = ip.to_string();

    // Check for any open TLS ports we can grab a cert from
    const TLS_PORTS: &[u16] = &[443, 5986, 6443, 8443, 8883, 50051];
    let tls_port = TLS_PORTS.iter()
        .find(|&&p| open_ports.iter().any(|op| op.port == p))
        .copied();

    let tls_fut = async move {
        if let Some(port) = tls_port {
            extract_tls_cert_hostname(ip, port).await
        } else {
            None
        }
    };

    let (r_dns, r_hosts, r_nslookup, r_netbios, r_arp, r_dig, r_host_cmd, r_tls, r_local_dns) = tokio::join!(
        reverse_dns_getnameinfo(ip),
        lookup_hosts_file(ip_str.clone()),
        nslookup_reverse(ip_str.clone()),
        nmblookup_name(ip_str.clone()),
        arp_hostname(ip_str.clone()),
        dig_reverse(ip_str.clone()),
        host_reverse(ip_str.clone()),
        tls_fut,
        reverse_dns_via_known_servers(ip_str.clone(), dns_servers),
    );

    r_dns.or(r_hosts).or(r_nslookup).or(r_netbios).or(r_arp).or(r_dig).or(r_host_cmd).or(r_tls).or(r_local_dns)
}

/// Method 1 — getnameinfo with timeout: PTR + mDNS via the system resolver.
async fn reverse_dns_getnameinfo(ip: IpAddr) -> Option<String> {
    tokio::time::timeout(
        Duration::from_secs(4),
        tokio::task::spawn_blocking(move || dns_lookup_via_getnameinfo(ip)),
    ).await.ok()?.ok().flatten()
}

#[cfg(unix)]
fn dns_lookup_via_getnameinfo(ip: IpAddr) -> Option<String> {
    use std::ffi::CStr;
    use std::mem;

    let ip_str = ip.to_string();
    let mut host = [0i8; 256];

    let ret = match ip {
        IpAddr::V4(v4) => unsafe {
            let mut sin: libc::sockaddr_in = mem::zeroed();
            sin.sin_family = libc::AF_INET as libc::sa_family_t;
            sin.sin_addr.s_addr = u32::from_ne_bytes(v4.octets());
            libc::getnameinfo(
                &sin as *const libc::sockaddr_in as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                host.as_mut_ptr(),
                host.len() as libc::socklen_t,
                std::ptr::null_mut(),
                0,
                0, // no NI_NAMEREQD — allows mDNS/.local resolution via system resolver
            )
        },
        IpAddr::V6(v6) => unsafe {
            let mut sin6: libc::sockaddr_in6 = mem::zeroed();
            sin6.sin6_family = libc::AF_INET6 as libc::sa_family_t;
            sin6.sin6_addr.s6_addr = v6.octets();
            libc::getnameinfo(
                &sin6 as *const libc::sockaddr_in6 as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
                host.as_mut_ptr(),
                host.len() as libc::socklen_t,
                std::ptr::null_mut(),
                0,
                0,
            )
        },
    };

    if ret == 0 {
        let cstr = unsafe { CStr::from_ptr(host.as_ptr()) };
        let resolved = cstr.to_string_lossy().into_owned();
        if resolved != ip_str { Some(resolved) } else { None }
    } else {
        None
    }
}

#[cfg(not(unix))]
fn dns_lookup_via_getnameinfo(_ip: IpAddr) -> Option<String> {
    // On Windows use nslookup (spawned below); getnameinfo is available but
    // the nslookup path is cleaner across MSVC and GNUC toolchains.
    None
}

/// Method 2 — /etc/hosts (or Windows hosts file): instant, no network needed.
async fn lookup_hosts_file(ip: String) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let path = r"C:\Windows\System32\drivers\etc\hosts";
        #[cfg(not(target_os = "windows"))]
        let path = "/etc/hosts";

        let content = std::fs::read_to_string(path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() { continue; }
            let mut parts = line.split_whitespace();
            if parts.next()? == ip {
                return parts.next().map(str::to_string);
            }
        }
        None
    }).await.ok().flatten()
}

/// Method 3 — nslookup: cross-platform DNS reverse lookup (works on macOS, Linux, Windows).
async fn nslookup_reverse(ip: String) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("nslookup").arg(&ip).output(),
    ).await.ok()?.ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    // Output contains lines like: "1.1.168.192.in-addr.arpa  name = hostname.domain."
    for line in text.lines() {
        if let Some(pos) = line.find("name = ") {
            let name = line[pos + 7..].trim().trim_end_matches('.');
            if !name.is_empty() && !name.contains("NXDOMAIN") {
                return Some(name.to_string());
            }
        }
        // Windows nslookup sometimes uses "Name:" format
        if let Some(rest) = line.strip_prefix("Name:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Method 4 — nmblookup: NetBIOS name resolution for Windows hosts (requires Samba/winbind).
/// Falls back gracefully if nmblookup is not installed.
async fn nmblookup_name(ip: String) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("nmblookup")
            .args(["-A", &ip])
            .output(),
    ).await.ok()?.ok()?;

    if !output.status.success() { return None; }
    let text = String::from_utf8_lossy(&output.stdout);
    // Look for <00> workstation/computer name entries (not GROUP entries)
    for line in text.lines() {
        if line.contains("<00>") && !line.contains("GROUP") {
            let name = line.split_whitespace().next()?;
            if !name.is_empty() && name != "Looking" {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Look up the MAC address for an IP from the system ARP cache. One short
/// retry covers the case where the cache write from the TCP connect/ping
/// that just happened hasn't landed yet.
async fn resolve_arp_mac(ip: IpAddr) -> Option<String> {
    for delay_ms in [0, 150, 350] {
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        if let Some(mac) = resolve_arp_mac_once(ip).await {
            return Some(mac);
        }
    }
    None
}

async fn resolve_arp_mac_once(ip: IpAddr) -> Option<String> {
    let ip_str = ip.to_string();
    let output = tokio::process::Command::new("arp").args(["-n", &ip_str]).output().await.ok()?;
    parse_arp_mac_output(&String::from_utf8_lossy(&output.stdout))
}

/// Extract and canonicalize a MAC address from `arp -n` output. macOS/BSD's
/// `arp` does **not** zero-pad single-hex-digit octets — a real line looks
/// like `? (192.168.0.39) at ac:a7:f1:8:6:a7 on en0 ifscope [ethernet]`, not
/// `...:08:06:a7`. A naive `{2}`-per-octet regex silently fails to match
/// roughly 2 in 5 real MAC addresses (any with at least one octet < 0x10),
/// which looked exactly like an intermittent timing race — the same devices
/// failed every single time, retries never helped — but was actually a
/// deterministic parse miss. Padding each octet back to 2 digits here also
/// matters for `lookup_mac_vendor`'s downstream OUI-prefix slice, which
/// assumes a fully-padded, colon-free 12-hex-digit string.
fn parse_arp_mac_output(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:[0-9a-fA-F]{1,2}[:\-]){5}[0-9a-fA-F]{1,2}").ok()?;
    let raw = re.find(text)?.as_str();
    let octets: Vec<String> = raw
        .split([':', '-'])
        .map(|o| format!("{:0>2}", o.to_ascii_uppercase()))
        .collect();
    if octets.len() != 6 {
        return None;
    }
    Some(octets.join(":"))
}

/// Method 5 — arp: extract hostname from the ARP cache line for this IP.
/// On macOS/Linux, `arp -n <ip>` may print the hostname resolved from the ARP table.
/// Example macOS output: `homelab.local (192.168.1.5) at aa:bb:cc:dd:ee:ff ...`
async fn arp_hostname(ip: String) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(1),
        tokio::process::Command::new("arp").arg(&ip).output(),
    ).await.ok()?.ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    // macOS/Linux format: "hostname (ip) at mac ..."
    // The hostname is the first token if it differs from the IP.
    let first_token = text.split_whitespace().next()?.to_string();
    if first_token != ip && first_token != "?" && !first_token.is_empty() {
        return Some(first_token.trim_end_matches('.').to_string());
    }
    None
}

/// Method 6 — dig +short -x: cleaner than nslookup; returns nothing on failure.
async fn dig_reverse(ip: String) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("dig")
            .args(["+short", "+time=2", "+tries=1", "-x", &ip])
            .output(),
    ).await.ok()?.ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let name = text.lines()
        .find(|l| !l.starts_with(';') && !l.trim().is_empty())?
        .trim()
        .trim_end_matches('.');
    if !name.is_empty() { Some(name.to_string()) } else { None }
}

/// Method 7 — host command: POSIX reverse DNS, available on macOS and most Linux distros.
async fn host_reverse(ip: String) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("host").arg(&ip).output(),
    ).await.ok()?.ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("domain name pointer") {
            let name = line.split("domain name pointer").nth(1)?
                .trim().trim_end_matches('.');
            if !name.is_empty() { return Some(name.to_string()); }
        }
    }
    None
}

/// Method 8 — TLS cert CN/SAN: extract hostname from X.509 cert on any open TLS port.
/// Works even when DNS has no PTR record (HTTPS, WinRM-TLS, K8s API, gRPC-TLS, MQTT-TLS...).
async fn extract_tls_cert_hostname(ip: IpAddr, port: u16) -> Option<String> {
    let cert = probe_tls_cert(ip, port, Duration::from_secs(5)).await?;
    pick_hostname_from_cert(cert)
}

/// Derive a hostname from a TLS certificate: prefer the first non-wildcard
/// SAN DNS entry (more authoritative than CN), fall back to the subject CN
/// — unless that's *also* a bare wildcard (`CN=*`, common on self-signed
/// certs like Proxmox/Heimdall's default cert), which isn't a hostname for
/// anything and must not be returned as if it were one.
fn pick_hostname_from_cert(cert: TlsCertInfo) -> Option<String> {
    cert.san.into_iter().find(|n| !n.starts_with('*'))
        .or(cert.subject_cn.filter(|cn| !cn.starts_with('*')))
}

// ── TLS certificate inspection ─────────────────────────────────────────────
// Native rustls handshake + x509-parser, replacing a previous `sh -c
// "openssl s_client | openssl x509"` shell-out that silently did nothing on
// Windows (no `sh`/`openssl` CLI guaranteed). We deliberately accept any
// certificate here — including self-signed/expired ones — because the goal
// is to *inspect* what a device presents, not to validate trust.

/// Cert verifier that accepts everything. Same pattern already used for
/// permissive TLS in `mqtt::MqttNoCertVerifier` / `websocket_term::NoCertVerifier`.
#[derive(Debug)]
struct AcceptAllCertVerifier;

impl rustls::client::danger::ServerCertVerifier for AcceptAllCertVerifier {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer, _: &[rustls::pki_types::CertificateDer], _: &rustls::pki_types::ServerName, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> { Ok(rustls::client::danger::ServerCertVerified::assertion()) }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> { rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes() }
}

fn tls_client_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: std::sync::OnceLock<Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    CONFIG.get_or_init(|| {
        // Idempotent: no-ops (returns Err, ignored) if a provider is already installed.
        let _ = rustls::crypto::ring::default_provider().install_default();
        Arc::new(
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAllCertVerifier))
                .with_no_client_auth(),
        )
    }).clone()
}

/// Connect + TLS handshake against `ip:port`, returning the decrypted stream.
async fn tls_connect(ip: IpAddr, port: u16, timeout: Duration) -> Option<tokio_rustls::client::TlsStream<TcpStream>> {
    let tcp = connect_bound(ip, port, timeout).await.ok()?;
    let connector = tokio_rustls::TlsConnector::from(tls_client_config());
    let server_name = rustls::pki_types::ServerName::from(ip);
    tokio::time::timeout(Duration::from_secs(4), connector.connect(server_name, tcp)).await.ok()?.ok()
}

/// Handshake against a TLS port and parse the peer's leaf certificate.
async fn probe_tls_cert(ip: IpAddr, port: u16, timeout: Duration) -> Option<TlsCertInfo> {
    let tls_stream = tls_connect(ip, port, timeout).await?;
    let (_, conn) = tls_stream.get_ref();
    let der = conn.peer_certificates()?.first()?.clone();
    parse_cert_der(&der)
}

fn parse_cert_der(der: &rustls::pki_types::CertificateDer<'_>) -> Option<TlsCertInfo> {
    let (_, cert) = x509_parser::parse_x509_certificate(der.as_ref()).ok()?;

    let subject_cn = cert.subject().iter_common_name().next().and_then(|a| a.as_str().ok()).map(String::from);
    let subject_org = cert.subject().iter_organization().next().and_then(|a| a.as_str().ok()).map(String::from);
    let issuer_org = cert.issuer().iter_organization().next().and_then(|a| a.as_str().ok()).map(String::from);

    let san: Vec<String> = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|ext| {
            ext.value
                .general_names
                .iter()
                .filter_map(|gn| match gn {
                    x509_parser::extensions::GeneralName::DNSName(s) => Some(s.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let not_after = Some(cert.validity().not_after.to_string());

    Some(TlsCertInfo { subject_cn, subject_org, issuer_org, san, not_after })
}

// ── Port enrichment (banner grabbing / lightweight version detection) ─────
// Ports where the server sends an identification banner immediately on
// connect, before the client sends anything.
const BANNER_FIRST_PORTS: &[u16] = &[21, 22, 25, 110, 143, 3306, 5900];
/// Plaintext-HTTP ports probed with a GET.
const HTTP_PORTS: &[u16] = &[80, 8080];
/// TLS ports: certificate is always captured; an HTTP GET is also attempted
/// since many admin UIs/REST APIs live on 443/8443/6443/etc.
const TLS_PROBE_PORTS: &[u16] = &[443, 5986, 6443, 8443, 8883, 50051];
const REDFISH_PROBE_PORTS: &[u16] = &[443, 8443];
const WEBDAV_PROBE_PORTS: &[u16] = &[80, 443, 8080, 8443];

/// Connect and read whatever the server sends first, without sending
/// anything ourselves. Covers SSH/FTP/SMTP/POP3/IMAP/MySQL/VNC, which are
/// all "server speaks first" protocols.
async fn grab_banner(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let mut stream = connect_bound(ip, port, timeout).await.ok()?;

    let mut buf = [0u8; 256];
    let n = tokio::time::timeout(Duration::from_millis(1500), stream.read(&mut buf)).await.ok()?.ok()?;
    if n == 0 {
        return None;
    }
    let text = String::from_utf8_lossy(&buf[..n]);
    let first_line = text.lines().next()?.trim();
    if first_line.is_empty() { None } else { Some(first_line.to_string()) }
}

/// Shorten a raw banner into a "product version" summary where the shape is
/// recognized (currently: SSH identification strings); otherwise pass through.
fn summarize_banner(banner: &str) -> String {
    banner
        .strip_prefix("SSH-2.0-")
        .or_else(|| banner.strip_prefix("SSH-1.99-"))
        .unwrap_or(banner)
        .replace('_', " ")
}

fn extract_html_title(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = raw[start..end].trim();
    if title.is_empty() { None } else { Some(title.to_string()) }
}

async fn read_response_bounded<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) -> Option<String> {
    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(Duration::from_millis(2500), stream.read(&mut buf)).await.ok()?.ok()?;
    if n == 0 { return None; }
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

/// Send a minimal HTTP GET and parse the `Server:` header + `<title>`.
/// When `tls` is true the request goes over a TLS handshake (used for ports
/// where we already know a cert is present, e.g. 443/8443/6443).
async fn probe_http(ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> Option<(Option<String>, Option<String>)> {
    let request = format!(
        "GET / HTTP/1.0\r\nHost: {ip}\r\nUser-Agent: CrossTerm-NetworkExplorer\r\nConnection: close\r\n\r\n"
    );

    let raw = if tls {
        let mut stream = tls_connect(ip, port, timeout).await?;
        stream.write_all(request.as_bytes()).await.ok()?;
        read_response_bounded(&mut stream).await?
    } else {
        let mut stream = connect_bound(ip, port, timeout).await.ok()?;
        stream.write_all(request.as_bytes()).await.ok()?;
        read_response_bounded(&mut stream).await?
    };

    if !raw.starts_with("HTTP/") {
        return None;
    }

    let server = raw
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("server:"))
        .map(|l| l.split_once(':').map(|x| x.1).unwrap_or("").trim().to_string());
    let title = extract_html_title(&raw);

    Some((server, title))
}

/// RTSP OPTIONS probe — the `Public:` methods line is a reliable IP-camera signal.
async fn probe_rtsp(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let mut stream = connect_bound(ip, port, timeout).await.ok()?;
    let request = format!("OPTIONS rtsp://{ip} RTSP/1.0\r\nCSeq: 1\r\n\r\n");
    stream.write_all(request.as_bytes()).await.ok()?;

    let mut buf = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_millis(2000), stream.read(&mut buf)).await.ok()?.ok()?;
    if n == 0 { return None; }
    let text = String::from_utf8_lossy(&buf[..n]);
    text.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("public:"))
        .or_else(|| text.lines().next())
        .map(|l| l.trim().to_string())
}

/// Issue a minimal unauthenticated HTTP GET for `path` and return the raw
/// response body (headers stripped) if the server answered 2xx-shaped.
async fn http_get_body(ip: IpAddr, port: u16, path: &str, timeout: Duration) -> Option<String> {
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: {ip}\r\nUser-Agent: CrossTerm-NetworkExplorer\r\nConnection: close\r\n\r\n"
    );
    let mut stream = connect_bound(ip, port, timeout).await.ok()?;
    stream.write_all(request.as_bytes()).await.ok()?;
    let raw = read_response_bounded(&mut stream).await?;
    if !raw.starts_with("HTTP/") {
        return None;
    }
    let (_headers, body) = raw.split_once("\r\n\r\n")?;
    Some(body.to_string())
}

/// Pulled out of `probe_redfish` so the detection logic is unit-testable
/// against hardcoded response bodies without a live TLS server. `body` is
/// the response with headers already stripped.
fn parse_redfish_body(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let version = json.get("RedfishVersion").and_then(|v| v.as_str());
    let is_service_root = json
        .get("@odata.type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.contains("ServiceRoot"));
    if version.is_none() && !is_service_root {
        return None;
    }
    Some(match version {
        Some(v) => format!("Redfish {v}"),
        None => "Redfish".to_string(),
    })
}

/// Redfish (BMC REST management API) exposes an unauthenticated service
/// root at `/redfish/v1/` describing itself via `RedfishVersion` /
/// `@odata.type`. Distinct from a generic HTTPS probe (which would false-
/// positive on almost any web server), this only fires on that specific
/// signature. Tried over TLS since Redfish is conventionally HTTPS-only.
async fn probe_redfish(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let request = format!(
        "GET /redfish/v1/ HTTP/1.0\r\nHost: {ip}\r\nUser-Agent: CrossTerm-NetworkExplorer\r\nConnection: close\r\n\r\n"
    );
    let mut stream = tls_connect(ip, port, timeout).await?;
    stream.write_all(request.as_bytes()).await.ok()?;
    let raw = read_response_bounded(&mut stream).await?;
    if !raw.starts_with("HTTP/") {
        return None;
    }
    let (_headers, body) = raw.split_once("\r\n\r\n")?;
    parse_redfish_body(body)
}

/// Pulled out of `probe_webdav` so the detection logic is unit-testable
/// against a hardcoded raw response without a live server. `raw` is the
/// full response including headers.
fn is_webdav_response(raw: &str) -> bool {
    if !raw.starts_with("HTTP/") {
        return false;
    }
    let lower = raw.to_ascii_lowercase();
    let has_dav_header = lower.lines().any(|l| l.starts_with("dav:"));
    let allows_propfind = lower
        .lines()
        .find(|l| l.starts_with("allow:"))
        .is_some_and(|l| l.contains("propfind"));
    has_dav_header || allows_propfind
}

/// WebDAV is detected via an OPTIONS request: a WebDAV-capable server
/// advertises `PROPFIND`/`MKCOL`/etc. in its `Allow` header, or sends a
/// `DAV:` header directly. Both are checked since servers vary in which
/// one they actually send.
async fn probe_webdav(ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> Option<()> {
    let request = format!(
        "OPTIONS / HTTP/1.0\r\nHost: {ip}\r\nUser-Agent: CrossTerm-NetworkExplorer\r\nConnection: close\r\n\r\n"
    );
    let raw = if tls {
        let mut stream = tls_connect(ip, port, timeout).await?;
        stream.write_all(request.as_bytes()).await.ok()?;
        read_response_bounded(&mut stream).await?
    } else {
        let mut stream = connect_bound(ip, port, timeout).await.ok()?;
        stream.write_all(request.as_bytes()).await.ok()?;
        read_response_bounded(&mut stream).await?
    };
    if is_webdav_response(&raw) { Some(()) } else { None }
}

// ── UDP probes (SNMP, IPMI) ──────────────────────────────────────────────
// Unlike TCP, a UDP "connect" doesn't verify reachability — the only
// reliable way to tell a UDP port is actually open and speaking the
// expected protocol is to send a real protocol payload and see if we get
// a real protocol response back. A silent UDP port (many services don't
// respond to malformed input) reads the same as a closed one here; that's
// an inherent UDP-scanning limitation, not something these probes can fix.

/// Send `payload` to `ip:port` over UDP (bound to `BOUND_INTERFACE` like the
/// TCP probes) and return whatever comes back within `timeout`, if anything.
async fn probe_udp(ip: IpAddr, port: u16, payload: &[u8], timeout: Duration) -> Option<Vec<u8>> {
    let bound_if = current_bound_interface();
    let bind_addr = if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
    let socket = tokio::task::spawn_blocking({
        let bind_addr = bind_addr.to_string();
        move || -> std::io::Result<std::net::UdpSocket> {
            let domain = if bind_addr.starts_with('[') { socket2::Domain::IPV6 } else { socket2::Domain::IPV4 };
            let socket = socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
            if let Some(iface) = bound_if.as_deref() {
                bind_socket_to_interface(&socket, iface)?;
            }
            let addr: SocketAddr = bind_addr.parse().map_err(std::io::Error::other)?;
            socket.bind(&addr.into())?;
            socket.set_nonblocking(true)?;
            Ok(socket.into())
        }
    })
    .await
    .ok()?
    .ok()?;
    let socket = UdpSocket::from_std(socket).ok()?;

    let dest = SocketAddr::new(ip, port);
    socket.send_to(payload, dest).await.ok()?;

    let mut buf = [0u8; 512];
    let (n, _from) = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await.ok()?.ok()?;
    if n == 0 { None } else { Some(buf[..n].to_vec()) }
}

/// Minimal BER/DER TLV encoder — just enough to build the one fixed SNMPv1
/// GetRequest packet below, not a general ASN.1 encoder.
fn ber_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 128 {
        out.push(len as u8);
    } else {
        // Not needed for this fixed, short packet, but kept correct rather
        // than silently truncating if the shape ever changes.
        let len_bytes = len.to_be_bytes();
        let significant: Vec<u8> = len_bytes.iter().copied().skip_while(|&b| b == 0).collect();
        out.push(0x80 | significant.len() as u8);
        out.extend_from_slice(&significant);
    }
    out.extend_from_slice(content);
    out
}

/// Build an SNMPv1 GetRequest for sysDescr.0 (OID 1.3.6.1.2.1.1.1.0) with
/// the given community string.
fn build_snmp_get_request(community: &str) -> Vec<u8> {
    let oid = ber_tlv(0x06, &[0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x01, 0x00]); // 1.3.6.1.2.1.1.1.0
    let null = ber_tlv(0x05, &[]);
    let varbind = ber_tlv(0x30, &[oid, null].concat());
    let varbind_list = ber_tlv(0x30, &varbind);
    let request_id = ber_tlv(0x02, &[0x01]);
    let error_status = ber_tlv(0x02, &[0x00]);
    let error_index = ber_tlv(0x02, &[0x00]);
    let pdu_content = [request_id, error_status, error_index, varbind_list].concat();
    let get_request_pdu = ber_tlv(0xA0, &pdu_content); // [0] IMPLICIT SEQUENCE

    let version = ber_tlv(0x02, &[0x00]); // SNMPv1
    let community_str = ber_tlv(0x04, community.as_bytes());
    let message_content = [version, community_str, get_request_pdu].concat();
    ber_tlv(0x30, &message_content)
}

/// Probe SNMP (UDP 161) with a real GetRequest for sysDescr.0 against the
/// "public" community string. Any well-formed SNMP response (starts with a
/// BER SEQUENCE tag) confirms an SNMP agent is present, even if sysDescr
/// itself can't be parsed out (custom/locked-down agents, different
/// community string, etc.).
/// Pulled out of `probe_snmp` so response parsing is unit-testable against
/// hardcoded byte sequences without a live UDP responder.
fn parse_snmp_response(response: &[u8]) -> Option<String> {
    if response.first() != Some(&0x30) {
        return None;
    }
    // Best-effort: find an OCTET STRING (tag 0x04) in the response and treat
    // it as sysDescr if printable — not a full ASN.1 parse, just a scan for
    // the shape we expect back for this specific request.
    for i in 0..response.len().saturating_sub(1) {
        if response[i] == 0x04 {
            let len = response[i + 1] as usize;
            let start = i + 2;
            if let Some(bytes) = response.get(start..start + len) {
                if let Ok(text) = std::str::from_utf8(bytes) {
                    if !text.is_empty() && text.chars().all(|c| !c.is_control() || c == '\n') {
                        return Some(text.trim().to_string());
                    }
                }
            }
        }
    }
    Some("SNMP agent".to_string())
}

async fn probe_snmp(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let packet = build_snmp_get_request("public");
    let response = probe_udp(ip, port, &packet, timeout).await?;
    parse_snmp_response(&response)
}

/// Build an RMCP Presence Ping (ASF message type 0x80) — the standard way
/// to probe for an IPMI-over-LAN service on UDP 623 without authenticating.
fn build_rmcp_presence_ping() -> Vec<u8> {
    vec![
        0x06, 0x00, 0xFF, 0x06, // RMCP header: version 6, reserved, seq 0xFF (no ACK), class ASF
        0x00, 0x00, 0x11, 0xBE, // ASF IANA enterprise number (4542)
        0x80, // message type: Presence Ping
        0x00, // message tag
        0x00, // reserved
        0x00, // data length: 0
    ]
}

/// Probe IPMI SOL (UDP 623) via RMCP Presence Ping. A genuine BMC answers
/// with a Presence Pong (ASF message type 0x40) at the same offset.
/// Pulled out of `probe_ipmi` so the check is unit-testable directly.
fn is_rmcp_presence_pong(response: &[u8]) -> bool {
    response.len() >= 9 && response[3] == 0x06 && response[8] == 0x40
}

async fn probe_ipmi(ip: IpAddr, port: u16, timeout: Duration) -> Option<()> {
    let response = probe_udp(ip, port, &build_rmcp_presence_ping(), timeout).await?;
    if is_rmcp_presence_pong(&response) { Some(()) } else { None }
}

const JELLYFIN_PORT: u16 = 8096;
const PLEX_PORT: u16 = 32400;

/// Jellyfin's unauthenticated public-info endpoint — identifies the server
/// by its actual configured name (e.g. "nl.jellyfin"), not just "a media
/// server is here". Confirmed live: this is exactly what surfaced the
/// Jellyfin-hosting Proxmox VM by name during a real scan.
async fn probe_jellyfin(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let body = http_get_body(ip, port, "/System/Info/Public", timeout).await?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let name = json.get("ServerName")?.as_str()?;
    if name.is_empty() { None } else { Some(format!("Jellyfin: {name}")) }
}

/// Plex's unauthenticated identity endpoint — confirms a Plex Media Server
/// is present (via `machineIdentifier`) and, on installs that expose it
/// without auth, its configured friendly name.
async fn probe_plex(ip: IpAddr, port: u16, timeout: Duration) -> Option<String> {
    let body = http_get_body(ip, port, "/identity", timeout).await?;
    if !body.contains("machineIdentifier") {
        return None;
    }
    let name = body
        .split("friendlyName=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .filter(|n| !n.is_empty());
    match name {
        Some(n) => Some(format!("Plex: {n}")),
        None => Some("Plex Media Server".to_string()),
    }
}

/// Enrich a single open port with a banner/version/title/TLS-cert summary
/// where the protocol allows it cheaply. Best-effort: probe failures just
/// leave the enrichment fields as `None` — the port still shows as open.
async fn enrich_port(ip: IpAddr, mut open_port: OpenPort, timeout: Duration) -> OpenPort {
    let port = open_port.port;
    let probe_timeout = timeout.min(Duration::from_millis(2000));

    if port == 554 {
        open_port.version = probe_rtsp(ip, port, probe_timeout).await;
        return open_port;
    }

    if port == JELLYFIN_PORT {
        open_port.version = probe_jellyfin(ip, port, probe_timeout).await;
        return open_port;
    }

    if port == PLEX_PORT {
        open_port.version = probe_plex(ip, port, probe_timeout).await;
        return open_port;
    }

    if TLS_PROBE_PORTS.contains(&port) {
        if let Some(cert) = probe_tls_cert(ip, port, probe_timeout).await {
            open_port.version = cert.subject_org.clone().or_else(|| cert.subject_cn.clone());
            open_port.tls = Some(cert);
        }
        if let Some((server, title)) = probe_http(ip, port, true, probe_timeout).await {
            if server.is_some() {
                open_port.version = server;
            }
            open_port.http_title = title;
        }
        // Redfish/WebDAV both ride on the same HTTPS ports as any other web
        // server, so they're only claimed on an explicit protocol signature
        // (Redfish's ServiceRoot JSON, WebDAV's DAV/PROPFIND header) rather
        // than assumed from the port alone.
        if REDFISH_PROBE_PORTS.contains(&port) {
            if let Some(redfish) = probe_redfish(ip, port, probe_timeout).await {
                open_port.version = Some(redfish);
            } else if WEBDAV_PROBE_PORTS.contains(&port)
                && probe_webdav(ip, port, true, probe_timeout).await.is_some()
            {
                open_port.version = Some("WebDAV".to_string());
            }
        }
        return open_port;
    }

    if HTTP_PORTS.contains(&port) {
        if let Some((server, title)) = probe_http(ip, port, false, probe_timeout).await {
            open_port.version = server;
            open_port.http_title = title;
        }
        if WEBDAV_PROBE_PORTS.contains(&port) && probe_webdav(ip, port, false, probe_timeout).await.is_some() {
            open_port.version = Some("WebDAV".to_string());
        }
        return open_port;
    }

    if BANNER_FIRST_PORTS.contains(&port) {
        if let Some(banner) = grab_banner(ip, port, probe_timeout).await {
            open_port.version = Some(summarize_banner(&banner));
            open_port.banner = Some(banner);
        }
    }

    open_port
}

// ── mDNS / Bonjour discovery ────────────────────────────────────────────
// Service types curated from what actually proved useful identifying real
// LAN devices (smart-home hubs, cast/cameras/speakers, file shares, dev
// tooling) rather than an exhaustive registry dump.
const MDNS_SERVICE_TYPES: &[&str] = &[
    "_http._tcp.local.",
    "_https._tcp.local.",
    "_ssh._tcp.local.",
    "_sftp-ssh._tcp.local.",
    "_smb._tcp.local.",
    "_workstation._tcp.local.",
    "_airplay._tcp.local.",
    "_raop._tcp.local.",
    "_googlecast._tcp.local.",
    "_spotify-connect._tcp.local.",
    "_home-assistant._tcp.local.",
    "_matter._tcp.local.",
    "_hap._tcp.local.",
    "_ipp._tcp.local.",
    "_printer._tcp.local.",
    "_device-info._tcp.local.",
    "_companion-link._tcp.local.",
    "_mqtt._tcp.local.",
    "_esphomebuilder._tcp.local.",
    "_dcp._tcp.local.",
    "_rfb._tcp.local.",
    "_onvif._tcp.local.",
    "_rtsp._tcp.local.",
];

/// Browse the curated mDNS service-type list for `duration`, returning
/// discovered service instances keyed by every IPv4 address they resolved
/// to. Best-effort: any daemon/browse failure just yields an empty map.
async fn mdns_discover(duration: Duration) -> HashMap<Ipv4Addr, Vec<MdnsRecord>> {
    use futures::StreamExt;

    let mdns = match mdns_sd::ServiceDaemon::new() {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };

    let merged = futures::stream::select_all(MDNS_SERVICE_TYPES.iter().filter_map(|svc| {
        mdns.browse(svc).ok().map(|rx| {
            Box::pin(rx.into_stream())
                as std::pin::Pin<Box<dyn futures::Stream<Item = mdns_sd::ServiceEvent> + Send>>
        })
    }));
    tokio::pin!(merged);

    let mut records: HashMap<Ipv4Addr, Vec<MdnsRecord>> = HashMap::new();
    let _ = tokio::time::timeout(duration, async {
        while let Some(event) = merged.next().await {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                let service_type = info.ty_domain.clone();
                let instance_name = info
                    .fullname
                    .split(&service_type)
                    .next()
                    .unwrap_or(&info.fullname)
                    .trim_end_matches('.')
                    .to_string();
                let hostname = {
                    let h = info.host.trim_end_matches('.').to_string();
                    if h.is_empty() { None } else { Some(h) }
                };
                let txt: HashMap<String, String> = info
                    .txt_properties
                    .iter()
                    .map(|p| (p.key().to_string(), p.val_str().to_string()))
                    .collect();
                let addrs: Vec<Ipv4Addr> = info
                    .addresses
                    .iter()
                    .filter_map(|a| match a.to_ip_addr() {
                        IpAddr::V4(v4) => Some(v4),
                        IpAddr::V6(_) => None,
                    })
                    .collect();

                let record = MdnsRecord { service_type, instance_name, hostname, txt };
                for addr in addrs {
                    records.entry(addr).or_default().push(record.clone());
                }
            }
        }
    }).await;

    let _ = mdns.shutdown();
    records
}

/// Bundled IEEE OUI prefix -> vendor table (see `resources/oui-prefixes.txt`
/// for provenance/format), parsed once on first use.
static OUI_DATABASE: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();

fn oui_database() -> &'static HashMap<String, String> {
    OUI_DATABASE.get_or_init(|| {
        const RAW: &str = include_str!("../../resources/oui-prefixes.txt");
        let mut map = HashMap::new();
        for line in RAW.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some((prefix, vendor)) = line.split_once('\t') {
                map.insert(prefix.to_string(), vendor.to_string());
            }
        }
        map
    })
}

/// Look up a MAC address's vendor: the bundled offline IEEE OUI database
/// first (instant, no network, no rate limit, no leaking LAN device info to
/// a third party), falling back to the `api.macvendors.com` API only for
/// prefixes the bundled snapshot doesn't cover.
async fn lookup_mac_vendor(mac: &str) -> Option<String> {
    let clean: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() < 6 {
        return None;
    }
    let prefix = clean[0..6].to_ascii_uppercase();

    if let Some(vendor) = oui_database().get(&prefix) {
        return Some(vendor.clone());
    }

    lookup_mac_vendor_online(&prefix).await
}

/// Fallback online lookup for OUIs missing from the bundled snapshot.
async fn lookup_mac_vendor_online(prefix: &str) -> Option<String> {
    let oui = format!("{}-{}-{}", &prefix[0..2], &prefix[2..4], &prefix[4..6]);
    let url = format!("https://api.macvendors.com/{}", oui);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if resp.status().is_success() {
        let text = resp.text().await.ok()?;
        let text = text.trim().to_string();
        // API returns plain text on success; JSON on error (rate-limit etc.)
        if !text.is_empty() && !text.starts_with('{') {
            return Some(text);
        }
    }
    None
}

/// Guess OS from open ports heuristic.
/// OS-name literals we look for verbatim in any banner/version/title string
/// — the strongest signal, since it's the device naming itself.
const OS_NAME_HINTS: &[&str] = &[
    "Ubuntu", "Debian", "CentOS", "Fedora", "Red Hat", "FreeBSD", "OpenBSD",
    "OpenWrt", "Windows", "Android", "Raspbian",
];

/// Vendor keywords in a TLS cert's subject/issuer org — implies an embedded
/// Linux appliance of a recognizable kind, even with no OS string anywhere.
const VENDOR_HINTS: &[&str] = &[
    "XIAOMI", "D-LINK", "SAMSUNG", "ROBOROCK", "PROXMOX", "APPLE", "SONOS",
    "UBIQUITI", "SYNOLOGY", "TP-LINK", "NETGEAR", "AMAZON", "GOOGLE",
];

/// Guess the OS/device family from every signal gathered for a host, and
/// return the human-readable evidence trail alongside it (each check that
/// matched appends one short note explaining why).
fn guess_os(open_ports: &[OpenPort], ttl: Option<u8>) -> (Option<String>, Vec<String>) {
    let mut evidence = Vec::new();

    // 1. OS name found verbatim in a banner/version/title string.
    for port in open_ports {
        for text in [&port.banner, &port.version, &port.http_title].into_iter().flatten() {
            for hint in OS_NAME_HINTS {
                if text.contains(hint) {
                    evidence.push(format!("port {}: \"{}\"", port.port, text));
                    return (Some(hint.to_string()), evidence);
                }
            }
        }
    }

    // 2. Vendor keyword in a TLS certificate's subject/issuer org.
    for port in open_ports {
        if let Some(tls) = &port.tls {
            for org in [&tls.subject_org, &tls.issuer_org].into_iter().flatten() {
                let upper = org.to_ascii_uppercase();
                for hint in VENDOR_HINTS {
                    if upper.contains(hint) {
                        evidence.push(format!("port {} TLS cert org: {}", port.port, org));
                        return (Some(format!("Embedded Linux ({} device)", to_title_case(hint))), evidence);
                    }
                }
            }
        }
    }

    // 3. Port-based heuristic (unchanged from the original implementation).
    let ports: Vec<u16> = open_ports.iter().map(|p| p.port).collect();
    if ports.contains(&3389) {
        evidence.push("port 3389 (RDP) open".to_string());
        return (Some("Windows".to_string()), evidence);
    }
    if ports.contains(&22) {
        evidence.push("port 22 (SSH) open".to_string());
        return (Some("Linux/Unix".to_string()), evidence);
    }
    if ports.contains(&445) {
        evidence.push("port 445 (SMB) open, no SSH".to_string());
        return (Some("Windows".to_string()), evidence);
    }

    // 4. ICMP TTL as a last-resort fallback/corroboration.
    if let Some(ttl) = ttl {
        let guess = match ttl {
            0..=64 => Some("Linux/macOS/BSD-like (TTL 64)"),
            65..=128 => Some("Windows-like (TTL 128)"),
            _ => Some("Network device/embedded (TTL 255)"),
        };
        if let Some(g) = guess {
            evidence.push(format!("ping TTL {}", ttl));
            return (Some(g.to_string()), evidence);
        }
    }

    (None, evidence)
}

fn to_title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &c.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

// ── State ───────────────────────────────────────────────────────────────

pub struct NetworkState {
    pub scan_results: Mutex<HashMap<String, Vec<ScanResult>>>,
    pub tunnel_rules: Mutex<Vec<TunnelRule>>,
    pub active_tunnels: Mutex<HashMap<String, TunnelStatus>>,
    pub file_servers: Mutex<HashMap<String, FileServerInfo>>,
    /// Whether the user has accepted the aircrack-ng educational disclaimer
    pub aircrack_disclaimer_accepted: AtomicBool,
    /// Running aircrack-ng child processes keyed by operation ID
    pub aircrack_processes: Mutex<HashMap<String, AircrackProcess>>,
    /// Audit log of all aircrack-ng operations
    pub aircrack_audit_log: Mutex<Vec<AircrackAuditEntry>>,
    /// Interfaces currently in (pseudo-)monitor mode (tracked for macOS)
    #[allow(dead_code)]
    pub monitor_interfaces: Mutex<HashSet<String>>,
    /// Cancellation flags for in-progress `network_explore_start` scans, keyed by scan_id.
    pub explore_cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            scan_results: Mutex::new(HashMap::new()),
            tunnel_rules: Mutex::new(Vec::new()),
            active_tunnels: Mutex::new(HashMap::new()),
            file_servers: Mutex::new(HashMap::new()),
            aircrack_disclaimer_accepted: AtomicBool::new(false),
            aircrack_processes: Mutex::new(HashMap::new()),
            aircrack_audit_log: Mutex::new(Vec::new()),
            monitor_interfaces: Mutex::new(HashSet::new()),
            explore_cancel_flags: Mutex::new(HashMap::new()),
        }
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn network_scan_start(
    target: ScanTarget,
    state: tauri::State<'_, NetworkState>,
    app: AppHandle,
) -> Result<String, NetworkError> {
    let scan_id = Uuid::new_v4().to_string();
    let addresses = parse_cidr(&target.cidr)?;
    let total_hosts = addresses.len() as u32;

    // Initialize empty results for this scan
    {
        let mut results = state.scan_results.lock().unwrap();
        results.insert(scan_id.clone(), Vec::new());
    }

    let scan_id_clone = scan_id.clone();

    tokio::spawn(async move {
        const MAX_CONCURRENT: usize = 25;
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let hosts_scanned = Arc::new(AtomicU32::new(0));
        let timeout = Duration::from_millis(1500);
        let mut tasks = tokio::task::JoinSet::new();

        for addr in addresses {
            let ip = IpAddr::V4(addr);
            let app = app.clone();
            let scan_id = scan_id_clone.clone();
            let sem = Arc::clone(&semaphore);
            let counter = Arc::clone(&hosts_scanned);

            tasks.spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let start = Instant::now();

                let port_futures: Vec<_> = DEFAULT_PORTS.iter().map(|&p| check_port(ip, p, timeout)).collect();
                let port_results = futures::future::join_all(port_futures).await;
                let open_ports: Vec<OpenPort> = port_results.into_iter().flatten().collect();
                let response_time = start.elapsed().as_secs_f64() * 1000.0;

                if !open_ports.is_empty() {
                    // Legacy path: no per-scan DNS-server discovery (that
                    // needs to grow across concurrent hosts, which this
                    // command's simpler per-host task shape doesn't share) —
                    // an empty, un-seeded registry just skips method 9.
                    let no_dns_servers = new_dns_server_registry();
                    let (hostname, mac_address) = tokio::join!(resolve_hostname_aggressive(ip, open_ports.clone(), &no_dns_servers), resolve_arp_mac(ip));
                    let mac_vendor = if let Some(ref mac) = mac_address {
                        lookup_mac_vendor(mac).await
                    } else {
                        None
                    };
                    let os_guess = guess_os(&open_ports, None).0;
                    let result = ScanResult {
                        ip: ip.to_string(),
                        hostname,
                        mac_address,
                        mac_vendor,
                        open_ports,
                        os_guess,
                        response_time_ms: response_time,
                    };
                    let _ = app.emit("network:scan_host_found", ScanHostFound {
                        scan_id: scan_id.clone(),
                        result,
                    });
                }

                let scanned = counter.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit("network:scan_progress", ScanProgress {
                    scan_id,
                    hosts_scanned: scanned,
                    total_hosts,
                });
            });
        }

        while tasks.join_next().await.is_some() {}
    });

    Ok(scan_id)
}

/// Default service filters when the user doesn't specify any.
const DEFAULT_EXPLORE_SERVICES: &[ServiceFilter] = &[
    ServiceFilter::Ssh,
    ServiceFilter::Rdp,
    ServiceFilter::Vnc,
    ServiceFilter::Http,
    ServiceFilter::Https,
    ServiceFilter::Telnet,
    ServiceFilter::Ftp,
    ServiceFilter::Smb,
    ServiceFilter::WinRm,
    ServiceFilter::WinRmTls,
    ServiceFilter::Mqtt,
    ServiceFilter::Netconf,
    ServiceFilter::Grpc,
    ServiceFilter::KubeApi,
    ServiceFilter::DockerApi,
    ServiceFilter::WsTerminal,
    ServiceFilter::Rtsp,
];

/// How long the concurrent mDNS/Bonjour browse runs for each explore scan.
/// A typical CIDR sweep at `MAX_CONCURRENT = 25` takes comfortably longer
/// than this, so it costs nothing extra in wall-clock time.
const MDNS_DISCOVER_WINDOW: Duration = Duration::from_secs(4);

#[tauri::command]
pub async fn network_explore_start(
    target: ExploreTarget,
    state: tauri::State<'_, NetworkState>,
    app: AppHandle,
) -> Result<String, NetworkError> {
    let scan_id = Uuid::new_v4().to_string();
    let addresses = parse_cidr(&target.cidr)?;
    let total_hosts = addresses.len() as u32;
    let timeout = Duration::from_millis(target.timeout_ms.unwrap_or(1500));

    // Build the deduplicated port list from service filters + extra ports
    let services = if target.services.is_empty() {
        DEFAULT_EXPLORE_SERVICES.to_vec()
    } else {
        target.services.clone()
    };
    let mut ports: Vec<u16> = services.iter().map(|s| s.port()).collect();
    ports.extend(&target.extra_ports);
    // Always probed regardless of service filter selection: 53 feeds the
    // local-DNS-server discovery behind hostname method 9 (see
    // `register_if_dns_server`) — without it, no host's port 53 ever gets
    // checked, so the registry could only ever hold the gateway/.1 guesses.
    // 8096/32400 are Jellyfin/Plex's default ports — cheap to always check
    // and the only way to name the media-server VMs this pass was built for.
    ports.push(53);
    ports.push(JELLYFIN_PORT);
    ports.push(PLEX_PORT);
    ports.sort_unstable();
    ports.dedup();

    // Validate port range
    if ports.contains(&0) {
        return Err(NetworkError::InvalidCidr("Port 0 is not valid".to_string()));
    }

    let scan_id_clone = scan_id.clone();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state.explore_cancel_flags.lock().unwrap().insert(scan_id.clone(), Arc::clone(&cancel_flag));

    // See the `BOUND_INTERFACE` note above `connect_bound`: pin probe
    // traffic to whichever local interface actually owns this subnet, so a
    // VPN's competing route for the same prefix can't steal it and starve
    // ARP resolution.
    let bound_if = interface_for_cidr(&target.cidr);

    tokio::spawn(async move {
        const MAX_CONCURRENT: usize = 25;
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let hosts_scanned = Arc::new(AtomicU32::new(0));
        let hosts_found = Arc::new(AtomicU32::new(0));
        let ports = Arc::new(ports);
        let mut tasks = tokio::task::JoinSet::new();

        // mDNS/Bonjour discovery runs concurrently with the per-host CIDR
        // sweep below; its results arrive later via a separate event since
        // by-IP results may already have been emitted before it completes.
        let mdns_app = app.clone();
        let mdns_scan_id = scan_id_clone.clone();
        let mdns_task = tokio::spawn(async move {
            let records = mdns_discover(MDNS_DISCOVER_WINDOW).await;
            if !records.is_empty() {
                let by_ip: HashMap<String, Vec<MdnsRecord>> =
                    records.into_iter().map(|(ip, recs)| (ip.to_string(), recs)).collect();
                let _ = mdns_app.emit("network:explore_mdns_update", ExploreMdnsUpdate {
                    scan_id: mdns_scan_id,
                    records: by_ip,
                });
            }
        });

        // See the note above `DnsServerRegistry`: seeded once per scan, then
        // grown as hosts with port 53 open are found during it.
        let dns_registry = seed_dns_server_registry(&target.cidr).await;

        for addr in addresses {
            let ip = IpAddr::V4(addr);
            let app = app.clone();
            let scan_id = scan_id_clone.clone();
            let sem = Arc::clone(&semaphore);
            let counter = Arc::clone(&hosts_scanned);
            let found_counter = Arc::clone(&hosts_found);
            let ports = Arc::clone(&ports);
            let cancelled = Arc::clone(&cancel_flag);
            let bound_if = bound_if.clone();
            let dns_registry = Arc::clone(&dns_registry);

            tasks.spawn(BOUND_INTERFACE.scope(bound_if, async move {
                let _permit = sem.acquire_owned().await.unwrap();
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }
                let start = Instant::now();

                // Run TCP port checks, ICMP ping, and the UDP protocol
                // probes (SNMP/IPMI — UDP can't be "connect scanned" like
                // TCP, so these send a real protocol payload and check for a
                // real protocol response) all concurrently. The UDP probes
                // use their own, much shorter timeout: unlike a closed TCP
                // port (which fails fast with a real RST), a UDP port with
                // no SNMP/IPMI agent listening never sends anything back at
                // all, so these two probes hit their full timeout on nearly
                // every host that doesn't have one — reusing the ~1.5s TCP/
                // ping timeout here made every single host's scan take that
                // much longer at minimum, regardless of how fast the real
                // TCP/ping checks resolved. A real reply, when a device does
                // answer, arrives in low milliseconds on a LAN.
                let udp_timeout = Duration::from_millis(300).min(timeout);
                let port_futures: Vec<_> = ports.iter().map(|&p| check_port(ip, p, timeout)).collect();
                let (port_results, (ping_alive, ttl), snmp_result, ipmi_result) = tokio::join!(
                    futures::future::join_all(port_futures),
                    ping_host(ip, timeout),
                    probe_snmp(ip, 161, udp_timeout),
                    probe_ipmi(ip, 623, udp_timeout),
                );
                let mut open_ports: Vec<OpenPort> = port_results.into_iter().flatten().collect();
                if let Some(sys_descr) = snmp_result {
                    open_ports.push(OpenPort {
                        port: 161,
                        service_name: "snmp".to_string(),
                        protocol: "udp".to_string(),
                        version: Some(sys_descr),
                        ..Default::default()
                    });
                }
                if ipmi_result.is_some() {
                    open_ports.push(OpenPort {
                        port: 623,
                        service_name: "ipmi".to_string(),
                        protocol: "udp".to_string(),
                        ..Default::default()
                    });
                }
                let found = !open_ports.is_empty() || ping_alive;
                register_if_dns_server(ip, &open_ports, &dns_registry).await;

                // Progress/"scanned" advances here, at the fast port-scan/ping
                // pass, not after the slower enrichment below — so the
                // progress bar (and the Stop button's lifetime) reflects
                // actual scan coverage instead of stalling on the tail
                // latency of hostname/ARP/banner resolution for whichever
                // hosts happen to be slowest. Enrichment keeps streaming
                // in afterward via `explore_host_enriched`, same pattern as
                // the mDNS merge-by-IP update already uses.
                if found {
                    found_counter.fetch_add(1, Ordering::Relaxed);
                }
                let scanned = counter.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit("network:explore_progress", ExploreProgress {
                    scan_id: scan_id.clone(),
                    hosts_scanned: scanned,
                    total_hosts,
                    hosts_found: found_counter.load(Ordering::Relaxed),
                });

                if !found {
                    return;
                }

                // Phase 1: surface the host immediately with just what the
                // port scan/ping already know — real ports, a TTL-only OS
                // guess, a ports-only suggested session type. No banner/TLS/
                // hostname/MAC/vendor yet; those come from phase 2 below.
                let (early_os_guess, early_evidence) = guess_os(&open_ports, ttl);
                let suggested_session_type = suggest_session_type(&open_ports);
                let early_result = ExploreResult {
                    ip: ip.to_string(),
                    hostname: None,
                    mac_address: None,
                    mac_vendor: None,
                    open_ports: open_ports.clone(),
                    os_guess: early_os_guess,
                    response_time_ms: start.elapsed().as_secs_f64() * 1000.0,
                    suggested_session_type: suggested_session_type.clone(),
                    ttl,
                    mdns: Vec::new(),
                    evidence: early_evidence,
                };
                let _ = app.emit("network:explore_host_found", ExploreHostFound {
                    scan_id: scan_id.clone(),
                    result: early_result,
                });

                // Phase 2: banner/version/TLS/RTSP enrichment, 9-method
                // hostname resolution, ARP/OUI vendor lookup — the slower
                // probes — then push the completed picture as an update.
                let enrich_futures: Vec<_> = open_ports
                    .into_iter()
                    .map(|p| enrich_port(ip, p, timeout))
                    .collect();
                let open_ports: Vec<OpenPort> = futures::future::join_all(enrich_futures).await;
                let response_time = start.elapsed().as_secs_f64() * 1000.0;

                let (hostname, mac_address) = tokio::join!(resolve_hostname_aggressive(ip, open_ports.clone(), &dns_registry), resolve_arp_mac(ip));
                let mac_vendor = if let Some(ref mac) = mac_address {
                    lookup_mac_vendor(mac).await
                } else {
                    None
                };
                let (os_guess, evidence) = guess_os(&open_ports, ttl);
                // Redfish/WebDAV can only be told apart from a generic web
                // server by the enrichment probe above (their signature
                // check needs an actual HTTP round trip, not just a port
                // number) — refine the port-only suggestion from phase 1
                // now that that's available, rather than never suggesting
                // them at all.
                let suggested_session_type = refine_suggested_type(suggested_session_type, &open_ports);
                let result = ExploreResult {
                    ip: ip.to_string(),
                    hostname,
                    mac_address,
                    mac_vendor,
                    open_ports,
                    os_guess,
                    response_time_ms: response_time,
                    suggested_session_type,
                    ttl,
                    mdns: Vec::new(),
                    evidence,
                };
                let _ = app.emit("network:explore_host_enriched", ExploreHostFound {
                    scan_id,
                    result,
                });
            }));
        }

        while tasks.join_next().await.is_some() {}
        let _ = mdns_task.await;
    });

    Ok(scan_id)
}

/// Cancel an in-progress `network_explore_start` scan. Hosts not yet started
/// are skipped; hosts already mid-probe finish their current step.
#[tauri::command]
pub fn network_explore_stop(scan_id: String, state: tauri::State<'_, NetworkState>) {
    if let Some(flag) = state.explore_cancel_flags.lock().unwrap().get(&scan_id) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// JSON shape written by [`run_explore_and_dump`].
#[derive(Serialize)]
pub struct ExploreDump {
    pub cidr: String,
    pub bound_interface: Option<String>,
    /// Every local DNS server used for hostname method 9, by the end of the
    /// scan — the default gateway and the CIDR's `.1` guess if either
    /// answered, plus any host discovered along the way with port 53 open.
    pub dns_servers_used: Vec<IpAddr>,
    pub ports_scanned: Vec<u16>,
    pub host_count: usize,
    pub results: Vec<ExploreResult>,
    /// mDNS records whose advertised address never answered a scanned port
    /// or ping — useful for spotting devices the port scan alone misses.
    pub unmerged_mdns: HashMap<String, Vec<MdnsRecord>>,
}

/// Runs the exact same discovery/enrichment pipeline as `network_explore_start`
/// (interface-bound probes, port scan, banner/TLS/RTSP enrichment, 9-method
/// hostname resolution, ARP/OUI vendor lookup, OS guess, mDNS/Bonjour merge)
/// but with no Tauri `AppHandle`/`State` and no event stream — it runs to
/// completion and writes one JSON file. This is the mechanism behind the
/// `network-explore-cli` binary: it lets the real scanner run standalone,
/// without launching the app, unlocking the vault, or going through any UI,
/// which is what makes it fast to use for debugging scan/enrichment gaps.
pub async fn run_explore_and_dump(
    cidr: &str,
    services: Option<&[ServiceFilter]>,
    extra_ports: &[u16],
    timeout_ms: u64,
    out_path: &str,
) -> Result<usize, NetworkError> {
    let addresses = parse_cidr(cidr)?;
    let timeout = Duration::from_millis(timeout_ms);
    let bound_if = interface_for_cidr(cidr);
    let dns_registry = seed_dns_server_registry(cidr).await;

    let mut ports: Vec<u16> = services
        .unwrap_or(DEFAULT_EXPLORE_SERVICES)
        .iter()
        .map(|s| s.port())
        .collect();
    ports.extend(extra_ports);
    ports.push(53); // feeds local-DNS-server discovery; see register_if_dns_server
    ports.push(JELLYFIN_PORT);
    ports.push(PLEX_PORT);
    ports.sort_unstable();
    ports.dedup();
    let ports = Arc::new(ports);

    let semaphore = Arc::new(Semaphore::new(25));
    let host_futs = addresses.into_iter().map(|addr| {
        let ip = IpAddr::V4(addr);
        let ports = Arc::clone(&ports);
        let sem = Arc::clone(&semaphore);
        let dns_registry = Arc::clone(&dns_registry);
        async move {
            let _permit = sem.acquire_owned().await.unwrap();
            let port_futures: Vec<_> = ports.iter().map(|&p| check_port(ip, p, timeout)).collect();
            let (port_results, (ping_alive, ttl)) = tokio::join!(
                futures::future::join_all(port_futures),
                ping_host(ip, timeout),
            );
            let open_ports: Vec<OpenPort> = port_results.into_iter().flatten().collect();
            if open_ports.is_empty() && !ping_alive {
                return None;
            }
            register_if_dns_server(ip, &open_ports, &dns_registry).await;
            let enrich_futures: Vec<_> = open_ports.into_iter().map(|p| enrich_port(ip, p, timeout)).collect();
            let open_ports: Vec<OpenPort> = futures::future::join_all(enrich_futures).await;
            let (hostname, mac_address) = tokio::join!(
                resolve_hostname_aggressive(ip, open_ports.clone(), &dns_registry),
                resolve_arp_mac(ip),
            );
            let mac_vendor = if let Some(ref mac) = mac_address { lookup_mac_vendor(mac).await } else { None };
            let (os_guess, evidence) = guess_os(&open_ports, ttl);
            let suggested_session_type = suggest_session_type(&open_ports);
            Some(ExploreResult {
                ip: ip.to_string(),
                hostname,
                mac_address,
                mac_vendor,
                open_ports,
                os_guess,
                response_time_ms: 0.0,
                suggested_session_type,
                ttl,
                mdns: Vec::new(),
                evidence,
            })
        }
    });

    let (mdns_records, host_results) = tokio::join!(
        mdns_discover(MDNS_DISCOVER_WINDOW),
        BOUND_INTERFACE.scope(bound_if.clone(), futures::future::join_all(host_futs)),
    );
    let mdns_by_ip: HashMap<String, Vec<MdnsRecord>> =
        mdns_records.into_iter().map(|(ip, recs)| (ip.to_string(), recs)).collect();

    let mut results: Vec<ExploreResult> = host_results.into_iter().flatten().collect();
    for r in &mut results {
        if let Some(recs) = mdns_by_ip.get(&r.ip) {
            r.mdns = recs.clone();
            if r.hostname.is_none() {
                r.hostname = derive_mdns_hostname(recs);
            }
        }
    }
    results.sort_by_key(|r| r.ip.split('.').filter_map(|o| o.parse::<u32>().ok()).fold(0u32, |acc, o| acc * 256 + o));

    let merged_ips: HashSet<&str> = results.iter().map(|r| r.ip.as_str()).collect();
    let unmerged_mdns: HashMap<String, Vec<MdnsRecord>> = mdns_by_ip
        .into_iter()
        .filter(|(ip, _)| !merged_ips.contains(ip.as_str()))
        .collect();

    let host_count = results.len();
    let dns_servers_used: Vec<IpAddr> = dns_registry.read().await.iter().copied().collect();
    let dump = ExploreDump {
        cidr: cidr.to_string(),
        bound_interface: bound_if,
        dns_servers_used,
        ports_scanned: (*ports).clone(),
        host_count,
        results,
        unmerged_mdns,
    };
    let json = serde_json::to_string_pretty(&dump)
        .map_err(|e| NetworkError::Io(format!("JSON serialization failed: {e}")))?;
    std::fs::write(out_path, json).map_err(|e| NetworkError::Io(format!("failed to write {out_path}: {e}")))?;
    Ok(host_count)
}

#[tauri::command]
pub async fn network_scan_results(
    scan_id: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<ScanResult>, NetworkError> {
    let results = state.scan_results.lock().unwrap();
    results
        .get(&scan_id)
        .cloned()
        .ok_or(NetworkError::ScanNotFound(scan_id))
}

#[tauri::command]
pub async fn network_scan_save_as_sessions(
    scan_id: String,
    _folder: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<String>, NetworkError> {
    let results = state.scan_results.lock().unwrap();
    let scan_results = results
        .get(&scan_id)
        .ok_or_else(|| NetworkError::ScanNotFound(scan_id.clone()))?;

    let mut session_ids = Vec::new();
    for _result in scan_results {
        let session_id = Uuid::new_v4().to_string();
        // In a full implementation, this would create actual sessions via the session store.
        // For now, return the generated IDs.
        session_ids.push(session_id);
    }

    Ok(session_ids)
}

#[tauri::command]
pub async fn network_wol_send(target: WolTarget) -> Result<(), NetworkError> {
    let mac_bytes = parse_mac(&target.mac_address)?;
    let packet = build_wol_packet(&mac_bytes);

    let broadcast_addr = target
        .broadcast_ip
        .unwrap_or_else(|| "255.255.255.255".to_string());

    let dest: SocketAddr = format!("{broadcast_addr}:9")
        .parse()
        .map_err(|e: std::net::AddrParseError| NetworkError::Io(e.to_string()))?;

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| NetworkError::Io(e.to_string()))?;
    socket
        .set_broadcast(true)
        .map_err(|e| NetworkError::Io(e.to_string()))?;
    socket
        .send_to(&packet, dest)
        .await
        .map_err(|e| NetworkError::Io(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn network_tunnel_create(
    rule: TunnelRule,
    state: tauri::State<'_, NetworkState>,
    app: AppHandle,
) -> Result<String, NetworkError> {
    let id = if rule.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        rule.id.clone()
    };

    let new_rule = TunnelRule { id: id.clone(), ..rule };

    {
        let mut rules = state.tunnel_rules.lock().unwrap();
        rules.push(new_rule);
    }

    {
        let mut active = state.active_tunnels.lock().unwrap();
        active.insert(id.clone(), TunnelStatus::Inactive);
    }

    let _ = app.emit(
        "network:tunnel_status",
        TunnelStatusEvent {
            rule_id: id.clone(),
            status: TunnelStatus::Inactive,
        },
    );

    Ok(id)
}

#[tauri::command]
pub async fn network_tunnel_remove(
    rule_id: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<(), NetworkError> {
    {
        let mut rules = state.tunnel_rules.lock().unwrap();
        let len_before = rules.len();
        rules.retain(|r| r.id != rule_id);
        if rules.len() == len_before {
            return Err(NetworkError::TunnelNotFound(rule_id));
        }
    }

    {
        let mut active = state.active_tunnels.lock().unwrap();
        active.remove(&rule_id);
    }

    Ok(())
}

#[tauri::command]
pub async fn network_tunnel_list(
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<(TunnelRule, TunnelStatus)>, NetworkError> {
    let rules = state.tunnel_rules.lock().unwrap();
    let active = state.active_tunnels.lock().unwrap();

    let result: Vec<(TunnelRule, TunnelStatus)> = rules
        .iter()
        .map(|rule| {
            let status = active
                .get(&rule.id)
                .cloned()
                .unwrap_or(TunnelStatus::Inactive);
            (rule.clone(), status)
        })
        .collect();

    Ok(result)
}

#[tauri::command]
pub async fn network_tunnel_toggle(
    rule_id: String,
    enabled: bool,
    state: tauri::State<'_, NetworkState>,
    app: AppHandle,
) -> Result<(), NetworkError> {
    {
        let mut rules = state.tunnel_rules.lock().unwrap();
        let rule = rules
            .iter_mut()
            .find(|r| r.id == rule_id)
            .ok_or_else(|| NetworkError::TunnelNotFound(rule_id.clone()))?;
        rule.enabled = enabled;
    }

    let new_status = if enabled {
        TunnelStatus::Active
    } else {
        TunnelStatus::Inactive
    };

    {
        let mut active = state.active_tunnels.lock().unwrap();
        active.insert(rule_id.clone(), new_status.clone());
    }

    let _ = app.emit(
        "network:tunnel_status",
        TunnelStatusEvent {
            rule_id,
            status: new_status,
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn network_fileserver_start(
    config: FileServerConfig,
    state: tauri::State<'_, NetworkState>,
) -> Result<FileServerInfo, NetworkError> {
    let id = Uuid::new_v4().to_string();

    let url = match config.server_type {
        FileServerType::Http => format!("http://0.0.0.0:{}", config.port),
        FileServerType::Tftp => format!("tftp://0.0.0.0:{}", config.port),
    };

    let info = FileServerInfo {
        id: id.clone(),
        directory: config.directory.clone(),
        port: config.port,
        server_type: config.server_type.clone(),
        running: true,
        url,
    };

    // In a full implementation, this would spawn actual HTTP/TFTP server tasks.
    // For now, we register the server in state.
    {
        let mut servers = state.file_servers.lock().unwrap();
        servers.insert(id, info.clone());
    }

    Ok(info)
}

#[tauri::command]
pub async fn network_fileserver_stop(
    server_id: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<(), NetworkError> {
    let mut servers = state.file_servers.lock().unwrap();
    let server = servers
        .get_mut(&server_id)
        .ok_or_else(|| NetworkError::ServerNotFound(server_id.clone()))?;

    server.running = false;
    servers.remove(&server_id);
    Ok(())
}

#[tauri::command]
pub async fn network_fileserver_list(
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<FileServerInfo>, NetworkError> {
    let servers = state.file_servers.lock().unwrap();
    Ok(servers.values().cloned().collect())
}

// ── WiFi Scan Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WifiBand {
    #[serde(rename = "2.4GHz")]
    Band2_4GHz,
    #[serde(rename = "5GHz")]
    Band5GHz,
    #[serde(rename = "6GHz")]
    Band6GHz,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WifiSecurity {
    Open,
    Wep,
    WpaPsk,
    Wpa2Psk,
    Wpa3Sae,
    Wpa3Transition,
    Wpa2Enterprise,
    Wpa3Enterprise,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: Option<String>,
    pub channel: u32,
    pub channel_width_mhz: Option<u32>,
    pub band: WifiBand,
    pub frequency_mhz: Option<u32>,
    pub signal_dbm: Option<i32>,
    pub noise_dbm: Option<i32>,
    pub security: WifiSecurity,
    pub phy_mode: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiSecurityIssue {
    pub ssid: String,
    pub severity: String,
    pub issue: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiChannelCongestion {
    pub channel: u32,
    pub band: WifiBand,
    pub network_count: u32,
    pub strongest_signal_dbm: Option<i32>,
    pub weakest_signal_dbm: Option<i32>,
    pub congestion_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiScanResult {
    pub networks: Vec<WifiNetwork>,
    pub security_issues: Vec<WifiSecurityIssue>,
    pub channel_congestion: Vec<WifiChannelCongestion>,
    pub recommended_channels_2g: Vec<u32>,
    pub recommended_channels_5g: Vec<u32>,
    pub current_network: Option<WifiNetwork>,
    pub interface_name: Option<String>,
    pub scan_timestamp: String,
}

// ── WiFi Scan Helpers ───────────────────────────────────────────────────

fn parse_band_from_channel(channel: u32, freq_hint: Option<&str>) -> WifiBand {
    if let Some(hint) = freq_hint {
        let h = hint.to_lowercase();
        if h.contains("6ghz") || h.contains("6 ghz") {
            return WifiBand::Band6GHz;
        }
        if h.contains("5ghz") || h.contains("5 ghz") {
            return WifiBand::Band5GHz;
        }
        if h.contains("2ghz") || h.contains("2.4ghz") || h.contains("2 ghz") {
            return WifiBand::Band2_4GHz;
        }
    }
    match channel {
        1..=14 => WifiBand::Band2_4GHz,
        32..=177 => WifiBand::Band5GHz,
        _ => WifiBand::Unknown,
    }
}

fn parse_macos_security(raw: &str) -> WifiSecurity {
    let r = raw.to_lowercase();
    if r.contains("wpa3_enterprise") {
        WifiSecurity::Wpa3Enterprise
    } else if r.contains("wpa3_transition") {
        WifiSecurity::Wpa3Transition
    } else if r.contains("wpa3") || r.contains("sae") {
        WifiSecurity::Wpa3Sae
    } else if r.contains("wpa2_enterprise") || r.contains("wpa2_802.1x") {
        WifiSecurity::Wpa2Enterprise
    } else if r.contains("wpa2") {
        WifiSecurity::Wpa2Psk
    } else if r.contains("wpa_personal") || r.contains("wpa ") {
        WifiSecurity::WpaPsk
    } else if r.contains("wep") {
        WifiSecurity::Wep
    } else if r.contains("none") || r.is_empty() {
        WifiSecurity::Open
    } else {
        WifiSecurity::Unknown(raw.to_string())
    }
}

#[allow(dead_code)]
fn parse_signal_noise(sn: &str) -> (Option<i32>, Option<i32>) {
    // Format: "-65 dBm / -92 dBm"
    let parts: Vec<&str> = sn.split('/').collect();
    let signal = parts.first().and_then(|s| {
        s.trim().replace("dBm", "").trim().parse::<i32>().ok()
    });
    let noise = parts.get(1).and_then(|s| {
        s.trim().replace("dBm", "").trim().parse::<i32>().ok()
    });
    (signal, noise)
}

fn parse_channel_info(raw: &str) -> (u32, Option<u32>, Option<&str>) {
    // Format: "36 (5GHz, 80MHz)" or "6 (2GHz, 20MHz)" or just "36"
    let channel: u32 = raw.split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let width = if raw.contains("160MHz") {
        Some(160)
    } else if raw.contains("80MHz") {
        Some(80)
    } else if raw.contains("40MHz") {
        Some(40)
    } else if raw.contains("20MHz") {
        Some(20)
    } else {
        None
    };
    let freq_hint = if raw.contains('(') {
        raw.find('(').and_then(|i| raw.get(i..))
    } else {
        None
    };
    (channel, width, freq_hint)
}

fn assess_security(networks: &[WifiNetwork]) -> Vec<WifiSecurityIssue> {
    let mut issues = Vec::new();
    for net in networks {
        match &net.security {
            WifiSecurity::Open => {
                issues.push(WifiSecurityIssue {
                    ssid: net.ssid.clone(),
                    severity: "critical".into(),
                    issue: "Network has no encryption — all traffic is visible to anyone nearby".into(),
                    recommendation: "Enable WPA3 or at minimum WPA2 encryption immediately".into(),
                });
            }
            WifiSecurity::Wep => {
                issues.push(WifiSecurityIssue {
                    ssid: net.ssid.clone(),
                    severity: "critical".into(),
                    issue: "WEP encryption is broken and can be cracked in minutes".into(),
                    recommendation: "Upgrade to WPA3-SAE or WPA2-AES. Replace the router if it only supports WEP".into(),
                });
            }
            WifiSecurity::WpaPsk => {
                issues.push(WifiSecurityIssue {
                    ssid: net.ssid.clone(),
                    severity: "high".into(),
                    issue: "WPA (TKIP) has known vulnerabilities and is deprecated".into(),
                    recommendation: "Upgrade to WPA2-AES or WPA3-SAE".into(),
                });
            }
            // WPA2 is acceptable but WPA3 is better; only flag when it's the current network
            WifiSecurity::Wpa2Psk if net.is_current => {
                issues.push(WifiSecurityIssue {
                    ssid: net.ssid.clone(),
                    severity: "info".into(),
                    issue: "WPA2-PSK is secure but WPA3-SAE offers stronger protection".into(),
                    recommendation: "Consider upgrading router firmware to enable WPA3 transition mode".into(),
                });
            }
            WifiSecurity::Unknown(raw) if !raw.is_empty() => {
                issues.push(WifiSecurityIssue {
                    ssid: net.ssid.clone(),
                    severity: "warning".into(),
                    issue: format!("Unrecognized security protocol: {}", raw),
                    recommendation: "Verify the security configuration of this network".into(),
                });
            }
            _ => {}
        }
        // Hidden SSID check
        if net.ssid.is_empty() || net.ssid.chars().all(|c| c == '\0') {
            issues.push(WifiSecurityIssue {
                ssid: "(hidden)".into(),
                severity: "info".into(),
                issue: "Hidden SSID provides no real security — the network name is still detectable in probe requests".into(),
                recommendation: "Rely on strong WPA3 encryption instead of SSID hiding".into(),
            });
        }
        // Weak signal on your own network
        if net.is_current {
            if let Some(sig) = net.signal_dbm {
                if sig < -80 {
                    issues.push(WifiSecurityIssue {
                        ssid: net.ssid.clone(),
                        severity: "warning".into(),
                        issue: format!("Very weak signal ({} dBm) — a potential dead spot", sig),
                        recommendation: "Consider adding a mesh node or repeater near this location".into(),
                    });
                } else if sig < -70 {
                    issues.push(WifiSecurityIssue {
                        ssid: net.ssid.clone(),
                        severity: "info".into(),
                        issue: format!("Moderate signal ({} dBm) — may experience intermittent performance", sig),
                        recommendation: "Move closer to the access point or reduce obstructions".into(),
                    });
                }
            }
        }
    }
    issues
}

fn compute_channel_congestion(networks: &[WifiNetwork]) -> (Vec<WifiChannelCongestion>, Vec<u32>, Vec<u32>) {
    let mut chan_map: HashMap<(u32, String), Vec<Option<i32>>> = HashMap::new();
    for net in networks {
        let band_key = match &net.band {
            WifiBand::Band2_4GHz => "2.4".to_string(),
            WifiBand::Band5GHz => "5".to_string(),
            WifiBand::Band6GHz => "6".to_string(),
            WifiBand::Unknown => "?".to_string(),
        };
        chan_map.entry((net.channel, band_key)).or_default().push(net.signal_dbm);
    }

    let mut congestion = Vec::new();
    for ((channel, band_str), signals) in &chan_map {
        let band = match band_str.as_str() {
            "2.4" => WifiBand::Band2_4GHz,
            "5" => WifiBand::Band5GHz,
            "6" => WifiBand::Band6GHz,
            _ => WifiBand::Unknown,
        };
        let count = signals.len() as u32;
        let strongest = signals.iter().filter_map(|s| *s).max();
        let weakest = signals.iter().filter_map(|s| *s).min();
        let level = match count {
            0..=1 => "low",
            2..=3 => "medium",
            _ => "high",
        };
        congestion.push(WifiChannelCongestion {
            channel: *channel,
            band,
            network_count: count,
            strongest_signal_dbm: strongest,
            weakest_signal_dbm: weakest,
            congestion_level: level.to_string(),
        });
    }
    congestion.sort_by_key(|c| c.channel);

    // Recommend least-congested non-overlapping channels
    let channels_2g = [1u32, 6, 11];
    let mut rec_2g: Vec<(u32, u32)> = channels_2g.iter().map(|&ch| {
        let count = chan_map.get(&(ch, "2.4".to_string())).map(|v| v.len() as u32).unwrap_or(0);
        (ch, count)
    }).collect();
    rec_2g.sort_by_key(|&(_, c)| c);
    let recommended_2g: Vec<u32> = rec_2g.iter().map(|&(ch, _)| ch).collect();

    let channels_5g = [36u32, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 149, 153, 157, 161, 165];
    let mut rec_5g: Vec<(u32, u32)> = channels_5g.iter().map(|&ch| {
        let count = chan_map.get(&(ch, "5".to_string())).map(|v| v.len() as u32).unwrap_or(0);
        (ch, count)
    }).collect();
    rec_5g.sort_by_key(|&(_, c)| c);
    let recommended_5g: Vec<u32> = rec_5g.into_iter().take(5).map(|(ch, _)| ch).collect();

    (congestion, recommended_2g, recommended_5g)
}

// ── WiFi Scan Platform Implementations ──────────────────────────────────

#[cfg(target_os = "macos")]
async fn platform_wifi_scan() -> Result<(Vec<WifiNetwork>, Option<WifiNetwork>, Option<String>), NetworkError> {
    // Use CoreWLAN via Swift helper script for non-redacted SSIDs.
    // macOS system_profiler redacts SSIDs in recent versions; CoreWLAN
    // accessed through the `swift` interpreter inherits the parent app's
    // Location Services authorisation and returns the real network names.
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("wifi-scan.swift");

    let output = tokio::process::Command::new("swift")
        .arg(&script)
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run wifi-scan.swift: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(NetworkError::Io(format!("wifi-scan.swift failed: {}", stderr)));
    }

    // JSON produced by wifi-scan.swift
    #[derive(serde::Deserialize)]
    struct SwiftScanNetwork {
        ssid: String,
        bssid: String,
        channel: u32,
        channel_width_mhz: u32,
        #[allow(dead_code)]
        band: String,
        signal_dbm: i32,
        noise_dbm: i32,
        security: String,
        #[allow(dead_code)]
        phy_mode: Option<String>,
        is_current: bool,
    }

    #[derive(serde::Deserialize)]
    struct SwiftScanOutput {
        networks: Vec<SwiftScanNetwork>,
        #[allow(dead_code)]
        current_ssid: Option<String>,
        interface_name: Option<String>,
    }

    let parsed: SwiftScanOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| NetworkError::Io(format!("Failed to parse wifi-scan JSON: {}", e)))?;

    let mut networks = Vec::new();
    let mut current_net = None;

    for net in &parsed.networks {
        let channel_str = format!("{}", net.channel);
        let (channel, channel_width, freq_hint) = parse_channel_info(&channel_str);
        let band = parse_band_from_channel(channel, freq_hint);
        let security = parse_macos_security(&net.security);
        let bssid = if net.bssid.is_empty() { None } else { Some(net.bssid.clone()) };
        let (signal_dbm, noise_dbm) = (Some(net.signal_dbm), Some(net.noise_dbm));
        let wifi = WifiNetwork {
            ssid: net.ssid.clone(),
            bssid,
            channel,
            channel_width_mhz: channel_width.or(Some(net.channel_width_mhz)),
            band,
            frequency_mhz: None,
            signal_dbm,
            noise_dbm,
            security,
            phy_mode: net.phy_mode.clone(),
            is_current: net.is_current,
        };
        if net.is_current && current_net.is_none() {
            current_net = Some(wifi.clone());
        }
        networks.push(wifi);
    }

    Ok((networks, current_net, parsed.interface_name))
}

#[cfg(target_os = "linux")]
async fn platform_wifi_scan() -> Result<(Vec<WifiNetwork>, Option<WifiNetwork>, Option<String>), NetworkError> {
    // Get current connection info
    let current_output = tokio::process::Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,NAME,TYPE", "connection", "show", "--active"])
        .output()
        .await
        .ok();

    let mut current_ssid = String::new();
    let mut iface_name = None;
    if let Some(ref out) = current_output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() >= 3 && fields[2] == "802-11-wireless" {
                iface_name = Some(fields[0].to_string());
                current_ssid = fields[1].to_string();
                break;
            }
        }
    }

    // Scan visible networks
    let output = tokio::process::Command::new("nmcli")
        .args(["-t", "-f", "SSID,BSSID,CHAN,FREQ,SIGNAL,SECURITY,MODE", "dev", "wifi", "list", "--rescan", "yes"])
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run nmcli: {}", e)))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut networks = Vec::new();
    let mut current_net = None;

    for line in text.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 7 { continue; }
        let ssid = fields[0].to_string();
        let bssid = Some(fields[1].trim().to_string());
        let freq: u32 = fields[3].split_whitespace().next()
            .and_then(|s| s.parse().ok()).unwrap_or(0);
        let signal_pct: i32 = fields[4].parse().unwrap_or(0);
        // Convert percentage to approximate dBm
        let signal_dbm = if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None };
        let security_raw = fields[5];
        let (channel, _channel_width, freq_hint) = parse_channel_info(fields[2]);
        let band = parse_band_from_channel(channel, freq_hint);
        let security = parse_macos_security(security_raw);
        let is_current = ssid == current_ssid;
        let net = WifiNetwork {
            ssid,
            bssid,
            channel,
            channel_width_mhz: None,
            band,
            frequency_mhz: Some(freq),
            signal_dbm,
            noise_dbm: None,
            security,
            phy_mode: None,
            is_current,
        };
        if is_current { current_net = Some(net.clone()); }
        networks.push(net);
    }

    Ok((networks, current_net, iface_name))
}

#[cfg(target_os = "windows")]
async fn platform_wifi_scan() -> Result<(Vec<WifiNetwork>, Option<WifiNetwork>, Option<String>), NetworkError> {
    let output = tokio::process::Command::new("netsh")
        .args(["wlan", "show", "networks", "mode=bssid"])
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run netsh: {}", e)))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut networks = Vec::new();
    let mut current_ssid = String::new();

    // Get current connection
    if let Ok(iface_out) = tokio::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .await
    {
        let iface_text = String::from_utf8_lossy(&iface_out.stdout);
        for line in iface_text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("SSID") && !trimmed.starts_with("SSID ") {
                if let Some(val) = trimmed.split(':').nth(1) {
                    current_ssid = val.trim().to_string();
                }
            }
        }
    }

    let mut ssid = String::new();
    let mut bssid = None;
    let mut signal_pct: i32 = 0;
    let mut channel: u32 = 0;
    let mut security = WifiSecurity::Open;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID") && !trimmed.starts_with("SSID ") {
            if let Some(val) = trimmed.split(':').nth(1) {
                ssid = val.trim().to_string();
            }
        } else if trimmed.starts_with("BSSID") {
            // Save previous network if any
            if bssid.is_some() {
                let is_current = ssid == current_ssid;
                let ch_str = channel.to_string();
                let (channel, channel_width, freq_hint) = parse_channel_info(&ch_str);
                let band = parse_band_from_channel(channel, freq_hint);
                let sec = security.clone();
                let net = WifiNetwork {
                    ssid: ssid.clone(), bssid: bssid.take(), channel, channel_width_mhz: channel_width,
                    band, frequency_mhz: None, signal_dbm: if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None }, noise_dbm: None,
                    security: sec, phy_mode: None, is_current,
                };
                networks.push(net);
            }
            bssid = trimmed.split(':').nth(1).map(|s| s.trim().to_string());
            // Reset for this BSSID entry
            signal_pct = 0;
            channel = 0;
        } else if trimmed.starts_with("Signal") {
            signal_pct = trimmed.replace('%', "").split(':').nth(1)
                .and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        } else if trimmed.starts_with("Channel") {
            channel = trimmed.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        } else if trimmed.starts_with("Authentication") {
            let auth = trimmed.split(':').nth(1).map(|s| s.trim().to_lowercase()).unwrap_or_default();
            security = parse_macos_security(&auth);
        }
    }
    // Push last entry
    if bssid.is_some() || !ssid.is_empty() {
        let is_current = ssid == current_ssid;
        let ch_str = channel.to_string();
        let (channel, channel_width, freq_hint) = parse_channel_info(&ch_str);
        let band = parse_band_from_channel(channel, freq_hint);
        networks.push(WifiNetwork {
            ssid, bssid, channel, channel_width_mhz: channel_width,
            band, frequency_mhz: None, signal_dbm: if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None }, noise_dbm: None,
            security, phy_mode: None, is_current,
        });
    }

    let current_net = networks.iter().find(|n| n.is_current).cloned();
    Ok((networks, current_net, None))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
async fn platform_wifi_scan() -> Result<(Vec<WifiNetwork>, Option<WifiNetwork>, Option<String>), NetworkError> {
    Err(NetworkError::Io("WiFi scanning not supported on this platform".into()))
}

// ── Local subnet detection ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LocalSubnet {
    pub interface: String,
    pub cidr: String,
    pub ip: String,
}

#[tauri::command]
pub async fn network_local_subnets() -> Vec<LocalSubnet> {
    let mut subnets: Vec<LocalSubnet> = Vec::new();

    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                let ip = v4.ip;
                let octets = ip.octets();
                // Skip loopback range and link-local (169.254.x.x)
                if octets[0] == 127 || (octets[0] == 169 && octets[1] == 254) {
                    continue;
                }
                let netmask = v4.netmask;
                let prefix = u32::from(netmask).count_ones() as u8;
                let network = Ipv4Addr::from(u32::from(ip) & u32::from(netmask));
                subnets.push(LocalSubnet {
                    interface: iface.name.clone(),
                    cidr: format!("{}/{}", network, prefix),
                    ip: ip.to_string(),
                });
            }
        }
    }

    subnets.sort_by(|a, b| a.cidr.cmp(&b.cidr));
    subnets.dedup_by(|a, b| a.cidr == b.cidr);
    subnets
}

// ── Tailscale peer discovery ────────────────────────────────────────────
// Tailscale peers live in the 100.64.0.0/10 CGNAT range — a different
// address space from the LAN a Network Explorer scan targets, and
// port-scanning that whole /10 to find them isn't remotely feasible (it's
// ~4 million addresses, sparsely and randomly assigned). But Tailscale
// already knows exactly who's on the tailnet, by name, with zero probing
// needed: `tailscale status --json` is authoritative. This is why a normal
// LAN scan never surfaces a device's tailnet identity — it was never asked.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscalePeer {
    pub ip: String,
    pub hostname: String,
    pub os: Option<String>,
    pub online: bool,
    pub is_self: bool,
}

fn find_tailscale_binary() -> Option<std::path::PathBuf> {
    if let Ok(path) = which::which("tailscale") {
        return Some(path);
    }
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["/Applications/Tailscale.app/Contents/MacOS/Tailscale", "/usr/local/bin/tailscale", "/opt/homebrew/bin/tailscale"]
    } else if cfg!(target_os = "windows") {
        &["C:\\Program Files\\Tailscale\\tailscale.exe"]
    } else {
        &["/usr/bin/tailscale", "/usr/local/bin/tailscale"]
    };
    candidates.iter().map(std::path::PathBuf::from).find(|p| p.exists())
}

fn extract_tailscale_peer(v: &serde_json::Value, is_self: bool) -> Option<TailscalePeer> {
    let ip = v.get("TailscaleIPs")?.as_array()?.iter().find_map(|a| {
        let s = a.as_str()?;
        (s.parse::<Ipv4Addr>().is_ok()).then(|| s.to_string())
    })?;
    let dns_name = v.get("DNSName").and_then(|d| d.as_str()).unwrap_or("").trim_end_matches('.');
    let host_name = v.get("HostName").and_then(|d| d.as_str()).unwrap_or("");
    let hostname = if !dns_name.is_empty() { dns_name.to_string() } else { host_name.to_string() };
    if hostname.is_empty() {
        return None;
    }
    let os = v.get("OS").and_then(|o| o.as_str()).map(str::to_string);
    let online = v.get("Online").and_then(|o| o.as_bool()).unwrap_or(is_self);
    Some(TailscalePeer { ip, hostname, os, online, is_self })
}

/// Parses `tailscale status --json` output (Self + Peer map) into a flat,
/// sorted peer list. Pure/testable separately from the subprocess spawn.
fn parse_tailscale_status(json: &serde_json::Value) -> Vec<TailscalePeer> {
    let mut peers = Vec::new();
    if let Some(self_node) = json.get("Self") {
        peers.extend(extract_tailscale_peer(self_node, true));
    }
    if let Some(peer_map) = json.get("Peer").and_then(|p| p.as_object()) {
        for v in peer_map.values() {
            peers.extend(extract_tailscale_peer(v, false));
        }
    }
    peers.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    peers
}

#[tauri::command]
pub async fn network_tailscale_peers() -> Result<Vec<TailscalePeer>, NetworkError> {
    let binary = find_tailscale_binary()
        .ok_or_else(|| NetworkError::Io("Tailscale CLI not found".to_string()))?;
    let output = tokio::process::Command::new(&binary)
        .args(["status", "--json"])
        .output()
        .await
        .map_err(|e| NetworkError::Io(e.to_string()))?;
    if !output.status.success() {
        return Err(NetworkError::Io("`tailscale status` failed — is Tailscale running and logged in?".to_string()));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| NetworkError::Io(format!("failed to parse tailscale status: {e}")))?;
    Ok(parse_tailscale_status(&json))
}

#[tauri::command]
pub async fn network_wifi_scan() -> Result<WifiScanResult, NetworkError> {
    let (networks, current_net, iface_name) = platform_wifi_scan().await?;
    let security_issues = assess_security(&networks);
    let (channel_congestion, recommended_2g, recommended_5g) = compute_channel_congestion(&networks);

    Ok(WifiScanResult {
        networks,
        security_issues,
        channel_congestion,
        recommended_channels_2g: recommended_2g,
        recommended_channels_5g: recommended_5g,
        current_network: current_net,
        interface_name: iface_name,
        scan_timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

// ── Aircrack-ng Types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircrackToolStatus {
    pub aircrack_ng: bool,
    pub airmon_ng: bool,
    pub airodump_ng: bool,
    pub aireplay_ng: bool,
    pub version: Option<String>,
    pub needs_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WirelessInterface {
    pub name: String,
    pub driver: Option<String>,
    pub chipset: Option<String>,
    pub monitor_mode: bool,
    pub monitor_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircrackOpKind {
    MonitorStart,
    MonitorStop,
    Scan,
    Deauth,
    CaptureHandshake,
    CrackWpa,
    CrackWep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircrackProcess {
    pub id: String,
    pub kind: AircrackOpKind,
    pub interface: String,
    pub started_at: String,
    pub target_bssid: Option<String>,
    pub pid: Option<u32>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircrackAuditEntry {
    pub timestamp: String,
    pub operation: AircrackOpKind,
    pub interface: String,
    pub target: Option<String>,
    pub command: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirodumpNetwork {
    pub bssid: String,
    pub channel: i32,
    pub privacy: String,
    pub cipher: Option<String>,
    pub auth: Option<String>,
    pub power: i32,
    pub beacons: u32,
    pub data_frames: u32,
    pub iv_count: u32,
    pub essid: String,
    pub wps: Option<String>,
    pub clients: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirodumpClient {
    pub station_mac: String,
    pub bssid: String,
    pub power: i32,
    pub packets: u32,
    pub probes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirodumpResult {
    pub networks: Vec<AirodumpNetwork>,
    pub clients: Vec<AirodumpClient>,
    pub scan_id: String,
    pub interface: String,
    pub scan_time_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeCaptureStatus {
    pub operation_id: String,
    pub target_bssid: String,
    pub target_essid: String,
    pub handshake_captured: bool,
    pub capture_file: Option<String>,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrackProgress {
    pub operation_id: String,
    pub target_bssid: String,
    pub keys_tested: u64,
    pub keys_per_second: f64,
    pub key_found: Option<String>,
    pub running: bool,
    pub elapsed_secs: u64,
}

// ── Aircrack-ng Helpers ─────────────────────────────────────────────────

fn aircrack_audit(
    state: &NetworkState,
    op: AircrackOpKind,
    interface: &str,
    target: Option<&str>,
    command: &str,
    result: &str,
) {
    let mut log = state.aircrack_audit_log.lock().unwrap();
    log.push(AircrackAuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        operation: op,
        interface: interface.to_string(),
        target: target.map(String::from),
        command: command.to_string(),
        result: result.to_string(),
    });
}

fn require_disclaimer(state: &NetworkState) -> Result<(), NetworkError> {
    if !state.aircrack_disclaimer_accepted.load(Ordering::SeqCst) {
        return Err(NetworkError::Io(
            "You must accept the educational disclaimer before using aircrack-ng tools. \
             These tools are for authorized security testing and education only."
                .into(),
        ));
    }
    Ok(())
}

/// Extra search paths for aircrack-ng tools (Homebrew sbin, etc.)
fn aircrack_search_paths() -> Vec<String> {
    let mut dirs: Vec<String> = vec![
        "/opt/homebrew/sbin".into(),
        "/opt/homebrew/bin".into(),
        "/usr/local/sbin".into(),
        "/usr/local/bin".into(),
        "/usr/sbin".into(),
        "/usr/bin".into(),
    ];
    if let Ok(path) = std::env::var("PATH") {
        for p in path.split(':') {
            if !dirs.contains(&p.to_string()) {
                dirs.push(p.to_string());
            }
        }
    }
    dirs
}

/// Resolve the full path for an aircrack tool, searching extra locations.
fn resolve_tool(name: &str) -> String {
    for dir in aircrack_search_paths() {
        let candidate = format!("{}/{}", dir, name);
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    name.to_string() // fallback to bare name
}

async fn check_tool_exists(name: &str) -> bool {
    // First: try bare `which`
    let which_ok = tokio::process::Command::new("which")
        .arg(name)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    if which_ok {
        return true;
    }
    // Second: search common install dirs directly
    for dir in aircrack_search_paths() {
        let candidate = format!("{}/{}", dir, name);
        if std::path::Path::new(&candidate).exists() {
            return true;
        }
    }
    false
}

async fn get_aircrack_version() -> Option<String> {
    let output = tokio::process::Command::new(resolve_tool("aircrack-ng"))
        .arg("--help")
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", text, stderr);
    for line in combined.lines() {
        if line.contains("Aircrack-ng") && (line.contains('.') || line.contains("1.")) {
            return Some(line.trim().to_string());
        }
    }
    None
}

#[allow(dead_code)]
fn parse_airodump_csv(csv_path: &str) -> Result<(Vec<AirodumpNetwork>, Vec<AirodumpClient>), NetworkError> {
    let content = std::fs::read_to_string(csv_path)
        .map_err(|e| NetworkError::Io(format!("Failed to read airodump CSV: {}", e)))?;

    let mut networks = Vec::new();
    let mut clients = Vec::new();
    let mut section = 0; // 0 = header, 1 = APs, 2 = clients

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            section += 1;
            continue;
        }
        if trimmed.starts_with("BSSID") && section <= 1 {
            section = 1;
            continue;
        }
        if trimmed.starts_with("Station MAC") {
            section = 2;
            continue;
        }

        let fields: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();

        if section == 1 && fields.len() >= 14 {
            let bssid = fields[0].to_string();
            if bssid.len() < 17 { continue; } // Skip invalid
            networks.push(AirodumpNetwork {
                bssid,
                channel: fields[3].parse().unwrap_or(-1),
                privacy: fields[5].to_string(),
                cipher: if fields[6].is_empty() { None } else { Some(fields[6].to_string()) },
                auth: if fields[7].is_empty() { None } else { Some(fields[7].to_string()) },
                power: fields[8].parse().unwrap_or(-1),
                beacons: fields[9].parse().unwrap_or(0),
                data_frames: fields[10].parse().unwrap_or(0),
                iv_count: fields[11].parse().unwrap_or(0),
                essid: fields[13].to_string(),
                wps: if fields.len() > 14 && !fields[14].is_empty() { Some(fields[14].to_string()) } else { None },
                clients: 0,
            });
        } else if section == 2 && fields.len() >= 6 {
            let station_mac = fields[0].to_string();
            if station_mac.len() < 17 { continue; }
            let bssid = fields[5].to_string();
            let probes: Vec<String> = if fields.len() > 6 {
                fields[6].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            } else {
                Vec::new()
            };
            clients.push(AirodumpClient {
                station_mac,
                bssid: bssid.clone(),
                power: fields[3].parse().unwrap_or(-1),
                packets: fields[4].parse().unwrap_or(0),
                probes,
            });
        }
    }

    // Count clients per network
    for net in &mut networks {
        net.clients = clients.iter().filter(|c| c.bssid == net.bssid).count() as u32;
    }

    Ok((networks, clients))
}

// ── Aircrack-ng Commands ────────────────────────────────────────────────

/// Check if aircrack-ng suite is installed and available
#[tauri::command]
pub async fn network_aircrack_check() -> Result<AircrackToolStatus, NetworkError> {
    let (aircrack, airmon, airodump, aireplay) = tokio::join!(
        check_tool_exists("aircrack-ng"),
        check_tool_exists("airmon-ng"),
        check_tool_exists("airodump-ng"),
        check_tool_exists("aireplay-ng"),
    );
    let version = get_aircrack_version().await;

    // Check if we need root/sudo
    #[cfg(unix)]
    let needs_root = unsafe { libc::getuid() != 0 };
    #[cfg(not(unix))]
    let needs_root = true;

    Ok(AircrackToolStatus {
        aircrack_ng: aircrack,
        airmon_ng: airmon,
        airodump_ng: airodump,
        aireplay_ng: aireplay,
        version,
        needs_root,
    })
}

/// Accept the educational/ethical use disclaimer
#[tauri::command]
pub async fn network_aircrack_accept_disclaimer(
    state: tauri::State<'_, NetworkState>,
) -> Result<bool, NetworkError> {
    state.aircrack_disclaimer_accepted.store(true, Ordering::SeqCst);

    // Audit the acceptance
    aircrack_audit(
        &state,
        AircrackOpKind::MonitorStart, // reusing — just an audit marker
        "none",
        None,
        "disclaimer_accepted",
        "User accepted educational/ethical use disclaimer",
    );

    Ok(true)
}

/// List wireless interfaces available for monitor mode
#[tauri::command]
pub async fn network_aircrack_interfaces(
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<WirelessInterface>, NetworkError> {
    require_disclaimer(&state)?;

    let mut interfaces = Vec::new();

    // ── macOS: use networksetup + system_profiler ──
    #[cfg(target_os = "macos")]
    {
        let mon_set = state.monitor_interfaces.lock().unwrap().clone();

        // `networksetup -listallhardwareports` gives us port/device/address triples.
        let output = tokio::process::Command::new("networksetup")
            .args(["-listallhardwareports"])
            .output()
            .await
            .map_err(|e| NetworkError::Io(format!("Failed to run networksetup: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout);

        let mut port_name = String::new();
        let mut device_name = String::new();
        for line in text.lines() {
            if let Some(p) = line.strip_prefix("Hardware Port: ") {
                port_name = p.trim().to_string();
            } else if let Some(d) = line.strip_prefix("Device: ") {
                device_name = d.trim().to_string();
            } else if line.starts_with("Ethernet Address:") || line.trim().is_empty() {
                // Wi-Fi interfaces show up as "Wi-Fi" or "AirPort"
                let lower = port_name.to_lowercase();
                if (lower.contains("wi-fi") || lower.contains("wifi") || lower.contains("airport"))
                    && !device_name.is_empty()
                {
                    let is_mon = mon_set.contains(&device_name);
                    interfaces.push(WirelessInterface {
                        name: device_name.clone(),
                        driver: Some(port_name.clone()),
                        chipset: Some("Apple Wi-Fi".into()),
                        monitor_mode: is_mon,
                        monitor_name: if is_mon { Some(device_name.clone()) } else { None },
                    });
                }
                port_name.clear();
                device_name.clear();
            }
        }
    }

    // ── Linux: use airmon-ng, fallback to iw dev ──
    #[cfg(not(target_os = "macos"))]
    {
        let airmon = resolve_tool("airmon-ng");
        let output = tokio::process::Command::new(&airmon)
            .output()
            .await;

        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines().skip(3) {
                // airmon-ng output: PHY Interface Driver Chipset
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[1].to_string();
                    let is_mon = name.contains("mon");
                    interfaces.push(WirelessInterface {
                        name: name.clone(),
                        driver: parts.get(2).map(|s| s.to_string()),
                        chipset: if parts.len() > 3 { Some(parts[3..].join(" ")) } else { None },
                        monitor_mode: is_mon,
                        monitor_name: if is_mon { Some(name) } else { None },
                    });
                }
            }
        }

        // Fallback: try iw dev if airmon-ng returned nothing
        if interfaces.is_empty() {
            let iw_out = tokio::process::Command::new("iw")
                .arg("dev")
                .output()
                .await
                .ok();
            if let Some(iw) = iw_out {
                let iw_text = String::from_utf8_lossy(&iw.stdout);
                let mut current_iface = String::new();
                for line in iw_text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Interface") {
                        current_iface = trimmed.replace("Interface ", "").trim().to_string();
                    }
                    if trimmed.starts_with("type") && !current_iface.is_empty() {
                        let is_mon = trimmed.contains("monitor");
                        interfaces.push(WirelessInterface {
                            name: current_iface.clone(),
                            driver: None,
                            chipset: None,
                            monitor_mode: is_mon,
                            monitor_name: if is_mon { Some(current_iface.clone()) } else { None },
                        });
                    }
                }
            }
        }
    }

    Ok(interfaces)
}

/// Enable monitor mode on a wireless interface.
/// ⚠️ WARNING: This disrupts normal WiFi connectivity on the interface.
#[tauri::command]
pub async fn network_aircrack_monitor_start(
    interface: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<WirelessInterface, NetworkError> {
    require_disclaimer(&state)?;

    // On macOS, monitor mode uses `airport` sniff or is simply not supported
    // on modern hardware. We try to create a PCAP-based monitor via
    // `tcpdump` / `en0 sniff` as a best-effort approach.
    #[cfg(target_os = "macos")]
    {
        // macOS doesn't support airmon-ng.  Apple removed the airport CLI,
        // and modern Apple Silicon Macs don't expose raw monitor mode.
        // Mark the interface as "pseudo-monitor" so the UI shows it, but
        // airodump-ng packet capture won't work the Linux way.
        let cmd_str = format!("(macOS) pseudo-monitor on {}", interface);
        aircrack_audit(
            &state, AircrackOpKind::MonitorStart, &interface, None,
            &cmd_str, "macOS monitor mode is limited",
        );
        state.monitor_interfaces.lock().unwrap().insert(interface.clone());
        Ok(WirelessInterface {
            name: interface.clone(),
            driver: Some("Apple Wi-Fi".into()),
            chipset: Some("macOS – limited monitor support".into()),
            monitor_mode: true,
            monitor_name: Some(interface),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        let airmon = resolve_tool("airmon-ng");
        let cmd_str = format!("{} start {}", airmon, interface);
        let output = tokio::process::Command::new(&airmon)
            .args(["start", &interface])
            .output()
            .await
            .map_err(|e| NetworkError::Io(format!("Failed to start monitor mode: {}", e)))?;

        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let result_str = if output.status.success() { "success" } else { "failed" };
        aircrack_audit(&state, AircrackOpKind::MonitorStart, &interface, None, &cmd_str, result_str);

        if !output.status.success() {
            return Err(NetworkError::Io(format!("airmon-ng start failed: {}", text)));
        }

        let mon_name = if text.contains("mon") {
            text.lines()
                .find_map(|l| {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    parts.iter().find(|p| p.contains("mon")).map(|s| {
                        s.trim_matches(|c: char| c == '(' || c == ')' || c == '[' || c == ']')
                            .to_string()
                    })
                })
                .unwrap_or_else(|| format!("{}mon", interface))
        } else {
            format!("{}mon", interface)
        };

        Ok(WirelessInterface {
            name: mon_name.clone(),
            driver: None,
            chipset: None,
            monitor_mode: true,
            monitor_name: Some(mon_name),
        })
    }
}

/// Disable monitor mode on a wireless interface.
#[tauri::command]
pub async fn network_aircrack_monitor_stop(
    interface: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<String, NetworkError> {
    require_disclaimer(&state)?;

    #[cfg(target_os = "macos")]
    {
        aircrack_audit(
            &state, AircrackOpKind::MonitorStop, &interface, None,
            "(macOS) pseudo-monitor stop", "success",
        );
        state.monitor_interfaces.lock().unwrap().remove(&interface);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let airmon = resolve_tool("airmon-ng");
        let cmd_str = format!("{} stop {}", airmon, interface);
        let output = tokio::process::Command::new(&airmon)
            .args(["stop", &interface])
            .output()
            .await
            .map_err(|e| NetworkError::Io(format!("Failed to stop monitor mode: {}", e)))?;

        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let result_str = if output.status.success() { "success" } else { "failed" };
        aircrack_audit(&state, AircrackOpKind::MonitorStop, &interface, None, &cmd_str, result_str);

        if !output.status.success() {
            return Err(NetworkError::Io(format!("airmon-ng stop failed: {}", text)));
        }
    }

    // Clean up process tracking
    let mut procs = state.aircrack_processes.lock().unwrap();
    procs.retain(|_, p| p.interface != interface);

    Ok(format!("Monitor mode stopped on {}", interface))
}

/// Start an airodump-ng scan to discover networks and clients.
/// This runs for the specified duration then returns results.
/// ⚠️ WARNING: Requires monitor mode interface.
#[tauri::command]
pub async fn network_aircrack_scan_start(
    interface: String,
    duration_secs: Option<u64>,
    channel: Option<i32>,
    state: tauri::State<'_, NetworkState>,
) -> Result<AirodumpResult, NetworkError> {
    require_disclaimer(&state)?;

    let scan_id = Uuid::new_v4().to_string();
    let duration = duration_secs.unwrap_or(15);

    // ── macOS: airodump-ng doesn't work with pseudo-monitor mode.
    //    Fall back to CoreWLAN via the existing platform_wifi_scan(). ──
    #[cfg(target_os = "macos")]
    {
        aircrack_audit(
            &state, AircrackOpKind::Scan, &interface, None,
            "(macOS) CoreWLAN scan fallback", "started",
        );

        let (wifi_networks, _current, _iface) = platform_wifi_scan().await?;

        let networks: Vec<AirodumpNetwork> = wifi_networks
            .iter()
            .filter(|n| {
                // If a specific channel was requested, filter to it
                channel.map_or(true, |ch| n.channel == ch as u32)
            })
            .map(|n| {
                let privacy = match &n.security {
                    WifiSecurity::Wpa3Sae => "WPA3".to_string(),
                    WifiSecurity::Wpa3Transition => "WPA3".to_string(),
                    WifiSecurity::Wpa3Enterprise => "WPA3".to_string(),
                    WifiSecurity::Wpa2Psk | WifiSecurity::Wpa2Enterprise => "WPA2".to_string(),
                    WifiSecurity::WpaPsk => "WPA".to_string(),
                    WifiSecurity::Wep => "WEP".to_string(),
                    WifiSecurity::Open => "OPN".to_string(),
                    WifiSecurity::Unknown(s) => s.clone(),
                };
                AirodumpNetwork {
                    bssid: n.bssid.clone().unwrap_or_default(),
                    channel: n.channel as i32,
                    privacy,
                    cipher: None,
                    auth: None,
                    power: n.signal_dbm.unwrap_or(-100),
                    beacons: 0,
                    data_frames: 0,
                    iv_count: 0,
                    essid: n.ssid.clone(),
                    wps: None,
                    clients: 0,
                }
            })
            .collect();

        let count = networks.len();
        aircrack_audit(
            &state, AircrackOpKind::Scan, &interface, None,
            "(macOS) CoreWLAN scan fallback",
            &format!("completed: {} networks, 0 clients", count),
        );

        Ok(AirodumpResult {
            networks,
            clients: Vec::new(),
            scan_id,
            interface,
            scan_time_secs: duration,
        })
    }

    // ── Linux: use airodump-ng ──
    #[cfg(not(target_os = "macos"))]
    {
    let tmp_prefix = format!("/tmp/crossterm_airodump_{}", scan_id);

    let mut args = vec![
        "--write".to_string(),
        tmp_prefix.clone(),
        "--write-interval".to_string(),
        "1".to_string(),
        "--output-format".to_string(),
        "csv".to_string(),
    ];
    if let Some(ch) = channel {
        args.push("--channel".to_string());
        args.push(ch.to_string());
    }
    args.push(interface.clone());

    let cmd_str = format!("airodump-ng {}", args.join(" "));
    aircrack_audit(&state, AircrackOpKind::Scan, &interface, None, &cmd_str, "started");

    let mut child = tokio::process::Command::new(resolve_tool("airodump-ng"))
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| NetworkError::Io(format!("Failed to start airodump-ng: {}", e)))?;

    let pid = child.id();

    // Track the process
    {
        let mut procs = state.aircrack_processes.lock().unwrap();
        procs.insert(scan_id.clone(), AircrackProcess {
            id: scan_id.clone(),
            kind: AircrackOpKind::Scan,
            interface: interface.clone(),
            started_at: chrono::Utc::now().to_rfc3339(),
            target_bssid: None,
            pid,
            active: true,
        });
    }

    // Wait for duration then kill
    tokio::time::sleep(Duration::from_secs(duration)).await;
    let _ = child.kill().await;

    // Mark as inactive
    {
        let mut procs = state.aircrack_processes.lock().unwrap();
        if let Some(p) = procs.get_mut(&scan_id) {
            p.active = false;
        }
    }

    // Parse results from CSV
    let csv_path = format!("{}-01.csv", tmp_prefix);
    let (networks, clients) = if std::path::Path::new(&csv_path).exists() {
        parse_airodump_csv(&csv_path)?
    } else {
        (Vec::new(), Vec::new())
    };

    // Clean up temp files
    for ext in &["csv", "cap", "kismet.csv", "kismet.netxml", "log.csv"] {
        let path = format!("{}-01.{}", tmp_prefix, ext);
        let _ = std::fs::remove_file(&path);
    }

    aircrack_audit(&state, AircrackOpKind::Scan, &interface, None, &cmd_str,
        &format!("completed: {} networks, {} clients", networks.len(), clients.len()));

    Ok(AirodumpResult {
        networks,
        clients,
        scan_id,
        interface,
        scan_time_secs: duration,
    })
    } // #[cfg(not(target_os = "macos"))]
}

/// Send deauthentication frames to a target.
/// ⚠️ DANGEROUS: This disconnects the target client from the network.
/// For authorized testing and education ONLY.
#[tauri::command]
pub async fn network_aircrack_deauth(
    interface: String,
    target_bssid: String,
    client_mac: Option<String>,
    count: Option<u32>,
    state: tauri::State<'_, NetworkState>,
) -> Result<String, NetworkError> {
    require_disclaimer(&state)?;

    let deauth_count = count.unwrap_or(5); // Default to 5, NOT continuous
    let mut args = vec![
        "--deauth".to_string(),
        deauth_count.to_string(),
        "-a".to_string(),
        target_bssid.clone(),
    ];
    if let Some(ref client) = client_mac {
        args.push("-c".to_string());
        args.push(client.clone());
    }
    args.push(interface.clone());

    let cmd_str = format!("aireplay-ng {}", args.join(" "));
    let target_desc = format!("bssid={} client={}", target_bssid, client_mac.as_deref().unwrap_or("broadcast"));
    aircrack_audit(&state, AircrackOpKind::Deauth, &interface, Some(&target_desc), &cmd_str, "started");

    let output = tokio::process::Command::new(resolve_tool("aireplay-ng"))
        .args(&args)
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run aireplay-ng: {}", e)))?;

    let result_str = if output.status.success() { "completed" } else { "failed" };
    aircrack_audit(&state, AircrackOpKind::Deauth, &interface, Some(&target_desc), &cmd_str, result_str);

    Ok(format!("Sent {} deauth frames to {} ({})", deauth_count, target_bssid, result_str))
}

/// Capture a WPA handshake by monitoring and optionally deauthing.
/// ⚠️ WARNING: May send deauth frames. For authorized testing only.
#[tauri::command]
pub async fn network_aircrack_capture_handshake(
    interface: String,
    target_bssid: String,
    target_channel: i32,
    send_deauth: Option<bool>,
    timeout_secs: Option<u64>,
    state: tauri::State<'_, NetworkState>,
) -> Result<HandshakeCaptureStatus, NetworkError> {
    require_disclaimer(&state)?;

    let op_id = Uuid::new_v4().to_string();
    let timeout = timeout_secs.unwrap_or(60);
    let tmp_prefix = format!("/tmp/crossterm_handshake_{}", op_id);

    // Start airodump-ng on specific channel/bssid to capture handshake
    let dump_args = vec![
        "--bssid".to_string(),
        target_bssid.clone(),
        "--channel".to_string(),
        target_channel.to_string(),
        "--write".to_string(),
        tmp_prefix.clone(),
        "--output-format".to_string(),
        "cap".to_string(),
        interface.clone(),
    ];

    let cmd_str = format!("airodump-ng {}", dump_args.join(" "));
    aircrack_audit(&state, AircrackOpKind::CaptureHandshake, &interface, Some(&target_bssid), &cmd_str, "started");

    let mut dump_child = tokio::process::Command::new(resolve_tool("airodump-ng"))
        .args(&dump_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| NetworkError::Io(format!("Failed to start airodump-ng: {}", e)))?;

    // Optionally send a deauth to speed up handshake capture
    if send_deauth.unwrap_or(false) {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let deauth_args = vec![
            "--deauth", "3",
            "-a", &target_bssid,
            &interface,
        ];
        let _ = tokio::process::Command::new(resolve_tool("aireplay-ng"))
            .args(&deauth_args)
            .output()
            .await;
        aircrack_audit(&state, AircrackOpKind::Deauth, &interface, Some(&target_bssid),
            &format!("aireplay-ng --deauth 3 -a {} {}", target_bssid, interface), "sent 3 deauth frames for handshake capture");
    }

    // Wait for timeout
    tokio::time::sleep(Duration::from_secs(timeout)).await;
    let _ = dump_child.kill().await;

    // Check if handshake was captured
    let cap_file = format!("{}-01.cap", tmp_prefix);
    let handshake_captured = if std::path::Path::new(&cap_file).exists() {
        // Use aircrack-ng to verify handshake exists in capture
        let verify = tokio::process::Command::new(resolve_tool("aircrack-ng"))
            .arg(&cap_file)
            .output()
            .await
            .ok();
        verify.map(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            text.contains("1 handshake") || text.contains("handshake")
        }).unwrap_or(false)
    } else {
        false
    };

    let result_str = if handshake_captured { "handshake captured" } else { "no handshake" };
    aircrack_audit(&state, AircrackOpKind::CaptureHandshake, &interface, Some(&target_bssid), &cmd_str, result_str);

    Ok(HandshakeCaptureStatus {
        operation_id: op_id,
        target_bssid,
        target_essid: String::new(), // Filled by frontend
        handshake_captured,
        capture_file: if handshake_captured { Some(cap_file) } else { None },
        elapsed_secs: timeout,
    })
}

/// Attempt to crack a WPA handshake using a wordlist.
/// ⚠️ For educational use — demonstrates why strong passwords matter.
#[tauri::command]
pub async fn network_aircrack_crack_start(
    capture_file: String,
    target_bssid: String,
    wordlist_path: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<CrackProgress, NetworkError> {
    require_disclaimer(&state)?;

    // Validate paths exist
    if !std::path::Path::new(&capture_file).exists() {
        return Err(NetworkError::Io("Capture file not found".into()));
    }
    if !std::path::Path::new(&wordlist_path).exists() {
        return Err(NetworkError::Io("Wordlist file not found".into()));
    }

    let op_id = Uuid::new_v4().to_string();
    let cmd_str = format!("aircrack-ng -b {} -w {} {}", target_bssid, wordlist_path, capture_file);
    aircrack_audit(&state, AircrackOpKind::CrackWpa, "n/a", Some(&target_bssid), &cmd_str, "started");

    let start = Instant::now();
    let output = tokio::process::Command::new(resolve_tool("aircrack-ng"))
        .args(["-b", &target_bssid, "-w", &wordlist_path, &capture_file])
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run aircrack-ng: {}", e)))?;

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Parse output for key found
    let key_found = text.lines()
        .find(|l| l.contains("KEY FOUND!"))
        .map(|l| {
            l.split('[')
                .nth(1)
                .and_then(|s| s.split(']').next())
                .unwrap_or("")
                .trim()
                .to_string()
        });

    // Parse keys tested
    let keys_tested = text.lines()
        .find_map(|l| {
            if l.contains("keys tested") {
                l.split_whitespace().next().and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            }
        })
        .unwrap_or(0);

    let elapsed = start.elapsed().as_secs();
    let kps = if elapsed > 0 { keys_tested as f64 / elapsed as f64 } else { 0.0 };

    let result_str = if key_found.is_some() { "KEY FOUND" } else { "key not found" };
    aircrack_audit(&state, AircrackOpKind::CrackWpa, "n/a", Some(&target_bssid), &cmd_str, result_str);

    Ok(CrackProgress {
        operation_id: op_id,
        target_bssid,
        keys_tested,
        keys_per_second: kps,
        key_found,
        running: false,
        elapsed_secs: elapsed,
    })
}

/// Get the full audit log of all aircrack-ng operations
#[tauri::command]
pub async fn network_aircrack_audit_log(
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<AircrackAuditEntry>, NetworkError> {
    require_disclaimer(&state)?;
    let log = state.aircrack_audit_log.lock().unwrap();
    Ok(log.clone())
}

/// Stop all running aircrack-ng processes
#[tauri::command]
pub async fn network_aircrack_stop_all(
    state: tauri::State<'_, NetworkState>,
) -> Result<String, NetworkError> {
    let mut procs = state.aircrack_processes.lock().unwrap();
    let mut killed = 0;
    for proc in procs.values_mut() {
        if proc.active {
            if let Some(_pid) = proc.pid {
                #[cfg(unix)]
                unsafe { libc::kill(_pid as i32, libc::SIGTERM); }
                proc.active = false;
                killed += 1;
            }
        }
    }
    Ok(format!("Stopped {} aircrack-ng processes", killed))
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_band_serde_values() {
        assert_eq!(serde_json::to_string(&WifiBand::Band2_4GHz).unwrap(), "\"2.4GHz\"");
        assert_eq!(serde_json::to_string(&WifiBand::Band5GHz).unwrap(), "\"5GHz\"");
        assert_eq!(serde_json::to_string(&WifiBand::Band6GHz).unwrap(), "\"6GHz\"");
        assert_eq!(serde_json::to_string(&WifiBand::Unknown).unwrap(), "\"unknown\"");
    }

    #[test]
    fn test_wol_packet_format() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let packet = build_wol_packet(&mac);

        // Magic packet must be 102 bytes: 6 bytes of 0xFF + 16 repetitions of 6-byte MAC
        assert_eq!(packet.len(), 102);

        // First 6 bytes must be 0xFF
        assert_eq!(&packet[0..6], &[0xFF; 6]);

        // Next 96 bytes must be 16 repetitions of the MAC
        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }
    }

    #[test]
    fn test_tunnel_rule_crud() {
        let state = NetworkState::new();

        // Create
        let rule = TunnelRule {
            id: "test-id-1".to_string(),
            name: "Test Tunnel".to_string(),
            local_port: 8080,
            remote_host: "example.com".to_string(),
            remote_port: 80,
            tunnel_type: TunnelType::Local,
            ssh_session_ref: None,
            auto_start: false,
            enabled: false,
        };

        {
            let mut rules = state.tunnel_rules.lock().unwrap();
            rules.push(rule.clone());
            let mut active = state.active_tunnels.lock().unwrap();
            active.insert("test-id-1".to_string(), TunnelStatus::Inactive);
        }

        // List
        {
            let rules = state.tunnel_rules.lock().unwrap();
            assert_eq!(rules.len(), 1);
            assert_eq!(rules[0].name, "Test Tunnel");
        }

        // Toggle enable
        {
            let mut rules = state.tunnel_rules.lock().unwrap();
            let r = rules.iter_mut().find(|r| r.id == "test-id-1").unwrap();
            r.enabled = true;
            let mut active = state.active_tunnels.lock().unwrap();
            active.insert("test-id-1".to_string(), TunnelStatus::Active);
        }

        {
            let rules = state.tunnel_rules.lock().unwrap();
            assert!(rules[0].enabled);
            let active = state.active_tunnels.lock().unwrap();
            assert!(matches!(
                active.get("test-id-1"),
                Some(TunnelStatus::Active)
            ));
        }

        // Remove
        {
            let mut rules = state.tunnel_rules.lock().unwrap();
            rules.retain(|r| r.id != "test-id-1");
            assert_eq!(rules.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_scan_localhost() {
        let addrs = parse_cidr("127.0.0.1/32").unwrap();
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], Ipv4Addr::new(127, 0, 0, 1));

        // Attempt to scan localhost — at minimum the parse should work
        let ip = IpAddr::V4(addrs[0]);
        let timeout = Duration::from_millis(500);

        // Try checking a likely-closed port to validate the check_port logic
        let result = check_port(ip, 39999, timeout).await;
        // Port 39999 is almost certainly closed, so result should be None
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ping_loopback() {
        // Loopback should always respond to ping on supported platforms.
        // If the test runner doesn't have `ping` in PATH the test is skipped.
        if which_ping().is_none() {
            eprintln!("skipping: `ping` binary not available in PATH");
            return;
        }
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let (alive, ttl) = ping_host(ip, Duration::from_millis(2000)).await;
        assert!(alive, "expected loopback to respond to ping");
        assert!(ttl.is_some(), "expected a TTL to be parsed from the loopback reply");
    }

    #[tokio::test]
    async fn test_ping_unreachable() {
        // 192.0.2.0/24 is TEST-NET-1, reserved for documentation; should not respond.
        let ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
        let (alive, ttl) = ping_host(ip, Duration::from_millis(500)).await;
        assert!(!alive, "TEST-NET-1 host must not respond to ping");
        assert!(ttl.is_none());
    }

    fn which_ping() -> Option<std::path::PathBuf> {
        let candidates = if cfg!(windows) {
            vec!["C:\\Windows\\System32\\PING.EXE"]
        } else {
            vec!["/sbin/ping", "/bin/ping", "/usr/bin/ping", "/usr/sbin/ping"]
        };
        candidates.into_iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
    }

    #[test]
    fn test_fileserver_lifecycle() {
        let state = NetworkState::new();

        // Start
        let id = Uuid::new_v4().to_string();
        let info = FileServerInfo {
            id: id.clone(),
            directory: "/tmp/serve".to_string(),
            port: 8080,
            server_type: FileServerType::Http,
            running: true,
            url: "http://0.0.0.0:8080".to_string(),
        };

        {
            let mut servers = state.file_servers.lock().unwrap();
            servers.insert(id.clone(), info);
        }

        // List
        {
            let servers = state.file_servers.lock().unwrap();
            assert_eq!(servers.len(), 1);
            assert!(servers.get(&id).unwrap().running);
        }

        // Stop
        {
            let mut servers = state.file_servers.lock().unwrap();
            servers.remove(&id);
        }

        {
            let servers = state.file_servers.lock().unwrap();
            assert_eq!(servers.len(), 0);
        }
    }

    #[test]
    fn test_tunnel_persistence() {
        let state = NetworkState::new();

        let rule = TunnelRule {
            id: "persist-1".to_string(),
            name: "Persistent Tunnel".to_string(),
            local_port: 3000,
            remote_host: "db.example.com".to_string(),
            remote_port: 5432,
            tunnel_type: TunnelType::Local,
            ssh_session_ref: Some("ssh-session-1".to_string()),
            auto_start: true,
            enabled: true,
        };

        {
            let mut rules = state.tunnel_rules.lock().unwrap();
            rules.push(rule);
        }

        // Verify stored in state
        {
            let rules = state.tunnel_rules.lock().unwrap();
            assert_eq!(rules.len(), 1);
            let stored = &rules[0];
            assert_eq!(stored.id, "persist-1");
            assert_eq!(stored.name, "Persistent Tunnel");
            assert_eq!(stored.local_port, 3000);
            assert_eq!(stored.remote_host, "db.example.com");
            assert_eq!(stored.remote_port, 5432);
            assert!(stored.auto_start);
            assert!(stored.enabled);
            assert!(matches!(stored.tunnel_type, TunnelType::Local));
            assert_eq!(
                stored.ssh_session_ref,
                Some("ssh-session-1".to_string())
            );
        }
    }

    #[test]
    fn test_parse_mac() {
        let mac = parse_mac("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

        let mac2 = parse_mac("aa-bb-cc-dd-ee-ff").unwrap();
        assert_eq!(mac2, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

        assert!(parse_mac("invalid").is_err());
        assert!(parse_mac("GG:HH:II:JJ:KK:LL").is_err());
    }

    #[test]
    fn test_parse_cidr() {
        let addrs = parse_cidr("192.168.1.0/30").unwrap();
        assert_eq!(addrs.len(), 4);

        let single = parse_cidr("10.0.0.1/32").unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0], Ipv4Addr::new(10, 0, 0, 1));

        assert!(parse_cidr("invalid").is_err());
        assert!(parse_cidr("192.168.1.0/33").is_err());
    }

    #[test]
    fn test_service_filter_ports() {
        assert_eq!(ServiceFilter::Ssh.port(), 22);
        assert_eq!(ServiceFilter::Rdp.port(), 3389);
        assert_eq!(ServiceFilter::Vnc.port(), 5900);
        assert_eq!(ServiceFilter::Http.port(), 80);
        assert_eq!(ServiceFilter::Https.port(), 443);
        assert_eq!(ServiceFilter::Telnet.port(), 23);
        assert_eq!(ServiceFilter::Ftp.port(), 21);
        assert_eq!(ServiceFilter::Smb.port(), 445);
        assert_eq!(ServiceFilter::Mysql.port(), 3306);
        assert_eq!(ServiceFilter::Postgresql.port(), 5432);
        assert_eq!(ServiceFilter::Redis.port(), 6379);
        assert_eq!(ServiceFilter::Mongodb.port(), 27017);
        assert_eq!(ServiceFilter::Custom(8080).port(), 8080);
    }

    #[test]
    fn test_suggest_session_type() {
        // SSH takes priority
        let ports = vec![
            OpenPort { port: 22, service_name: "ssh".to_string(), protocol: "tcp".to_string(), ..Default::default() },
            OpenPort { port: 3389, service_name: "rdp".to_string(), protocol: "tcp".to_string(), ..Default::default() },
        ];
        assert_eq!(suggest_session_type(&ports), Some("ssh".to_string()));

        // RDP when no SSH
        let ports_rdp = vec![
            OpenPort { port: 3389, service_name: "rdp".to_string(), protocol: "tcp".to_string(), ..Default::default() },
            OpenPort { port: 80, service_name: "http".to_string(), protocol: "tcp".to_string(), ..Default::default() },
        ];
        assert_eq!(suggest_session_type(&ports_rdp), Some("rdp".to_string()));

        // VNC
        let ports_vnc = vec![
            OpenPort { port: 5900, service_name: "vnc".to_string(), protocol: "tcp".to_string(), ..Default::default() },
        ];
        assert_eq!(suggest_session_type(&ports_vnc), Some("vnc".to_string()));

        // Telnet
        let ports_telnet = vec![
            OpenPort { port: 23, service_name: "telnet".to_string(), protocol: "tcp".to_string(), ..Default::default() },
        ];
        assert_eq!(suggest_session_type(&ports_telnet), Some("telnet".to_string()));

        // No connectable service
        let ports_web = vec![
            OpenPort { port: 80, service_name: "http".to_string(), protocol: "tcp".to_string(), ..Default::default() },
        ];
        assert_eq!(suggest_session_type(&ports_web), None);

        // Empty
        assert_eq!(suggest_session_type(&[]), None);
    }

    #[test]
    fn test_guess_service_ftp() {
        assert_eq!(guess_service(21), "ftp");
        assert_eq!(guess_service(22), "ssh");
        assert_eq!(guess_service(60000), "port-60000");
    }

    #[test]
    fn test_guess_service_new_protocol_breadth_ports() {
        assert_eq!(guess_service(445), "smb");
        assert_eq!(guess_service(513), "rlogin");
        assert_eq!(guess_service(2049), "nfs");
        assert_eq!(guess_service(8006), "proxmox");
    }

    // Regression coverage: suggest_session_type used to return
    // "kubernetes_exec"/"docker_exec"/"mqtt_client"/"grpc_explorer"/
    // "websocket_terminal"/"sftp" (for FTP's port 21) — strings that don't
    // match any guess_service() output for the same port. The frontend
    // matches a suggested type's real port via
    // open_ports.find(p => p.service_name === suggested_session_type); a
    // naming mismatch meant that lookup always missed and silently fell
    // back to a hardcoded port 22, so e.g. a discovered Kubernetes API
    // server would open a "connect" attempt on the wrong port entirely.
    #[test]
    fn test_suggest_session_type_matches_guess_service_naming() {
        let cases: &[(u16, &str)] = &[
            (5985, "winrm"),
            (5986, "winrm-tls"),
            (6443, "kube-api"),
            (2375, "docker-api"),
            (2376, "docker-api-tls"),
            (1883, "mqtt"),
            (8883, "mqtt-tls"),
            (50051, "grpc"),
            (7681, "wsterm"),
            (445, "smb"),
            (2049, "nfs"),
            (513, "rlogin"),
            (8006, "proxmox"),
            (623, "ipmi"),
            (161, "snmp"),
            (21, "ftp"),
        ];
        for &(port, expected) in cases {
            let ports = vec![OpenPort { port, service_name: guess_service(port), protocol: "tcp".to_string(), ..Default::default() }];
            let suggested = suggest_session_type(&ports);
            assert_eq!(suggested.as_deref(), Some(expected), "port {port}");
            // The naming-consistency invariant itself: whatever
            // suggest_session_type returns for a lone open port must equal
            // guess_service's own name for that same port, so the
            // frontend's service_name lookup always finds the real port.
            assert_eq!(suggested.as_deref(), Some(guess_service(port).as_str()), "port {port} naming mismatch");
        }
    }

    #[test]
    fn test_refine_suggested_type_upgrades_generic_https_to_redfish() {
        let open_ports = vec![OpenPort {
            port: 443,
            service_name: "https".to_string(),
            protocol: "tcp".to_string(),
            version: Some("Redfish 1.6.0".to_string()),
            ..Default::default()
        }];
        assert_eq!(refine_suggested_type(None, &open_ports).as_deref(), Some("redfish"));
    }

    #[test]
    fn test_refine_suggested_type_upgrades_generic_http_to_webdav() {
        let open_ports = vec![OpenPort {
            port: 80,
            service_name: "http".to_string(),
            protocol: "tcp".to_string(),
            version: Some("WebDAV".to_string()),
            ..Default::default()
        }];
        assert_eq!(refine_suggested_type(None, &open_ports).as_deref(), Some("webdav"));
    }

    #[test]
    fn test_refine_suggested_type_never_overrides_a_real_protocol_match() {
        // SSH must win even if, say, port 443 on the same host happens to
        // carry a Redfish signature too — a real shell beats a BMC API.
        let open_ports = vec![OpenPort {
            port: 443,
            service_name: "https".to_string(),
            protocol: "tcp".to_string(),
            version: Some("Redfish 1.6.0".to_string()),
            ..Default::default()
        }];
        assert_eq!(refine_suggested_type(Some("ssh".to_string()), &open_ports).as_deref(), Some("ssh"));
    }

    #[test]
    fn test_refine_suggested_type_leaves_generic_http_alone_without_a_signature() {
        let open_ports = vec![OpenPort {
            port: 80,
            service_name: "http".to_string(),
            protocol: "tcp".to_string(),
            version: Some("nginx".to_string()),
            ..Default::default()
        }];
        assert_eq!(refine_suggested_type(None, &open_ports), None);
    }

    #[test]
    fn test_parse_redfish_body_extracts_version() {
        let body = "{\"RedfishVersion\":\"1.6.0\",\"@odata.type\":\"#ServiceRoot.v1_9_0.ServiceRoot\"}";
        assert_eq!(parse_redfish_body(body).as_deref(), Some("Redfish 1.6.0"));
    }

    #[test]
    fn test_parse_redfish_body_accepts_service_root_without_version_field() {
        let body = "{\"@odata.type\":\"#ServiceRoot.v1_9_0.ServiceRoot\",\"Id\":\"RootService\"}";
        assert_eq!(parse_redfish_body(body).as_deref(), Some("Redfish"));
    }

    #[test]
    fn test_parse_redfish_body_rejects_unrelated_json() {
        assert_eq!(parse_redfish_body(r#"{"hello":"world"}"#), None);
        assert_eq!(parse_redfish_body("not json at all"), None);
    }

    #[test]
    fn test_is_webdav_response_detects_dav_header() {
        assert!(is_webdav_response("HTTP/1.1 200 OK\r\nDAV: 1,2\r\n\r\n"));
    }

    #[test]
    fn test_is_webdav_response_detects_propfind_in_allow_header() {
        assert!(is_webdav_response("HTTP/1.1 200 OK\r\nAllow: GET, PROPFIND, MKCOL\r\n\r\n"));
    }

    #[test]
    fn test_is_webdav_response_rejects_generic_http_server() {
        assert!(!is_webdav_response("HTTP/1.1 200 OK\r\nAllow: GET, POST, HEAD\r\n\r\n"));
        assert!(!is_webdav_response("not an http response"));
    }

    #[test]
    fn test_ber_tlv_short_form_length() {
        assert_eq!(ber_tlv(0x04, b"public"), vec![0x04, 0x06, b'p', b'u', b'b', b'l', b'i', b'c']);
        assert_eq!(ber_tlv(0x05, &[]), vec![0x05, 0x00]);
    }

    #[test]
    fn test_build_snmp_get_request_is_well_formed_ber() {
        let packet = build_snmp_get_request("public");
        // Outer SEQUENCE tag + a length byte, at minimum.
        assert_eq!(packet[0], 0x30);
        // version INTEGER 0 (SNMPv1)
        assert_eq!(&packet[2..5], &[0x02, 0x01, 0x00]);
        // community OCTET STRING "public"
        assert_eq!(&packet[5..13], b"\x04\x06public");
        // Contains the sysDescr.0 OID bytes somewhere in the GetRequest PDU.
        let oid = [0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x01, 0x00];
        assert!(packet.windows(oid.len()).any(|w| w == oid));
    }

    #[test]
    fn test_parse_snmp_response_extracts_sys_descr() {
        // Minimal well-formed-enough response carrying an OCTET STRING.
        let mut response = vec![0x30, 0x00];
        response.extend_from_slice(&[0x04, 0x0B]);
        response.extend_from_slice(b"Linux host1");
        assert_eq!(parse_snmp_response(&response).as_deref(), Some("Linux host1"));
    }

    #[test]
    fn test_parse_snmp_response_falls_back_to_generic_label() {
        let response = vec![0x30, 0x03, 0x02, 0x01, 0x00];
        assert_eq!(parse_snmp_response(&response).as_deref(), Some("SNMP agent"));
    }

    #[test]
    fn test_parse_snmp_response_rejects_non_ber_data() {
        assert_eq!(parse_snmp_response(&[0xFF, 0xFF, 0xFF]), None);
        assert_eq!(parse_snmp_response(&[]), None);
    }

    #[test]
    fn test_build_rmcp_presence_ping_structure() {
        let ping = build_rmcp_presence_ping();
        assert_eq!(ping.len(), 12);
        assert_eq!(ping[0], 0x06); // RMCP version
        assert_eq!(ping[3], 0x06); // ASF class
        assert_eq!(&ping[4..8], &[0x00, 0x00, 0x11, 0xBE]); // IANA 4542
        assert_eq!(ping[8], 0x80); // Presence Ping message type
    }

    #[test]
    fn test_is_rmcp_presence_pong_accepts_real_pong_shape() {
        let pong = [0x06, 0x00, 0xFF, 0x06, 0x00, 0x00, 0x11, 0xBE, 0x40, 0x00, 0x00, 0x00];
        assert!(is_rmcp_presence_pong(&pong));
    }

    #[test]
    fn test_is_rmcp_presence_pong_rejects_short_or_wrong_type() {
        assert!(!is_rmcp_presence_pong(&[0x06, 0x00, 0xFF]));
        let wrong_type = [0x06, 0x00, 0xFF, 0x06, 0x00, 0x00, 0x11, 0xBE, 0x80, 0x00, 0x00, 0x00];
        assert!(!is_rmcp_presence_pong(&wrong_type));
    }

    #[tokio::test]
    async fn test_probe_webdav_over_plain_tcp() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nAllow: OPTIONS, GET, PROPFIND, MKCOL\r\nDAV: 1,2\r\n\r\n"
                ).await;
            }
        });

        let result = probe_webdav(addr.ip(), addr.port(), false, Duration::from_millis(500)).await;
        assert_eq!(result, Some(()));
    }

    #[tokio::test]
    async fn test_probe_webdav_rejects_a_plain_web_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nAllow: GET, HEAD, POST\r\n\r\n").await;
            }
        });

        let result = probe_webdav(addr.ip(), addr.port(), false, Duration::from_millis(500)).await;
        assert_eq!(result, None);
    }

    #[test]
    fn test_default_explore_services() {
        // Ensure default list has the 17 core + protocol-breadth + RTSP services
        assert_eq!(DEFAULT_EXPLORE_SERVICES.len(), 17);
        let ports: Vec<u16> = DEFAULT_EXPLORE_SERVICES.iter().map(|s| s.port()).collect();
        assert!(ports.contains(&22));  // ssh
        assert!(ports.contains(&3389)); // rdp
        assert!(ports.contains(&5900)); // vnc
        assert!(ports.contains(&80));  // http
        assert!(ports.contains(&443)); // https
        assert!(ports.contains(&554)); // rtsp — the IP-camera protocol
    }

    #[test]
    fn test_parse_ping_ttl() {
        assert_eq!(parse_ping_ttl("64 bytes from 127.0.0.1: icmp_seq=0 ttl=64 time=0.05 ms"), Some(64));
        assert_eq!(parse_ping_ttl("Reply from 192.168.1.1: bytes=32 time=1ms TTL=128"), Some(128));
        assert_eq!(parse_ping_ttl("no ttl field here"), None);
        assert_eq!(parse_ping_ttl(""), None);
    }

    #[test]
    fn test_summarize_banner() {
        assert_eq!(summarize_banner("SSH-2.0-OpenSSH_9.6"), "OpenSSH 9.6");
        assert_eq!(summarize_banner("SSH-1.99-Cisco-1.25"), "Cisco-1.25");
        assert_eq!(summarize_banner("220 mail.example.com ESMTP"), "220 mail.example.com ESMTP");
    }

    #[test]
    fn test_oui_lookup_known_prefix() {
        // First line of resources/oui-prefixes.txt is 000000 -> XEROX CORPORATION.
        assert_eq!(oui_database().get("000000").map(|s| s.as_str()), Some("XEROX CORPORATION"));
    }

    #[tokio::test]
    async fn test_lookup_mac_vendor_bundled_db() {
        // Uses a MAC whose OUI is bundled, so this resolves offline without
        // falling through to the api.macvendors.com network call.
        let vendor = lookup_mac_vendor("00:00:00:11:22:33").await;
        assert_eq!(vendor.as_deref(), Some("XEROX CORPORATION"));
    }

    #[tokio::test]
    async fn test_grab_banner_reads_first_line() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = stream.write_all(b"SSH-2.0-OpenSSH_9.6\r\nignored second line\r\n").await;
            }
        });

        let banner = grab_banner(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(banner.as_deref(), Some("SSH-2.0-OpenSSH_9.6"));
    }

    #[tokio::test]
    async fn test_probe_rtsp_captures_public_methods() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(
                    b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS, DESCRIBE, SETUP, PLAY\r\n\r\n"
                ).await;
            }
        });

        let result = probe_rtsp(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(result.as_deref(), Some("Public: OPTIONS, DESCRIBE, SETUP, PLAY"));
    }

    #[tokio::test]
    async fn test_probe_jellyfin_extracts_server_name() {
        // Real response body captured from a Jellyfin instance on this LAN.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf).await;
                let body = r#"{"LocalAddress":"http://192.168.0.4:8096","ServerName":"nl.jellyfin","Version":"10.11.11","ProductName":"Jellyfin Server","OperatingSystem":"","Id":"3ce59f3011384575941b9f52868bd68f","StartupWizardCompleted":true}"#;
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let result = probe_jellyfin(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(result.as_deref(), Some("Jellyfin: nl.jellyfin"));
    }

    #[tokio::test]
    async fn test_probe_plex_falls_back_to_generic_label_without_friendly_name() {
        // Real response body captured from a Plex instance on this LAN —
        // note the absence of a friendlyName attribute, which many
        // real-world installs don't expose unauthenticated.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf).await;
                let body = r#"<?xml version="1.0" encoding="UTF-8"?><MediaContainer size="0" apiVersion="1.1.1" claimed="1" machineIdentifier="c71ee46d7d03a1d22474bacc2f03d2f599828e48" version="1.42.2.10156-f737b826c"></MediaContainer>"#;
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let result = probe_plex(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(result.as_deref(), Some("Plex Media Server"));
    }

    #[tokio::test]
    async fn test_probe_plex_extracts_friendly_name_when_present() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf).await;
                let body = r#"<MediaContainer friendlyName="Living Room Plex" machineIdentifier="abc123"></MediaContainer>"#;
                let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let result = probe_plex(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(result.as_deref(), Some("Plex: Living Room Plex"));
    }

    #[tokio::test]
    async fn test_probe_plex_rejects_non_plex_response() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf).await;
                let body = "<html><body>Not Plex</body></html>";
                let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let result = probe_plex(addr.ip(), addr.port(), Duration::from_millis(500)).await;
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_tailscale_status_extracts_self_and_peers() {
        // Trimmed real `tailscale status --json` shape captured on this
        // machine's tailnet.
        let json: serde_json::Value = serde_json::from_str(r#"{
            "Self": {
                "HostName": "Abhishek's MacBook Pro (4)",
                "DNSName": "abhisheks-macbook-pro-4.tailc76fbd.ts.net.",
                "OS": "macOS",
                "TailscaleIPs": ["100.108.219.100", "fd7a:115c:a1e0::b232:db64"],
                "Online": true
            },
            "Peer": {
                "peerkey1": {
                    "HostName": "newserver",
                    "DNSName": "newserver.tailc76fbd.ts.net.",
                    "OS": "linux",
                    "TailscaleIPs": ["100.100.111.101", "fd7a:115c:a1e0::bb32:6f65"],
                    "Online": true
                },
                "peerkey2": {
                    "HostName": "Redmi Pad Pro 5G",
                    "DNSName": "redmi-pad-pro-5g.tailc76fbd.ts.net.",
                    "OS": "android",
                    "TailscaleIPs": ["100.79.163.121", "fd7a:115c:a1e0::a301:a37a"],
                    "Online": false
                }
            }
        }"#).unwrap();

        let peers = parse_tailscale_status(&json);
        assert_eq!(peers.len(), 3);

        let self_peer = peers.iter().find(|p| p.is_self).unwrap();
        assert_eq!(self_peer.ip, "100.108.219.100");
        assert_eq!(self_peer.hostname, "abhisheks-macbook-pro-4.tailc76fbd.ts.net");
        assert_eq!(self_peer.os.as_deref(), Some("macOS"));
        assert!(self_peer.online);

        let newserver = peers.iter().find(|p| p.hostname.starts_with("newserver")).unwrap();
        assert_eq!(newserver.ip, "100.100.111.101");
        assert!(newserver.online);
        assert!(!newserver.is_self);

        let redmi = peers.iter().find(|p| p.hostname.contains("redmi")).unwrap();
        assert!(!redmi.online);
    }

    #[test]
    fn test_parse_tailscale_status_skips_peers_with_no_ipv4() {
        // IPv6-only entries (or malformed ones) shouldn't produce a peer
        // with an empty/garbage IP.
        let json: serde_json::Value = serde_json::from_str(r#"{
            "Peer": {
                "k1": { "HostName": "v6only", "TailscaleIPs": ["fd7a:115c:a1e0::1"] }
            }
        }"#).unwrap();
        assert_eq!(parse_tailscale_status(&json).len(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn debug_live_tailscale_peers() {
        let peers = network_tailscale_peers().await.unwrap();
        for p in &peers {
            eprintln!("{} {} os={:?} online={} self={}", p.ip, p.hostname, p.os, p.online, p.is_self);
        }
        assert!(!peers.is_empty());
    }

    #[test]
    fn test_parse_arp_mac_output_unpadded_octets() {
        // Real macOS output for a MAC with several octets < 0x10 — this
        // exact line is what caused .4/.5/.6/.7/.26/.39/.52 etc. to show a
        // blank MAC/vendor on every single scan, deterministically, not
        // intermittently: a `{2}`-per-octet regex simply never matched it.
        let line = "? (192.168.0.39) at ac:a7:f1:8:6:a7 on en0 ifscope [ethernet]\n";
        assert_eq!(parse_arp_mac_output(line).as_deref(), Some("AC:A7:F1:08:06:A7"));

        let line2 = "? (192.168.0.4) at bc:24:11:41:65:1 on en0 ifscope [ethernet]\n";
        assert_eq!(parse_arp_mac_output(line2).as_deref(), Some("BC:24:11:41:65:01"));

        // Fully-padded input (the common case) still works.
        let line3 = "? (192.168.0.2) at 50:88:11:c3:a2:39 on en0 ifscope [ethernet]\n";
        assert_eq!(parse_arp_mac_output(line3).as_deref(), Some("50:88:11:C3:A2:39"));

        assert_eq!(parse_arp_mac_output("192.168.0.99 (192.168.0.99) -- no entry\n"), None);
    }

    #[test]
    fn test_parse_arp_mac_output_feeds_correct_oui_prefix() {
        // The whole point of zero-padding: lookup_mac_vendor's downstream
        // hex-digit-strip must land on the real OUI prefix, not a
        // misaligned one shifted left by however many digits were dropped.
        let line = "? (192.168.0.26) at 6:17:b6:5:4b:a5 on en0 ifscope [ethernet]\n";
        let mac = parse_arp_mac_output(line).unwrap();
        let clean: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        assert_eq!(&clean[0..6], "0617B6");
    }

    #[test]
    fn test_pick_hostname_from_cert_rejects_bare_wildcard_cn() {
        // Real cert captured from this LAN (a Proxmox host running Heimdall
        // behind nginx, self-signed): subject_cn "*", no SAN at all. Every
        // other cert-derived field (org, issuer, expiry) is still useful,
        // but "*" is not a hostname and must not be surfaced as this host's
        // name — it previously was, verbatim, showing up as the literal
        // string "*" in the Hostname column.
        let wildcard_only = TlsCertInfo {
            subject_cn: Some("*".to_string()),
            subject_org: Some("Linuxserver.io".to_string()),
            issuer_org: Some("Linuxserver.io".to_string()),
            san: vec![],
            not_after: Some("Jan 20 07:43:08 2036 +00:00".to_string()),
        };
        assert_eq!(pick_hostname_from_cert(wildcard_only), None);

        // A wildcard CN with no usable SAN entries either (all wildcards) —
        // still None, not the wildcard.
        let all_wildcard_san = TlsCertInfo {
            subject_cn: Some("*".to_string()),
            subject_org: None,
            issuer_org: None,
            san: vec!["*.example.com".to_string()],
            not_after: None,
        };
        assert_eq!(pick_hostname_from_cert(all_wildcard_san), None);

        // A real SAN entry alongside a wildcard CN: the SAN wins, as before.
        let real_san = TlsCertInfo {
            subject_cn: Some("*".to_string()),
            subject_org: None,
            issuer_org: None,
            san: vec!["*.example.com".to_string(), "nas.local".to_string()],
            not_after: None,
        };
        assert_eq!(pick_hostname_from_cert(real_san).as_deref(), Some("nas.local"));

        // A real (non-wildcard) CN with no SAN at all is still legitimate.
        let real_cn = TlsCertInfo {
            subject_cn: Some("TPRI-DEVICE".to_string()),
            subject_org: Some("TPRI".to_string()),
            issuer_org: Some("TPRI".to_string()),
            san: vec![],
            not_after: None,
        };
        assert_eq!(pick_hostname_from_cert(real_cn).as_deref(), Some("TPRI-DEVICE"));
    }

    #[test]
    fn test_conventional_dot_one() {
        assert_eq!(conventional_dot_one("192.168.0.0/24"), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
        assert_eq!(conventional_dot_one("10.20.30.0/24"), Some(IpAddr::V4(Ipv4Addr::new(10, 20, 30, 1))));
        // Non-zero host bits in the CIDR's network part still resolve off
        // the masked network address, not the literal input.
        assert_eq!(conventional_dot_one("192.168.0.5/24"), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
        // Too small a prefix to have a meaningful "first host".
        assert_eq!(conventional_dot_one("192.168.0.5/31"), None);
        assert_eq!(conventional_dot_one("192.168.0.5/32"), None);
        assert_eq!(conventional_dot_one("not-a-cidr"), None);
    }

    #[tokio::test]
    async fn test_dns_server_registry_grows_from_discovered_hosts() {
        let registry = new_dns_server_registry();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        let with_dns = vec![OpenPort { port: 53, service_name: "dns".to_string(), protocol: "tcp".to_string(), ..Default::default() }];
        let without_dns = vec![OpenPort { port: 22, service_name: "ssh".to_string(), protocol: "tcp".to_string(), ..Default::default() }];

        register_if_dns_server(ip, &without_dns, &registry).await;
        assert!(registry.read().await.is_empty(), "no port 53 open — must not register");

        register_if_dns_server(ip, &with_dns, &registry).await;
        assert!(registry.read().await.contains(&ip), "port 53 open — must register as a candidate DNS server");
    }

    #[test]
    fn test_derive_mdns_hostname_prefers_cast_friendly_name() {
        // Real record captured from this LAN: a MAC registered to "Motorola
        // (Wuhan) Mobility Technologies" (Lenovo's OEM arm) that only
        // resolve_hostname_aggressive's 8 DNS/NetBIOS/ARP/TLS methods can't
        // name at all — but the device's own Cast advertisement says exactly
        // what it is.
        let records = vec![MdnsRecord {
            service_type: "_googlecast._tcp.local.".to_string(),
            instance_name: "LenovoCD-24502F-453e6721".to_string(),
            hostname: Some("453e6721-d97d-f36c-d859.local".to_string()),
            txt: HashMap::from([
                ("fn".to_string(), "Hall clock".to_string()),
                ("md".to_string(), "LenovoCD-24502F".to_string()),
            ]),
        }];
        assert_eq!(derive_mdns_hostname(&records).as_deref(), Some("Hall clock"));

        // No "fn" TXT key: falls back to the advertised hostname.
        let no_fn = vec![MdnsRecord {
            service_type: "_ssh._tcp.local.".to_string(),
            instance_name: "Living Room TV".to_string(),
            hostname: Some("living-room-tv.local".to_string()),
            txt: HashMap::new(),
        }];
        assert_eq!(derive_mdns_hostname(&no_fn).as_deref(), Some("living-room-tv.local"));

        // Neither "fn" nor hostname: falls back to the instance name.
        let instance_only = vec![MdnsRecord {
            service_type: "_home-assistant._tcp.local.".to_string(),
            instance_name: "Home".to_string(),
            hostname: None,
            txt: HashMap::new(),
        }];
        assert_eq!(derive_mdns_hostname(&instance_only).as_deref(), Some("Home"));

        assert_eq!(derive_mdns_hostname(&[]), None);
    }

    #[tokio::test]
    async fn test_enrich_port_dispatches_banner_first() {
        // enrich_port dispatches purely on `open_port.port`, which is also the
        // port it connects to — so unlike the other probe tests, this one has
        // to bind the *real* well-known port (3306/MySQL: in BANNER_FIRST_PORTS,
        // and unlike 21/22/25/110/143 it doesn't need root to bind). Skips
        // gracefully if something else already owns the port on this machine.
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:3306").await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("skipping: could not bind 127.0.0.1:3306 ({e})");
                return;
            }
        };
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = stream.write_all(b"\x4a\x00\x00\x00\x0a8.0.34\x00mysql banner\r\n").await;
            }
        });

        let open_port = OpenPort {
            port: 3306,
            service_name: "mysql".to_string(),
            protocol: "tcp".to_string(),
            ..Default::default()
        };
        let enriched = enrich_port(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            open_port,
            Duration::from_millis(500),
        ).await;

        assert!(enriched.banner.is_some(), "expected a banner to be grabbed from the canned MySQL greeting");
        assert!(enriched.version.is_some());
    }

    /// Debug-only: runs the real scan pipeline (via [`run_explore_and_dump`],
    /// the same function backing the `network-explore-cli` binary) against a
    /// real CIDR with no Tauri State/AppHandle/UI, dumping every
    /// `ExploreResult` field to a JSON file for inspection on disk instead of
    /// re-running the app and screenshotting it. Ignored by default:
    ///   NETWORK_DEBUG_CIDR=192.168.0.0/24 NETWORK_DEBUG_OUT=/path/to/out.json \
    ///     cargo test --lib network::tests::dump_full_scan_debug -- --ignored --nocapture
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    #[ignore]
    async fn dump_full_scan_debug() {
        let cidr = std::env::var("NETWORK_DEBUG_CIDR").unwrap_or_else(|_| "192.168.0.0/24".to_string());
        let out_path = std::env::var("NETWORK_DEBUG_OUT")
            .unwrap_or_else(|_| "/tmp/network_debug_scan.json".to_string());
        run_explore_and_dump(&cidr, None, &[], 1500, &out_path).await.unwrap();
    }
}

// ── Web Relay ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRelayConfig {
    pub bind_addr: String,
    pub auth_token: String,
    pub max_sessions: u32,
    pub tls_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRelayStatus {
    pub running: bool,
    pub bind_addr: String,
    pub active_sessions: u32,
    pub started_at: Option<String>,
}

#[allow(dead_code)]
pub struct WebRelayState {
    config: Arc<Mutex<Option<WebRelayConfig>>>,
    status: Arc<Mutex<WebRelayStatus>>,
}

#[allow(dead_code)]
impl Default for WebRelayState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebRelayState {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(WebRelayStatus {
                running: false,
                bind_addr: String::new(),
                active_sessions: 0,
                started_at: None,
            })),
        }
    }
}

static WEB_RELAY_STATUS: std::sync::OnceLock<Arc<Mutex<WebRelayStatus>>> =
    std::sync::OnceLock::new();

fn get_relay_status() -> Arc<Mutex<WebRelayStatus>> {
    WEB_RELAY_STATUS
        .get_or_init(|| {
            Arc::new(Mutex::new(WebRelayStatus {
                running: false,
                bind_addr: String::new(),
                active_sessions: 0,
                started_at: None,
            }))
        })
        .clone()
}

#[tauri::command]
pub fn network_web_relay_start(
    config: WebRelayConfig,
    _state: tauri::State<NetworkState>,
) -> Result<WebRelayStatus, String> {
    let relay = get_relay_status();
    let mut status = relay.lock().map_err(|e| e.to_string())?;
    status.running = true;
    status.bind_addr = config.bind_addr.clone();
    status.active_sessions = 0;
    status.started_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(status.clone())
}

#[tauri::command]
pub fn network_web_relay_stop(
    _state: tauri::State<NetworkState>,
) -> Result<(), String> {
    let relay = get_relay_status();
    let mut status = relay.lock().map_err(|e| e.to_string())?;
    status.running = false;
    status.started_at = None;
    Ok(())
}

#[tauri::command]
pub fn network_web_relay_status(
    _state: tauri::State<NetworkState>,
) -> Result<WebRelayStatus, String> {
    let relay = get_relay_status();
    let status = relay.lock().map_err(|e| e.to_string())?;
    Ok(status.clone())
}

// ── Web Relay Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod web_relay_tests {
    use super::*;

    /// Create a fresh, isolated WebRelayStatus for each test (avoids shared-state
    /// ordering issues with the module-level OnceLock when tests run in parallel).
    fn fresh_status() -> Arc<Mutex<WebRelayStatus>> {
        Arc::new(Mutex::new(WebRelayStatus {
            running: false,
            bind_addr: String::new(),
            active_sessions: 0,
            started_at: None,
        }))
    }

    #[test]
    fn test_web_relay_initial_status() {
        let relay = fresh_status();
        let status = relay.lock().unwrap();
        assert!(!status.running);
        assert!(status.started_at.is_none());
        assert_eq!(status.active_sessions, 0);
    }

    #[test]
    fn test_web_relay_start_stop() {
        let relay = fresh_status();

        // Simulate start
        {
            let mut status = relay.lock().unwrap();
            status.running = true;
            status.bind_addr = "127.0.0.1:8080".to_string();
            status.started_at = Some(chrono::Utc::now().to_rfc3339());
        }

        {
            let status = relay.lock().unwrap();
            assert!(status.running);
            assert!(status.started_at.is_some());
        }

        // Simulate stop
        {
            let mut status = relay.lock().unwrap();
            status.running = false;
            status.started_at = None;
        }

        {
            let status = relay.lock().unwrap();
            assert!(!status.running);
            assert!(status.started_at.is_none());
        }
    }

    #[test]
    fn test_web_relay_config_reflected() {
        let relay = fresh_status();
        let bind = "0.0.0.0:9090".to_string();

        {
            let mut status = relay.lock().unwrap();
            status.running = true;
            status.bind_addr = bind.clone();
            status.started_at = Some(chrono::Utc::now().to_rfc3339());
        }

        let status = relay.lock().unwrap();
        assert_eq!(status.bind_addr, bind);
        assert!(status.running);
        assert!(status.started_at.is_some());
    }
}

// ── Feature 1: Tunnel Live Metrics ───────────────────────────────────────────

/// Byte-counter and connection metrics for a single tunnel.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TunnelMetrics {
    pub tunnel_id: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub active_connections: u32,
    pub uptime_seconds: u64,
    pub last_activity: Option<String>,
}

// Module-level metrics storage
static TUNNEL_METRICS: std::sync::OnceLock<Arc<Mutex<HashMap<String, TunnelMetrics>>>> =
    std::sync::OnceLock::new();

fn get_tunnel_metrics() -> Arc<Mutex<HashMap<String, TunnelMetrics>>> {
    TUNNEL_METRICS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// Accumulates traffic on a tunnel, called from the relay loops in
/// `ssh::ssh_port_forward_add` (local/dynamic forwards) and
/// `ssh::SshClientHandler::server_channel_open_forwarded_tcpip` (remote
/// forwards) as bytes are read from either side of the tunnel. `tunnel_id`
/// is the same id `PortForwardManager.tsx` already threads through as both
/// `TunnelRule.id` and `PortForward.id`, so no separate id-mapping is needed.
pub fn record_tunnel_bytes(tunnel_id: &str, bytes_in: u64, bytes_out: u64) {
    if tunnel_id.is_empty() {
        return;
    }
    if let Ok(mut map) = get_tunnel_metrics().lock() {
        let entry = map
            .entry(tunnel_id.to_string())
            .or_insert_with(|| TunnelMetrics {
                tunnel_id: tunnel_id.to_string(),
                ..Default::default()
            });
        entry.bytes_in += bytes_in;
        entry.bytes_out += bytes_out;
        entry.last_activity = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Marks one more relayed connection as open on a tunnel (each accepted
/// local/inbound connection on a port-forward gets its own relay task).
pub fn tunnel_connection_opened(tunnel_id: &str) {
    if tunnel_id.is_empty() {
        return;
    }
    if let Ok(mut map) = get_tunnel_metrics().lock() {
        let entry = map
            .entry(tunnel_id.to_string())
            .or_insert_with(|| TunnelMetrics {
                tunnel_id: tunnel_id.to_string(),
                ..Default::default()
            });
        entry.active_connections += 1;
    }
}

/// Marks a relayed connection as closed; called from the relay task's
/// cleanup path so `active_connections` doesn't just grow forever.
pub fn tunnel_connection_closed(tunnel_id: &str) {
    if tunnel_id.is_empty() {
        return;
    }
    if let Ok(mut map) = get_tunnel_metrics().lock() {
        if let Some(entry) = map.get_mut(tunnel_id) {
            entry.active_connections = entry.active_connections.saturating_sub(1);
        }
    }
}

#[tauri::command]
pub fn network_tunnel_metrics(
    tunnel_id: String,
    _state: tauri::State<NetworkState>,
) -> Result<TunnelMetrics, String> {
    let arc = get_tunnel_metrics();
    let map = arc.lock().map_err(|e| e.to_string())?;
    Ok(map.get(&tunnel_id).cloned().unwrap_or_else(|| TunnelMetrics {
        tunnel_id: tunnel_id.clone(),
        ..Default::default()
    }))
}

#[tauri::command]
pub fn network_tunnel_metrics_all(
    _state: tauri::State<NetworkState>,
) -> Result<Vec<TunnelMetrics>, String> {
    let arc = get_tunnel_metrics();
    let map = arc.lock().map_err(|e| e.to_string())?;
    Ok(map.values().cloned().collect())
}

#[tauri::command]
pub fn network_tunnel_metrics_reset(
    tunnel_id: String,
    _state: tauri::State<NetworkState>,
) -> Result<(), String> {
    let arc = get_tunnel_metrics();
    let mut map = arc.lock().map_err(|e| e.to_string())?;
    map.remove(&tunnel_id);
    Ok(())
}

// ── Feature 2: Tunnel Health Events ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelHealthStatus {
    Active,
    Degraded,
    Dropped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelHealthEvent {
    pub tunnel_id: String,
    pub status: TunnelHealthStatus,
    pub message: String,
    pub timestamp: String,
}

#[allow(dead_code)]
pub fn emit_tunnel_health(
    app: &tauri::AppHandle,
    tunnel_id: &str,
    status: TunnelHealthStatus,
    message: &str,
) {
    let event = TunnelHealthEvent {
        tunnel_id: tunnel_id.to_string(),
        status,
        message: message.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("tunnel_health", event);
}

#[tauri::command]
pub fn network_tunnel_health_check(
    tunnel_id: String,
    _state: tauri::State<NetworkState>,
) -> Result<TunnelHealthEvent, String> {
    // Stub: returns Active for any tunnel_id; real implementation would
    // ping the tunnel endpoint and check byte flow
    Ok(TunnelHealthEvent {
        tunnel_id: tunnel_id.clone(),
        status: TunnelHealthStatus::Active,
        message: "tunnel_health_check_requires_live_connection".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

// ── Tests: Tunnel Metrics & Health ───────────────────────────────────────────

#[cfg(test)]
mod tunnel_tests {
    use super::*;

    // Helper: isolated metrics map so tests don't share global state.
    fn fresh_metrics() -> Arc<Mutex<HashMap<String, TunnelMetrics>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn record_bytes_into(
        map: &Arc<Mutex<HashMap<String, TunnelMetrics>>>,
        tunnel_id: &str,
        bytes_in: u64,
        bytes_out: u64,
    ) {
        if let Ok(mut m) = map.lock() {
            let entry = m
                .entry(tunnel_id.to_string())
                .or_insert_with(|| TunnelMetrics {
                    tunnel_id: tunnel_id.to_string(),
                    ..Default::default()
                });
            entry.bytes_in += bytes_in;
            entry.bytes_out += bytes_out;
            entry.last_activity = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    // ── Metrics tests ─────────────────────────────────────────────────────

    #[test]
    fn test_record_tunnel_bytes() {
        let map = fresh_metrics();
        record_bytes_into(&map, "tun-1", 100, 200);
        record_bytes_into(&map, "tun-1", 50, 75);

        let m = map.lock().unwrap();
        let entry = m.get("tun-1").expect("metrics for tun-1 must exist");
        assert_eq!(entry.bytes_in, 150, "bytes_in should accumulate");
        assert_eq!(entry.bytes_out, 275, "bytes_out should accumulate");
        assert!(entry.last_activity.is_some(), "last_activity must be set");
    }

    #[test]
    fn test_tunnel_metrics_default_when_missing() {
        let map = fresh_metrics();
        let m = map.lock().unwrap();
        let entry = m
            .get("nonexistent")
            .cloned()
            .unwrap_or_else(|| TunnelMetrics {
                tunnel_id: "nonexistent".to_string(),
                ..Default::default()
            });
        assert_eq!(entry.tunnel_id, "nonexistent");
        assert_eq!(entry.bytes_in, 0);
        assert_eq!(entry.bytes_out, 0);
        assert_eq!(entry.active_connections, 0);
        assert_eq!(entry.uptime_seconds, 0);
        assert!(entry.last_activity.is_none());
    }

    #[test]
    fn test_tunnel_metrics_all() {
        let map = fresh_metrics();
        record_bytes_into(&map, "tun-a", 1000, 2000);
        record_bytes_into(&map, "tun-b", 500, 800);

        let m = map.lock().unwrap();
        let all: Vec<TunnelMetrics> = m.values().cloned().collect();
        assert_eq!(all.len(), 2, "all() should return exactly 2 entries");

        let ids: Vec<&str> = all.iter().map(|e| e.tunnel_id.as_str()).collect();
        assert!(ids.contains(&"tun-a"), "tun-a should be present");
        assert!(ids.contains(&"tun-b"), "tun-b should be present");
    }

    // The tests above exercise `record_tunnel_bytes`'s accumulation logic
    // against an isolated map rather than the real function, to avoid
    // sharing the process-wide TUNNEL_METRICS static across parallel test
    // threads. The tests below exercise the real, production
    // `record_tunnel_bytes`/`tunnel_connection_opened`/`_closed` functions
    // directly (this is what ssh/mod.rs's relay loops actually call) —
    // each uses a tunnel_id unique to this test so concurrent tests can't
    // collide on the shared global map.

    #[test]
    fn test_real_record_tunnel_bytes_accumulates_across_calls() {
        let id = "test-real-record-tunnel-bytes";
        record_tunnel_bytes(id, 100, 50);
        record_tunnel_bytes(id, 20, 5);

        let metrics = get_tunnel_metrics();
        let map = metrics.lock().unwrap();
        let entry = map.get(id).expect("entry must exist after recording");
        assert_eq!(entry.bytes_in, 120);
        assert_eq!(entry.bytes_out, 55);
        assert!(entry.last_activity.is_some());
    }

    #[test]
    fn test_real_record_tunnel_bytes_ignores_an_empty_tunnel_id() {
        // The Remote-forward fallback in ssh/mod.rs can pass an empty
        // forward_id if a lookup somehow misses; this must not create a
        // bogus "" entry in the shared metrics map.
        record_tunnel_bytes("", 10, 10);
        let metrics = get_tunnel_metrics();
        let map = metrics.lock().unwrap();
        assert!(map.get("").is_none());
    }

    #[test]
    fn test_real_tunnel_connection_opened_and_closed_track_active_connections() {
        let id = "test-real-tunnel-connection-lifecycle";
        tunnel_connection_opened(id);
        tunnel_connection_opened(id);
        {
            let metrics = get_tunnel_metrics();
            let map = metrics.lock().unwrap();
            assert_eq!(map.get(id).unwrap().active_connections, 2);
        }

        tunnel_connection_closed(id);
        let metrics = get_tunnel_metrics();
        let map = metrics.lock().unwrap();
        assert_eq!(map.get(id).unwrap().active_connections, 1);
    }

    #[test]
    fn test_real_tunnel_connection_closed_saturates_at_zero() {
        let id = "test-real-tunnel-connection-closed-saturates";
        tunnel_connection_opened(id);
        tunnel_connection_closed(id);
        tunnel_connection_closed(id); // one more than was opened

        let metrics = get_tunnel_metrics();
        let map = metrics.lock().unwrap();
        assert_eq!(map.get(id).unwrap().active_connections, 0);
    }

    #[test]
    fn test_real_tunnel_connection_closed_is_a_noop_for_an_unknown_tunnel() {
        // Must not panic or create a spurious entry.
        tunnel_connection_closed("test-real-tunnel-never-opened");
        let metrics = get_tunnel_metrics();
        let map = metrics.lock().unwrap();
        assert!(map.get("test-real-tunnel-never-opened").is_none());
    }

    // ── Health tests ──────────────────────────────────────────────────────

    fn health_check_logic(tunnel_id: &str) -> TunnelHealthEvent {
        TunnelHealthEvent {
            tunnel_id: tunnel_id.to_string(),
            status: TunnelHealthStatus::Active,
            message: "stub".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_tunnel_health_event_serialize() {
        let event = TunnelHealthEvent {
            tunnel_id: "tun-serialize".to_string(),
            status: TunnelHealthStatus::Active,
            message: "ok".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize must succeed");
        assert!(
            json.contains("tunnel_id"),
            "serialized JSON must contain 'tunnel_id' key"
        );
        assert!(json.contains("tun-serialize"), "tunnel_id value must be present");
    }

    #[test]
    fn test_tunnel_health_check_stub() {
        let event = health_check_logic("tun-stub");
        assert_eq!(event.tunnel_id, "tun-stub");
        // Status must be the Active variant
        assert!(
            matches!(event.status, TunnelHealthStatus::Active),
            "stub must return Active status"
        );
    }
}

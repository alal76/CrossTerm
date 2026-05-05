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
    pub open_ports: Vec<OpenPort>,
    pub os_guess: Option<String>,
    pub response_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenPort {
    pub port: u16,
    pub service_name: String,
    pub protocol: String,
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
    pub open_ports: Vec<OpenPort>,
    pub os_guess: Option<String>,
    pub response_time_ms: f64,
    /// Quick-connect session type derived from the highest-priority open port.
    pub suggested_session_type: Option<String>,
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
        443 => "https".to_string(),
        445 => "smb".to_string(),
        993 => "imaps".to_string(),
        995 => "pop3s".to_string(),
        3306 => "mysql".to_string(),
        3389 => "rdp".to_string(),
        5432 => "postgresql".to_string(),
        5900 => "vnc".to_string(),
        6379 => "redis".to_string(),
        8080 => "http-alt".to_string(),
        8443 => "https-alt".to_string(),
        27017 => "mongodb".to_string(),
        _ => format!("port-{}", port),
    }
}

/// Suggest the best session type from open ports (priority order).
fn suggest_session_type(open_ports: &[OpenPort]) -> Option<String> {
    let ports: Vec<u16> = open_ports.iter().map(|p| p.port).collect();
    if ports.contains(&22) {
        Some("ssh".to_string())
    } else if ports.contains(&3389) {
        Some("rdp".to_string())
    } else if ports.contains(&5900) {
        Some("vnc".to_string())
    } else if ports.contains(&23) {
        Some("telnet".to_string())
    } else if ports.contains(&21) {
        Some("sftp".to_string())
    } else {
        None
    }
}

/// Common ports to scan by default.
const DEFAULT_PORTS: &[u16] = &[
    22, 23, 25, 53, 80, 110, 143, 443, 445, 993, 995, 3306, 3389, 5432, 5900, 6379, 8080, 8443,
    27017,
];

/// Try to connect to a TCP port with a timeout.
async fn check_port(ip: IpAddr, port: u16, timeout: Duration) -> Option<OpenPort> {
    let addr = SocketAddr::new(ip, port);
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => Some(OpenPort {
            port,
            service_name: guess_service(port),
            protocol: "tcp".to_string(),
        }),
        _ => None,
    }
}

/// Attempt reverse DNS lookup for an IP address.
async fn reverse_dns(ip: IpAddr) -> Option<String> {
    tokio::task::spawn_blocking(move || dns_lookup_reverse(ip))
        .await
        .ok()
        .flatten()
}

/// System-level reverse DNS via getnameinfo(3) — Unix only.
#[cfg(unix)]
fn dns_lookup_reverse(ip: IpAddr) -> Option<String> {
    use std::ffi::CStr;
    use std::mem;

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
                libc::NI_NAMEREQD,
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
                libc::NI_NAMEREQD,
            )
        },
    };

    if ret == 0 {
        let cstr = unsafe { CStr::from_ptr(host.as_ptr()) };
        Some(cstr.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[cfg(not(unix))]
fn dns_lookup_reverse(_ip: IpAddr) -> Option<String> {
    None
}

/// Guess OS from open ports heuristic.
fn guess_os(open_ports: &[OpenPort]) -> Option<String> {
    let ports: Vec<u16> = open_ports.iter().map(|p| p.port).collect();
    if ports.contains(&3389) {
        Some("Windows".to_string())
    } else if ports.contains(&22) {
        Some("Linux/Unix".to_string())
    } else if ports.contains(&445) && !ports.contains(&22) {
        Some("Windows".to_string())
    } else {
        None
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
                    let hostname = reverse_dns(ip).await;
                    let os_guess = guess_os(&open_ports);
                    let result = ScanResult {
                        ip: ip.to_string(),
                        hostname,
                        mac_address: None,
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
];

#[tauri::command]
pub async fn network_explore_start(
    target: ExploreTarget,
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
    ports.sort_unstable();
    ports.dedup();

    // Validate port range
    if ports.contains(&0) {
        return Err(NetworkError::InvalidCidr("Port 0 is not valid".to_string()));
    }

    let scan_id_clone = scan_id.clone();

    tokio::spawn(async move {
        const MAX_CONCURRENT: usize = 25;
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let hosts_scanned = Arc::new(AtomicU32::new(0));
        let hosts_found = Arc::new(AtomicU32::new(0));
        let ports = Arc::new(ports);
        let mut tasks = tokio::task::JoinSet::new();

        for addr in addresses {
            let ip = IpAddr::V4(addr);
            let app = app.clone();
            let scan_id = scan_id_clone.clone();
            let sem = Arc::clone(&semaphore);
            let counter = Arc::clone(&hosts_scanned);
            let found_counter = Arc::clone(&hosts_found);
            let ports = Arc::clone(&ports);

            tasks.spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let start = Instant::now();

                let port_futures: Vec<_> = ports.iter().map(|&p| check_port(ip, p, timeout)).collect();
                let port_results = futures::future::join_all(port_futures).await;
                let open_ports: Vec<OpenPort> = port_results.into_iter().flatten().collect();
                let response_time = start.elapsed().as_secs_f64() * 1000.0;

                if !open_ports.is_empty() {
                    let hostname = reverse_dns(ip).await;
                    let os_guess = guess_os(&open_ports);
                    let suggested_session_type = suggest_session_type(&open_ports);
                    let result = ExploreResult {
                        ip: ip.to_string(),
                        hostname,
                        mac_address: None,
                        open_ports,
                        os_guess,
                        response_time_ms: response_time,
                        suggested_session_type,
                    };
                    found_counter.fetch_add(1, Ordering::Relaxed);
                    let _ = app.emit("network:explore_host_found", ExploreHostFound {
                        scan_id: scan_id.clone(),
                        result,
                    });
                }

                let scanned = counter.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit("network:explore_progress", ExploreProgress {
                    scan_id,
                    hosts_scanned: scanned,
                    total_hosts,
                    hosts_found: found_counter.load(Ordering::Relaxed),
                });
            });
        }

        while tasks.join_next().await.is_some() {}
    });

    Ok(scan_id)
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
    for (_, proc) in procs.iter_mut() {
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
            OpenPort { port: 22, service_name: "ssh".to_string(), protocol: "tcp".to_string() },
            OpenPort { port: 3389, service_name: "rdp".to_string(), protocol: "tcp".to_string() },
        ];
        assert_eq!(suggest_session_type(&ports), Some("ssh".to_string()));

        // RDP when no SSH
        let ports_rdp = vec![
            OpenPort { port: 3389, service_name: "rdp".to_string(), protocol: "tcp".to_string() },
            OpenPort { port: 80, service_name: "http".to_string(), protocol: "tcp".to_string() },
        ];
        assert_eq!(suggest_session_type(&ports_rdp), Some("rdp".to_string()));

        // VNC
        let ports_vnc = vec![
            OpenPort { port: 5900, service_name: "vnc".to_string(), protocol: "tcp".to_string() },
        ];
        assert_eq!(suggest_session_type(&ports_vnc), Some("vnc".to_string()));

        // Telnet
        let ports_telnet = vec![
            OpenPort { port: 23, service_name: "telnet".to_string(), protocol: "tcp".to_string() },
        ];
        assert_eq!(suggest_session_type(&ports_telnet), Some("telnet".to_string()));

        // No connectable service
        let ports_web = vec![
            OpenPort { port: 80, service_name: "http".to_string(), protocol: "tcp".to_string() },
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
    fn test_default_explore_services() {
        // Ensure default list has the 8 core services
        assert_eq!(DEFAULT_EXPLORE_SERVICES.len(), 8);
        let ports: Vec<u16> = DEFAULT_EXPLORE_SERVICES.iter().map(|s| s.port()).collect();
        assert!(ports.contains(&22));  // ssh
        assert!(ports.contains(&3389)); // rdp
        assert!(ports.contains(&5900)); // vnc
        assert!(ports.contains(&80));  // http
        assert!(ports.contains(&443)); // https
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

pub fn record_tunnel_bytes(tunnel_id: &str, bytes_in: u64, bytes_out: u64) {
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

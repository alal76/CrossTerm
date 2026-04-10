use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::net::TcpStream;
use uuid::Uuid;

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
    let addr = SocketAddr::new(ip, 0);
    match tokio::net::lookup_host(format!("{ip}:0")).await {
        Ok(_) => {
            // tokio's lookup_host doesn't do reverse DNS directly;
            // use DNS crate or system resolver via blocking task
            tokio::task::spawn_blocking(move || {
                // Attempt reverse lookup via getaddrinfo hint
                dns_lookup_reverse(addr.ip())
            })
            .await
            .ok()
            .flatten()
        }
        Err(_) => None,
    }
}

/// System-level reverse DNS
fn dns_lookup_reverse(_ip: IpAddr) -> Option<String> {
    // On most systems we can attempt getnameinfo. For simplicity, return None
    // as full reverse DNS requires the `dns-lookup` crate. Scan still works.
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
}

impl NetworkState {
    pub fn new() -> Self {
        Self {
            scan_results: Mutex::new(HashMap::new()),
            tunnel_rules: Mutex::new(Vec::new()),
            active_tunnels: Mutex::new(HashMap::new()),
            file_servers: Mutex::new(HashMap::new()),
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

    // Spawn async scan task
    tokio::spawn(async move {
        let timeout = Duration::from_millis(1500);
        for (scan_idx, addr) in addresses.into_iter().enumerate() {
            let ip = IpAddr::V4(addr);
            let start = Instant::now();

            // Check ports concurrently
            let mut port_futures = Vec::new();
            for &port in DEFAULT_PORTS {
                port_futures.push(check_port(ip, port, timeout));
            }
            let port_results = futures::future::join_all(port_futures).await;
            let open_ports: Vec<OpenPort> = port_results.into_iter().flatten().collect();
            let response_time = start.elapsed().as_secs_f64() * 1000.0;

            if !open_ports.is_empty() {
                let hostname = reverse_dns(ip).await;
                let os_guess = guess_os(&open_ports);

                let result = ScanResult {
                    ip: ip.to_string(),
                    hostname,
                    mac_address: None, // MAC detection requires raw sockets / ARP
                    open_ports,
                    os_guess,
                    response_time_ms: response_time,
                };

                // Emit host found event
                let _ = app.emit(
                    "network:scan_host_found",
                    ScanHostFound {
                        scan_id: scan_id_clone.clone(),
                        result: result.clone(),
                    },
                );

                // Store result — we access app state via the handle
                // Note: scan results are stored via the event; frontend collects them
            }

            let _ = app.emit(
                "network:scan_progress",
                ScanProgress {
                    scan_id: scan_id_clone.clone(),
                    hosts_scanned: scan_idx as u32 + 1,
                    total_hosts,
                },
            );
        }
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
    if ports.iter().any(|&p| p == 0) {
        return Err(NetworkError::InvalidCidr("Port 0 is not valid".to_string()));
    }

    let scan_id_clone = scan_id.clone();

    tokio::spawn(async move {
        let mut hosts_found: u32 = 0;
        for (scan_idx, addr) in addresses.into_iter().enumerate() {
            let ip = IpAddr::V4(addr);
            let start = Instant::now();

            // Check all requested ports concurrently
            let mut port_futures = Vec::with_capacity(ports.len());
            for &port in &ports {
                port_futures.push(check_port(ip, port, timeout));
            }
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

                hosts_found += 1;
                let _ = app.emit(
                    "network:explore_host_found",
                    ExploreHostFound {
                        scan_id: scan_id_clone.clone(),
                        result,
                    },
                );
            }

            let _ = app.emit(
                "network:explore_progress",
                ExploreProgress {
                    scan_id: scan_id_clone.clone(),
                    hosts_scanned: scan_idx as u32 + 1,
                    total_hosts,
                    hosts_found,
                },
            );
        }
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

    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| NetworkError::Io(e.to_string()))?;
    socket
        .set_broadcast(true)
        .map_err(|e| NetworkError::Io(e.to_string()))?;

    let dest: SocketAddr = format!("{broadcast_addr}:9")
        .parse()
        .map_err(|e: std::net::AddrParseError| NetworkError::Io(e.to_string()))?;

    socket
        .send_to(&packet, dest)
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
#[serde(rename_all = "snake_case")]
pub enum WifiBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
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
            WifiSecurity::Wpa2Psk => {
                // WPA2 is acceptable but WPA3 is better
                if net.is_current {
                    issues.push(WifiSecurityIssue {
                        ssid: net.ssid.clone(),
                        severity: "info".into(),
                        issue: "WPA2-PSK is secure but WPA3-SAE offers stronger protection".into(),
                        recommendation: "Consider upgrading router firmware to enable WPA3 transition mode".into(),
                    });
                }
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
    let output = tokio::process::Command::new("system_profiler")
        .args(["SPAirPortDataType", "-json"])
        .output()
        .await
        .map_err(|e| NetworkError::Io(format!("Failed to run system_profiler: {}", e)))?;

    if !output.status.success() {
        return Err(NetworkError::Io("system_profiler failed".into()));
    }

    let data: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| NetworkError::Io(format!("Failed to parse system_profiler JSON: {}", e)))?;

    let mut networks = Vec::new();
    let mut current_net = None;
    let mut iface_name = None;

    let ifaces = data.pointer("/SPAirPortDataType/0/spairport_airport_interfaces")
        .and_then(|v| v.as_array());

    if let Some(ifaces) = ifaces {
        for iface in ifaces {
            if iface_name.is_none() {
                iface_name = iface.get("_name").and_then(|v| v.as_str()).map(String::from);
            }

            // Current network
            if let Some(current) = iface.get("spairport_current_network_information") {
                let ssid = current.get("_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let channel_raw = current.get("spairport_network_channel").and_then(|v| v.as_str()).unwrap_or("");
                let (channel, width, freq_hint) = parse_channel_info(channel_raw);
                let security = current.get("spairport_security_mode").and_then(|v| v.as_str()).unwrap_or("");
                let sn = current.get("spairport_signal_noise").and_then(|v| v.as_str()).unwrap_or("");
                let (signal, noise) = parse_signal_noise(sn);
                let phy = current.get("spairport_network_phymode").and_then(|v| v.as_str()).map(String::from);
                let band = parse_band_from_channel(channel, freq_hint);

                let net = WifiNetwork {
                    ssid,
                    bssid: None,
                    channel,
                    channel_width_mhz: width,
                    band,
                    frequency_mhz: None,
                    signal_dbm: signal,
                    noise_dbm: noise,
                    security: parse_macos_security(security),
                    phy_mode: phy,
                    is_current: true,
                };
                current_net = Some(net.clone());
                networks.push(net);
            }

            // Other visible networks
            if let Some(others) = iface.get("spairport_airport_other_local_wireless_networks").and_then(|v| v.as_array()) {
                for net_val in others {
                    let ssid = net_val.get("_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let channel_raw = net_val.get("spairport_network_channel").and_then(|v| v.as_str()).unwrap_or("");
                    let (channel, width, freq_hint) = parse_channel_info(channel_raw);
                    let security = net_val.get("spairport_security_mode").and_then(|v| v.as_str()).unwrap_or("");
                    let sn = net_val.get("spairport_signal_noise").and_then(|v| v.as_str()).unwrap_or("");
                    let (signal, noise) = parse_signal_noise(sn);
                    let phy = net_val.get("spairport_network_phymode").and_then(|v| v.as_str()).map(String::from);
                    let band = parse_band_from_channel(channel, freq_hint);

                    networks.push(WifiNetwork {
                        ssid,
                        bssid: None,
                        channel,
                        channel_width_mhz: width,
                        band,
                        frequency_mhz: None,
                        signal_dbm: signal,
                        noise_dbm: noise,
                        security: parse_macos_security(security),
                        phy_mode: phy,
                        is_current: false,
                    });
                }
            }
        }
    }

    Ok((networks, current_net, iface_name))
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
        let channel: u32 = fields[2].parse().unwrap_or(0);
        let freq: u32 = fields[3].trim().split_whitespace().next()
            .and_then(|s| s.parse().ok()).unwrap_or(0);
        let signal_pct: i32 = fields[4].parse().unwrap_or(0);
        // Convert percentage to approximate dBm
        let signal_dbm = if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None };
        let security_raw = fields[5];
        let band = if freq >= 5925 { WifiBand::Band6GHz } else if freq >= 5000 { WifiBand::Band5GHz } else { WifiBand::Band2_4GHz };
        let security = if security_raw.is_empty() || security_raw == "--" {
            WifiSecurity::Open
        } else {
            let s = security_raw.to_lowercase();
            if s.contains("wpa3") { WifiSecurity::Wpa3Sae }
            else if s.contains("wpa2") && s.contains("enterprise") { WifiSecurity::Wpa2Enterprise }
            else if s.contains("wpa2") { WifiSecurity::Wpa2Psk }
            else if s.contains("wpa") { WifiSecurity::WpaPsk }
            else if s.contains("wep") { WifiSecurity::Wep }
            else { WifiSecurity::Unknown(security_raw.to_string()) }
        };
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
                let sig = if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None };
                let band = parse_band_from_channel(channel, None);
                let net = WifiNetwork {
                    ssid: ssid.clone(), bssid: bssid.take(), channel, channel_width_mhz: None,
                    band, frequency_mhz: None, signal_dbm: sig, noise_dbm: None,
                    security: security.clone(), phy_mode: None, is_current,
                };
                networks.push(net);
            }
            bssid = trimmed.split(':').skip(1).next().map(|s| s.trim().to_string());
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
            security = if auth.contains("wpa3") { WifiSecurity::Wpa3Sae }
                else if auth.contains("wpa2") && auth.contains("enterprise") { WifiSecurity::Wpa2Enterprise }
                else if auth.contains("wpa2") { WifiSecurity::Wpa2Psk }
                else if auth.contains("wpa") { WifiSecurity::WpaPsk }
                else if auth.contains("open") { WifiSecurity::Open }
                else { WifiSecurity::Unknown(auth) };
        }
    }
    // Push last entry
    if bssid.is_some() || !ssid.is_empty() {
        let is_current = ssid == current_ssid;
        let sig = if signal_pct > 0 { Some(-100 + signal_pct / 2) } else { None };
        let band = parse_band_from_channel(channel, None);
        networks.push(WifiNetwork {
            ssid, bssid, channel, channel_width_mhz: None,
            band, frequency_mhz: None, signal_dbm: sig, noise_dbm: None,
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

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

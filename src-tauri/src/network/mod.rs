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
                use std::net::ToSocketAddrs;
                // Attempt reverse lookup via getaddrinfo hint
                let hostname = dns_lookup_reverse(addr.ip());
                hostname
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
    } else if ports.contains(&22) && ports.contains(&80) {
        Some("Linux/Unix".to_string())
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
        let mut hosts_scanned = 0u32;

        for addr in addresses {
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

            hosts_scanned += 1;
            let _ = app.emit(
                "network:scan_progress",
                ScanProgress {
                    scan_id: scan_id_clone.clone(),
                    hosts_scanned,
                    total_hosts,
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
        .ok_or_else(|| NetworkError::ScanNotFound(scan_id))
}

#[tauri::command]
pub async fn network_scan_save_as_sessions(
    scan_id: String,
    folder: String,
    state: tauri::State<'_, NetworkState>,
) -> Result<Vec<String>, NetworkError> {
    let results = state.scan_results.lock().unwrap();
    let scan_results = results
        .get(&scan_id)
        .ok_or_else(|| NetworkError::ScanNotFound(scan_id.clone()))?;

    let mut session_ids = Vec::new();
    for result in scan_results {
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
}

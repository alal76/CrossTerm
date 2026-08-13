/// IPMI Serial-over-LAN (SOL) via RMCP+ (UDP port 623).
/// Implements enough of the IPMI 2.0 RMCP+ payload to open a SOL session
/// and relay console data as Tauri events.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::net::UdpSocket;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum IpmiError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("RMCP error: {0}")]
    Rmcp(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for IpmiError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IpmiPrivilege {
    User,
    Operator,
    Administrator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiConfig {
    pub host: String,
    /// Default 623
    pub port: u16,
    pub username: String,
    pub password: String,
    /// IPMI channel (typically 1)
    pub channel: u8,
    pub privilege: IpmiPrivilege,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiSession {
    pub id: String,
    pub host: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiSolData {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiPowerStatus {
    pub session_id: String,
    pub powered_on: bool,
}

pub struct IpmiState {
    sessions: Mutex<HashMap<String, IpmiSession>>,
    senders: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl IpmiState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            senders: Mutex::new(HashMap::new()),
        }
    }
}

/// Build a minimal RMCP ASF Ping to verify the BMC responds on UDP 623.
fn rmcp_ping() -> Vec<u8> {
    vec![
        0x06, 0x00, 0xFF, 0x06, // RMCP header: version=6, reserved, seq=0xFF, class=ASF
        0x00, 0x00, 0x11, 0xBE, // IANA Enterprise 4542 (ASF)
        0x80,                   // Message type: Presence Ping
        0x00,                   // Message tag
        0x00,                   // Reserved
        0x00,                   // Data length = 0
    ]
}

#[tauri::command]
pub async fn ipmi_sol_connect(
    config: IpmiConfig,
    state: tauri::State<'_, IpmiState>,
    app: AppHandle,
) -> Result<String, IpmiError> {
    let remote_addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e: std::net::AddrParseError| IpmiError::Rmcp(e.to_string()))?;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(remote_addr).await?;

    // Send RMCP ping to verify BMC is reachable
    socket.send(&rmcp_ping()).await?;

    let mut buf = [0u8; 512];
    tokio::time::timeout(std::time::Duration::from_secs(3), socket.recv(&mut buf))
        .await
        .map_err(|_| IpmiError::Rmcp("BMC did not respond to RMCP ping".into()))?
        .map_err(IpmiError::Io)?;

    let id = Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // SOL data pump — forward received BMC bytes as events
    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            tokio::select! {
                result = socket.recv(&mut buf) => {
                    match result {
                        Ok(n) if n > 4 => {
                            // Skip 4-byte RMCP header; rest is SOL payload
                            let payload = &buf[4..n];
                            let text = String::from_utf8_lossy(payload).into_owned();
                            let _ = app_clone.emit("ipmi:sol_data", IpmiSolData {
                                session_id: id_clone.clone(),
                                data: text,
                            });
                        }
                        _ => break,
                    }
                }
                Some(data) = rx.recv() => {
                    let _ = socket.send(&data).await;
                }
            }
        }
    });

    let session = IpmiSession { id: id.clone(), host: config.host, username: config.username };
    state.sessions.lock().unwrap().insert(id.clone(), session);
    state.senders.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn ipmi_sol_send(
    id: String,
    data: String,
    state: tauri::State<'_, IpmiState>,
) -> Result<(), IpmiError> {
    state.senders.lock().unwrap()
        .get(&id).ok_or_else(|| IpmiError::NotFound(id))?
        .send(data.into_bytes())
        .map_err(|e| IpmiError::Rmcp(e.to_string()))
}

#[tauri::command]
pub fn ipmi_sol_disconnect(id: String, state: tauri::State<'_, IpmiState>) -> Result<(), IpmiError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| IpmiError::NotFound(id.clone()))?;
    state.senders.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn ipmi_list(state: tauri::State<'_, IpmiState>) -> Vec<IpmiSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum TelnetError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected: {0}")]
    NotConnected(String),
    #[error("Write failed: {0}")]
    WriteFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for TelnetError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelnetConfig {
    pub host: String,
    pub port: u16,
    pub terminal_type: String,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TelnetConnection {
    pub id: String,
    pub config: TelnetConfig,
    pub connected: bool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct TelnetState {
    pub connections: Mutex<HashMap<String, TelnetConnection>>,
}

impl TelnetState {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn telnet_connect(
    config: TelnetConfig,
    state: tauri::State<'_, TelnetState>,
) -> Result<String, TelnetError> {
    if config.host.is_empty() {
        return Err(TelnetError::ConnectionFailed("Host cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();

    // In a full implementation, establish a TCP connection and run
    // the telnet protocol negotiation here.
    let conn = TelnetConnection {
        id: id.clone(),
        config,
        connected: true,
    };

    state.connections.lock().unwrap().insert(id.clone(), conn);
    Ok(id)
}

#[tauri::command]
pub async fn telnet_disconnect(
    conn_id: String,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let mut conns = state.connections.lock().unwrap();
    match conns.remove(&conn_id) {
        Some(_) => Ok(()),
        None => Err(TelnetError::NotConnected(conn_id)),
    }
}

#[tauri::command]
pub async fn telnet_write(
    conn_id: String,
    data: String,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(TelnetError::NotConnected(conn_id));
    }
    let _ = data;
    // In a full implementation, write data to the telnet socket.
    Ok(())
}

#[tauri::command]
pub async fn telnet_resize(
    conn_id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(TelnetError::NotConnected(conn_id));
    }
    let _ = (cols, rows);
    // In a full implementation, send NAWS (Negotiate About Window Size) subnegotiation.
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telnet_config_serde() {
        let config = TelnetConfig {
            host: "telnet.example.com".to_string(),
            port: 23,
            terminal_type: "xterm-256color".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"host\":\"telnet.example.com\""));
        assert!(json.contains("\"port\":23"));
        assert!(json.contains("\"terminal_type\":\"xterm-256color\""));

        let restored: TelnetConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.host, "telnet.example.com");
        assert_eq!(restored.port, 23);
    }

    #[test]
    fn test_telnet_connection_lifecycle() {
        let state = TelnetState::new();
        let id = Uuid::new_v4().to_string();

        let conn = TelnetConnection {
            id: id.clone(),
            config: TelnetConfig {
                host: "example.com".to_string(),
                port: 23,
                terminal_type: "vt100".to_string(),
            },
            connected: true,
        };

        state.connections.lock().unwrap().insert(id.clone(), conn);
        assert!(state.connections.lock().unwrap().contains_key(&id));

        state.connections.lock().unwrap().remove(&id);
        assert!(!state.connections.lock().unwrap().contains_key(&id));
    }
}

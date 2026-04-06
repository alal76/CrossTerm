use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SerialError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected: {0}")]
    NotConnected(String),
    #[error("Write failed: {0}")]
    WriteFailed(String),
    #[error("Port not found: {0}")]
    PortNotFound(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for SerialError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopBits {
    One,
    Two,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowControl {
    None,
    Software,
    Hardware,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub flow_control: FlowControl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPort {
    pub name: String,
    pub description: Option<String>,
    pub manufacturer: Option<String>,
}

#[derive(Debug)]
pub struct SerialConnection {
    pub id: String,
    pub config: SerialConfig,
    pub connected: bool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SerialState {
    pub connections: Mutex<HashMap<String, SerialConnection>>,
}

impl SerialState {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn serial_list_ports() -> Result<Vec<SerialPort>, SerialError> {
    // In a full implementation, enumerate available serial ports using
    // a library like serialport-rs. Return empty for now.
    Ok(vec![])
}

#[tauri::command]
pub async fn serial_connect(
    config: SerialConfig,
    state: tauri::State<'_, SerialState>,
) -> Result<String, SerialError> {
    if config.port_name.is_empty() {
        return Err(SerialError::ConfigError("Port name cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();

    // In a full implementation, open the serial port here.
    let conn = SerialConnection {
        id: id.clone(),
        config,
        connected: true,
    };

    state.connections.lock().unwrap().insert(id.clone(), conn);
    Ok(id)
}

#[tauri::command]
pub async fn serial_disconnect(
    conn_id: String,
    state: tauri::State<'_, SerialState>,
) -> Result<(), SerialError> {
    let mut conns = state.connections.lock().unwrap();
    match conns.remove(&conn_id) {
        Some(_) => Ok(()),
        None => Err(SerialError::NotConnected(conn_id)),
    }
}

#[tauri::command]
pub async fn serial_write(
    conn_id: String,
    data: Vec<u8>,
    state: tauri::State<'_, SerialState>,
) -> Result<(), SerialError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(SerialError::NotConnected(conn_id));
    }
    let _ = data;
    // In a full implementation, write data to the serial port.
    Ok(())
}

#[tauri::command]
pub async fn serial_set_baud(
    conn_id: String,
    baud_rate: u32,
    state: tauri::State<'_, SerialState>,
) -> Result<(), SerialError> {
    let mut conns = state.connections.lock().unwrap();
    match conns.get_mut(&conn_id) {
        Some(conn) => {
            conn.config.baud_rate = baud_rate;
            // In a full implementation, reconfigure the port.
            Ok(())
        }
        None => Err(SerialError::NotConnected(conn_id)),
    }
}

#[tauri::command]
pub async fn serial_set_dtr(
    conn_id: String,
    level: bool,
    state: tauri::State<'_, SerialState>,
) -> Result<(), SerialError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(SerialError::NotConnected(conn_id));
    }
    let _ = level;
    // In a full implementation, set DTR line.
    Ok(())
}

#[tauri::command]
pub async fn serial_set_rts(
    conn_id: String,
    level: bool,
    state: tauri::State<'_, SerialState>,
) -> Result<(), SerialError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(SerialError::NotConnected(conn_id));
    }
    let _ = level;
    // In a full implementation, set RTS line.
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_serde() {
        let config = SerialConfig {
            port_name: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"data_bits\":\"eight\""));
        assert!(json.contains("\"stop_bits\":\"one\""));
        assert!(json.contains("\"parity\":\"none\""));
        assert!(json.contains("\"flow_control\":\"none\""));

        let restored: SerialConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.port_name, "/dev/ttyUSB0");
        assert_eq!(restored.baud_rate, 9600);
    }

    #[test]
    fn test_serial_connection_lifecycle() {
        let state = SerialState::new();
        let id = Uuid::new_v4().to_string();

        let conn = SerialConnection {
            id: id.clone(),
            config: SerialConfig {
                port_name: "/dev/ttyUSB0".to_string(),
                baud_rate: 115200,
                data_bits: DataBits::Eight,
                stop_bits: StopBits::One,
                parity: Parity::None,
                flow_control: FlowControl::Hardware,
            },
            connected: true,
        };

        state.connections.lock().unwrap().insert(id.clone(), conn);
        assert!(state.connections.lock().unwrap().contains_key(&id));

        // Update baud rate
        {
            let mut conns = state.connections.lock().unwrap();
            if let Some(c) = conns.get_mut(&id) {
                c.config.baud_rate = 9600;
            }
        }
        assert_eq!(
            state.connections.lock().unwrap().get(&id).unwrap().config.baud_rate,
            9600
        );

        state.connections.lock().unwrap().remove(&id);
        assert!(!state.connections.lock().unwrap().contains_key(&id));
    }

    #[test]
    fn test_serial_state_management() {
        let state = SerialState::new();
        assert!(state.connections.lock().unwrap().is_empty());

        // Add multiple connections
        for i in 0..3 {
            let id = Uuid::new_v4().to_string();
            let conn = SerialConnection {
                id: id.clone(),
                config: SerialConfig {
                    port_name: format!("/dev/ttyUSB{}", i),
                    baud_rate: 9600,
                    data_bits: DataBits::Eight,
                    stop_bits: StopBits::One,
                    parity: Parity::None,
                    flow_control: FlowControl::None,
                },
                connected: true,
            };
            state.connections.lock().unwrap().insert(id, conn);
        }

        assert_eq!(state.connections.lock().unwrap().len(), 3);
    }
}

/// IBM AS/400 5250 terminal over TCP with TN5250E option negotiation.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Tn5250Error {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for Tn5250Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250Config {
    pub host: String,
    pub port: u16,
    /// Optional virtual device name (up to 10 chars)
    pub device_name: Option<String>,
    /// Optional system name (host EBCDIC name)
    pub system_name: Option<String>,
    pub ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250Session {
    pub id: String,
    pub host: String,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250Data {
    pub session_id: String,
    pub data_b64: String,
}

pub struct Tn5250State {
    sessions: Mutex<HashMap<String, Tn5250Session>>,
    writers: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl Tn5250State {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            writers: Mutex::new(HashMap::new()),
        }
    }
}

// TN5250E negotiation constants
const IAC: u8 = 0xFF;
const DO: u8 = 0xFD;
const WILL: u8 = 0xFB;
const WONT: u8 = 0xFC;
const DONT: u8 = 0xFE;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;
const OPT_BINARY: u8 = 0x00;
const OPT_EOR: u8 = 0x19;
const OPT_TTYPE: u8 = 0x18;
const OPT_TN5250E: u8 = 0x28; // RFC 2877
const EOR: u8 = 0xEF;

#[tauri::command]
pub async fn tn5250_connect(
    config: Tn5250Config,
    state: tauri::State<'_, Tn5250State>,
    app: AppHandle,
) -> Result<String, Tn5250Error> {
    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr).await?;
    let id = Uuid::new_v4().to_string();

    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let device = config.device_name.clone().unwrap_or_else(|| "QPADEV0001".to_string());

    tokio::spawn(async move {
        // Negotiate TN5250E: DO BINARY, DO EOR, WILL TN5250E, WILL TTYPE
        let _ = writer.write_all(&[
            IAC, DO, OPT_BINARY, IAC, DO, OPT_EOR,
            IAC, WILL, OPT_TN5250E, IAC, WILL, OPT_TTYPE,
        ]).await;

        // Send TN5250E terminal-type sub-negotiation
        let mut ttype: Vec<u8> = vec![IAC, SB, OPT_TN5250E, 0x02]; // TERMINAL-TYPE
        ttype.extend_from_slice(b"IBM-5555-C01"); // generic 5250 terminal
        ttype.extend_from_slice(&[0x01]); // separator
        ttype.extend_from_slice(device.as_bytes());
        ttype.extend_from_slice(&[IAC, SE]);
        let _ = writer.write_all(&ttype).await;

        while let Some(data) = rx.recv().await {
            if writer.write_all(&data).await.is_err() { break; }
            let _ = writer.write_all(&[IAC, EOR]).await;
        }
    });

    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let encoded = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD, &buf[..n],
                    );
                    let _ = app_clone.emit("tn5250:data", Tn5250Data {
                        session_id: id_clone.clone(), data_b64: encoded,
                    });
                }
            }
        }
    });

    let session = Tn5250Session {
        id: id.clone(), host: config.host, device_name: config.device_name,
    };
    state.sessions.lock().unwrap().insert(id.clone(), session);
    state.writers.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn tn5250_send(
    id: String,
    data_b64: String,
    state: tauri::State<'_, Tn5250State>,
) -> Result<(), Tn5250Error> {
    let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data_b64)
        .map_err(|e| Tn5250Error::ConnectionFailed(e.to_string()))?;
    state.writers.lock().unwrap()
        .get(&id).ok_or_else(|| Tn5250Error::NotFound(id))?
        .send(data).map_err(|e| Tn5250Error::ConnectionFailed(e.to_string()))
}

#[tauri::command]
pub fn tn5250_disconnect(id: String, state: tauri::State<'_, Tn5250State>) -> Result<(), Tn5250Error> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| Tn5250Error::NotFound(id.clone()))?;
    state.writers.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn tn5250_list(state: tauri::State<'_, Tn5250State>) -> Vec<Tn5250Session> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

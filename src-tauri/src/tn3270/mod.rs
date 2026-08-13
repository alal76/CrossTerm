/// IBM 3270 mainframe terminal over TCP with TN3270E option negotiation.
/// The implementation handles Telnet IAC option negotiation to switch the
/// connection into TN3270 binary mode, then emits raw 3270 data-streams as
/// Tauri events for the frontend renderer.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Tn3270Error {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for Tn3270Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tn3270Model {
    /// 24×80 monochrome
    Model2,
    /// 32×80 extended
    Model3,
    /// 43×80 extended  
    Model4,
    /// 27×132 extended
    Model5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270Config {
    pub host: String,
    pub port: u16,
    pub model: Tn3270Model,
    /// Optional LU name for specific LU addressing
    pub lu_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270Session {
    pub id: String,
    pub host: String,
    pub model: Tn3270Model,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270Data {
    pub session_id: String,
    /// Raw base64-encoded 3270 data-stream record
    pub data_b64: String,
}

pub struct Tn3270State {
    sessions: Mutex<HashMap<String, Tn3270Session>>,
    writers: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl Tn3270State {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            writers: Mutex::new(HashMap::new()),
        }
    }
}

// Telnet option bytes used during TN3270E negotiation
const IAC: u8 = 0xFF;
const DO: u8 = 0xFD;
const WILL: u8 = 0xFB;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;
const OPT_BINARY: u8 = 0x00;
const OPT_EOR: u8 = 0x19;
const OPT_TTYPE: u8 = 0x18;
const EOR: u8 = 0xEF;

fn model_terminal_type(model: &Tn3270Model) -> &'static str {
    match model {
        Tn3270Model::Model2 => "IBM-3278-2-E",
        Tn3270Model::Model3 => "IBM-3278-3-E",
        Tn3270Model::Model4 => "IBM-3278-4-E",
        Tn3270Model::Model5 => "IBM-3278-5-E",
    }
}

#[tauri::command]
pub async fn tn3270_connect(
    config: Tn3270Config,
    state: tauri::State<'_, Tn3270State>,
    app: AppHandle,
) -> Result<String, Tn3270Error> {
    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr).await?;
    let id = Uuid::new_v4().to_string();

    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    let terminal_type = model_terminal_type(&config.model).to_string();
    let lu_name = config.lu_name.clone();

    // Telnet negotiation + data pump (write side)
    tokio::spawn(async move {
        // Respond DO BINARY, DO EOR, WILL TTYPE
        let _ = writer.write_all(&[IAC, DO, OPT_BINARY, IAC, DO, OPT_EOR, IAC, WILL, OPT_TTYPE]).await;

        // Send terminal type sub-negotiation when prompted
        let ttype_bytes: Vec<u8> = {
            let mut v = vec![IAC, SB, OPT_TTYPE, 0x00]; // IS
            v.extend_from_slice(terminal_type.as_bytes());
            if let Some(lu) = &lu_name {
                v.push(b'@');
                v.extend_from_slice(lu.as_bytes());
            }
            v.extend_from_slice(&[IAC, SE]);
            v
        };
        let _ = writer.write_all(&ttype_bytes).await;

        while let Some(data) = rx.recv().await {
            if writer.write_all(&data).await.is_err() {
                break;
            }
            let _ = writer.write_all(&[IAC, EOR]).await;
        }
    });

    // Read pump — forward raw 3270 data-stream records as events
    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let encoded = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &buf[..n],
                    );
                    let _ = app_clone.emit("tn3270:data", Tn3270Data {
                        session_id: id_clone.clone(),
                        data_b64: encoded,
                    });
                }
            }
        }
    });

    let session = Tn3270Session { id: id.clone(), host: config.host, model: config.model };
    state.sessions.lock().unwrap().insert(id.clone(), session);
    state.writers.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn tn3270_send(
    id: String,
    data_b64: String, // base64-encoded 3270 AID+data record
    state: tauri::State<'_, Tn3270State>,
) -> Result<(), Tn3270Error> {
    let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data_b64)
        .map_err(|e| Tn3270Error::ConnectionFailed(e.to_string()))?;
    state.writers.lock().unwrap()
        .get(&id)
        .ok_or_else(|| Tn3270Error::NotFound(id))?
        .send(data)
        .map_err(|e| Tn3270Error::ConnectionFailed(e.to_string()))
}

#[tauri::command]
pub fn tn3270_disconnect(id: String, state: tauri::State<'_, Tn3270State>) -> Result<(), Tn3270Error> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| Tn3270Error::NotFound(id.clone()))?;
    state.writers.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn tn3270_list(state: tauri::State<'_, Tn3270State>) -> Vec<Tn3270Session> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

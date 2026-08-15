/// BSD `rlogin` (RFC 1282) over TCP, typically port 513.
///
/// Rlogin predates SSH and has no encryption or real authentication — the
/// client just asserts a local and remote username and the server decides
/// whether to trust that (via `.rhosts`/`hosts.equiv`) or fall back to a
/// password prompt sent as plain terminal text. It's included here for
/// legacy/lab equipment that still only speaks it, not as something to
/// prefer over SSH.
///
/// The handshake (RFC 1282 §2) is four NUL-delimited fields sent as a
/// single burst right after connecting: a leading NUL, the local username,
/// the remote username, and `term-type/speed` — then the link is a plain
/// bidirectional byte stream. There's no option negotiation to get wrong
/// here (unlike Telnet/TN3270), so this is a small, low-risk module.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RloginError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for RloginError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RloginConfig {
    pub host: String,
    /// Default 513
    pub port: u16,
    pub local_username: String,
    pub remote_username: String,
    pub terminal_type: String,
    pub terminal_speed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RloginSession {
    pub id: String,
    pub host: String,
    pub remote_username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RloginData {
    pub session_id: String,
    pub data: String,
}

pub struct RloginState {
    sessions: Mutex<HashMap<String, RloginSession>>,
    writers: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl RloginState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()), writers: Mutex::new(HashMap::new()) }
    }
}

fn build_handshake(config: &RloginConfig) -> Vec<u8> {
    let mut out = vec![0u8];
    out.extend_from_slice(config.local_username.as_bytes());
    out.push(0);
    out.extend_from_slice(config.remote_username.as_bytes());
    out.push(0);
    out.extend_from_slice(format!("{}/{}", config.terminal_type, config.terminal_speed).as_bytes());
    out.push(0);
    out
}

#[tauri::command]
pub async fn rlogin_connect(config: RloginConfig, state: tauri::State<'_, RloginState>, app: AppHandle) -> Result<String, RloginError> {
    let addr = format!("{}:{}", config.host, config.port);
    let mut stream = TcpStream::connect(&addr).await?;
    stream.write_all(&build_handshake(&config)).await?;

    let id = Uuid::new_v4().to_string();
    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if writer.write_all(&data).await.is_err() {
                break;
            }
        }
    });

    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = app_clone.emit("rlogin:data", RloginData { session_id: id_clone.clone(), data: String::from_utf8_lossy(&buf[..n]).into_owned() });
                }
            }
        }
        let _ = app_clone.emit("rlogin:disconnected", &id_clone);
    });

    state.sessions.lock().unwrap().insert(id.clone(), RloginSession { id: id.clone(), host: config.host, remote_username: config.remote_username });
    state.writers.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn rlogin_send(id: String, data: String, state: tauri::State<'_, RloginState>) -> Result<(), RloginError> {
    state
        .writers
        .lock()
        .unwrap()
        .get(&id)
        .ok_or_else(|| RloginError::NotFound(id.clone()))?
        .send(data.into_bytes())
        .map_err(|e| RloginError::ConnectionFailed(e.to_string()))
}

#[tauri::command]
pub fn rlogin_disconnect(id: String, state: tauri::State<'_, RloginState>) -> Result<(), RloginError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| RloginError::NotFound(id.clone()))?;
    state.writers.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn rlogin_list(state: tauri::State<'_, RloginState>) -> Vec<RloginSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_handshake_layout() {
        let config = RloginConfig {
            host: "10.0.0.70".into(),
            port: 513,
            local_username: "alal".into(),
            remote_username: "root".into(),
            terminal_type: "xterm".into(),
            terminal_speed: 38400,
        };
        let handshake = build_handshake(&config);
        assert_eq!(handshake[0], 0);
        assert_eq!(&handshake[1..5], b"alal");
        assert_eq!(handshake[5], 0);
        assert_eq!(&handshake[6..10], b"root");
        assert_eq!(handshake[10], 0);
        assert_eq!(&handshake[11..22], b"xterm/38400");
        assert_eq!(handshake[22], 0);
        assert_eq!(handshake.len(), 23);
    }

    #[test]
    fn test_build_handshake_empty_local_username() {
        let config = RloginConfig {
            host: "h".into(),
            port: 513,
            local_username: "".into(),
            remote_username: "u".into(),
            terminal_type: "vt100".into(),
            terminal_speed: 9600,
        };
        let handshake = build_handshake(&config);
        // Leading NUL immediately followed by the field-separator NUL
        // since local_username is empty.
        assert_eq!(&handshake[0..2], &[0, 0]);
    }
}

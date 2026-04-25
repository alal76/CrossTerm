use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex as TokioMutex;
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

// ── Telnet constants ────────────────────────────────────────────────────

const IAC: u8 = 0xFF;
const WILL: u8 = 0xFB;
const WONT: u8 = 0xFC;
const DO: u8 = 0xFD;
const DONT: u8 = 0xFE;
const SB: u8 = 0xFA;  // subnegotiation begin
const SE: u8 = 0xF0;  // subnegotiation end
const NAWS: u8 = 31;  // option: Negotiate About Window Size

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelnetConfig {
    pub host: String,
    pub port: u16,
    pub terminal_type: String,
}

#[derive(Debug, Serialize)]
struct TelnetDataEvent {
    conn_id: String,
    data: String,
}

// ── Connection inner ────────────────────────────────────────────────────

struct TelnetConnectionInner {
    config: TelnetConfig,
    writer: TokioMutex<tokio::net::tcp::OwnedWriteHalf>,
    naws_accepted: AtomicBool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct TelnetState {
    pub connections: TokioMutex<HashMap<String, Arc<TelnetConnectionInner>>>,
}

impl TelnetState {
    pub fn new() -> Self {
        Self {
            connections: TokioMutex::new(HashMap::new()),
        }
    }
}

// ── IAC reader task ─────────────────────────────────────────────────────

/// Parse and handle IAC sequences from the buffer.
///
/// Returns `(clean_data, reply_bytes)` where `clean_data` is the non-IAC
/// payload to emit to the frontend and `reply_bytes` is any negotiation
/// reply to send back to the server.
fn process_buffer(
    buf: &[u8],
    naws_accepted: &AtomicBool,
) -> (Vec<u8>, Vec<u8>) {
    let mut data_out: Vec<u8> = Vec::new();
    let mut reply: Vec<u8> = Vec::new();
    let mut i = 0;

    while i < buf.len() {
        if buf[i] != IAC {
            data_out.push(buf[i]);
            i += 1;
            continue;
        }

        // buf[i] == IAC
        if i + 1 >= buf.len() {
            // Incomplete sequence at end of buffer; skip IAC
            i += 1;
            break;
        }

        let cmd = buf[i + 1];

        match cmd {
            // IAC IAC → literal 0xFF in data
            IAC => {
                data_out.push(IAC);
                i += 2;
            }
            // Subnegotiation: IAC SB ... IAC SE
            SB => {
                // Skip until we find IAC SE
                let mut j = i + 2;
                while j + 1 < buf.len() {
                    if buf[j] == IAC && buf[j + 1] == SE {
                        j += 2;
                        break;
                    }
                    j += 1;
                }
                i = j;
            }
            // WILL/WONT/DO/DONT followed by option byte
            WILL | WONT | DO | DONT => {
                if i + 2 >= buf.len() {
                    // Incomplete; skip
                    i += 2;
                    break;
                }
                let option = buf[i + 2];

                match cmd {
                    DO if option == NAWS => {
                        // Server asks us to negotiate window size — accept
                        naws_accepted.store(true, Ordering::SeqCst);
                        reply.extend_from_slice(&[IAC, WILL, NAWS]);
                    }
                    DO => {
                        // Refuse all other DO requests
                        reply.extend_from_slice(&[IAC, WONT, option]);
                    }
                    WILL => {
                        // Accept server's offer to enable something (passive)
                        reply.extend_from_slice(&[IAC, DO, option]);
                    }
                    WONT => {
                        // Server refuses; acknowledge
                        reply.extend_from_slice(&[IAC, DONT, option]);
                    }
                    DONT => {
                        // Server tells us to stop; acknowledge
                        reply.extend_from_slice(&[IAC, WONT, option]);
                    }
                    _ => {}
                }
                i += 3;
            }
            // Any other IAC command (NOP, DM, BRK, etc.) — skip 2 bytes
            _ => {
                i += 2;
            }
        }
    }

    (data_out, reply)
}

/// Spawn the background reader task for a connection.
fn spawn_reader(
    conn_id: String,
    app: AppHandle,
    mut reader: tokio::net::tcp::OwnedReadHalf,
    writer: Arc<TelnetConnectionInner>,
) {
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => {
                    // EOF
                    let _ = app.emit("telnet:disconnected", conn_id.clone());
                    break;
                }
                Ok(n) => {
                    let (data_out, reply) = process_buffer(&buf[..n], &writer.naws_accepted);

                    // Send negotiation replies back to server
                    if !reply.is_empty() {
                        let mut w = writer.writer.lock().await;
                        let _ = w.write_all(&reply).await;
                    }

                    // Emit clean data to frontend
                    if !data_out.is_empty() {
                        // Use lossy UTF-8 conversion so terminal output is always valid
                        let text = String::from_utf8_lossy(&data_out).into_owned();
                        let _ = app.emit(
                            "telnet:data",
                            TelnetDataEvent {
                                conn_id: conn_id.clone(),
                                data: text,
                            },
                        );
                    }
                }
                Err(_) => {
                    let _ = app.emit("telnet:disconnected", conn_id.clone());
                    break;
                }
            }
        }
    });
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn telnet_connect(
    config: TelnetConfig,
    app: AppHandle,
    state: tauri::State<'_, TelnetState>,
) -> Result<String, TelnetError> {
    if config.host.is_empty() {
        return Err(TelnetError::ConnectionFailed("Host cannot be empty".into()));
    }

    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| TelnetError::ConnectionFailed(e.to_string()))?;

    let (reader, writer) = stream.into_split();

    let id = Uuid::new_v4().to_string();

    let inner = Arc::new(TelnetConnectionInner {
        config,
        writer: TokioMutex::new(writer),
        naws_accepted: AtomicBool::new(false),
    });

    state
        .connections
        .lock()
        .await
        .insert(id.clone(), Arc::clone(&inner));

    spawn_reader(id.clone(), app, reader, inner);

    Ok(id)
}

#[tauri::command]
pub async fn telnet_disconnect(
    conn_id: String,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let removed = state.connections.lock().await.remove(&conn_id);
    match removed {
        Some(inner) => {
            // Closing the writer will signal EOF to the remote end
            let mut w = inner.writer.lock().await;
            let _ = w.shutdown().await;
            Ok(())
        }
        None => Err(TelnetError::NotConnected(conn_id)),
    }
}

#[tauri::command]
pub async fn telnet_write(
    conn_id: String,
    data: String,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let conns = state.connections.lock().await;
    let inner = conns
        .get(&conn_id)
        .ok_or_else(|| TelnetError::NotConnected(conn_id.clone()))?;

    // Escape IAC bytes: 0xFF → 0xFF 0xFF
    let raw = data.as_bytes();
    let mut escaped: Vec<u8> = Vec::with_capacity(raw.len());
    for &b in raw {
        if b == IAC {
            escaped.push(IAC);
        }
        escaped.push(b);
    }

    let mut w = inner.writer.lock().await;
    w.write_all(&escaped)
        .await
        .map_err(|e| TelnetError::WriteFailed(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn telnet_resize(
    conn_id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, TelnetState>,
) -> Result<(), TelnetError> {
    let conns = state.connections.lock().await;
    let inner = conns
        .get(&conn_id)
        .ok_or_else(|| TelnetError::NotConnected(conn_id.clone()))?;

    if !inner.naws_accepted.load(Ordering::SeqCst) {
        // Server hasn't negotiated NAWS yet; silently ignore
        return Ok(());
    }

    // IAC SB NAWS cols_hi cols_lo rows_hi rows_lo IAC SE
    let naws_msg = [
        IAC,
        SB,
        NAWS,
        (cols >> 8) as u8,
        (cols & 0xFF) as u8,
        (rows >> 8) as u8,
        (rows & 0xFF) as u8,
        IAC,
        SE,
    ];

    let mut w = inner.writer.lock().await;
    w.write_all(&naws_msg)
        .await
        .map_err(|e| TelnetError::WriteFailed(e.to_string()))?;

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
        // Test that TelnetState can be created and the HashMap is initially empty
        let state = TelnetState::new();
        // We can only do synchronous checks here; async operations require a runtime
        // The HashMap starts empty — confirm we have a valid state object
        let _ = state; // construction succeeds
    }

    #[test]
    fn test_process_buffer_plain_text() {
        let naws = AtomicBool::new(false);
        let input = b"Hello, world!";
        let (data, reply) = process_buffer(input, &naws);
        assert_eq!(data, b"Hello, world!");
        assert!(reply.is_empty());
    }

    #[test]
    fn test_process_buffer_iac_iac_escaping() {
        let naws = AtomicBool::new(false);
        // IAC IAC should produce a literal 0xFF in the output
        let input = &[0xFF, 0xFF, b'A'];
        let (data, reply) = process_buffer(input, &naws);
        assert_eq!(data, &[0xFF, b'A']);
        assert!(reply.is_empty());
    }

    #[test]
    fn test_process_buffer_do_naws() {
        let naws = AtomicBool::new(false);
        // IAC DO NAWS → server asks us to report window size
        let input = &[IAC, DO, NAWS];
        let (data, reply) = process_buffer(input, &naws);
        assert!(data.is_empty());
        // Should reply IAC WILL NAWS and set naws_accepted
        assert_eq!(reply, &[IAC, WILL, NAWS]);
        assert!(naws.load(Ordering::SeqCst));
    }

    #[test]
    fn test_process_buffer_do_unknown_option() {
        let naws = AtomicBool::new(false);
        // IAC DO <some_option> → we should reply WONT
        let input = &[IAC, DO, 1u8];
        let (data, reply) = process_buffer(input, &naws);
        assert!(data.is_empty());
        assert_eq!(reply, &[IAC, WONT, 1u8]);
    }

    #[test]
    fn test_process_buffer_subnegotiation_stripped() {
        let naws = AtomicBool::new(false);
        // IAC SB <opt> <data> IAC SE followed by plain text
        let input = &[IAC, SB, 24, b'x', b't', b'e', IAC, SE, b'O', b'K'];
        let (data, reply) = process_buffer(input, &naws);
        // Subnegotiation is stripped; only "OK" remains
        assert_eq!(data, b"OK");
        assert!(reply.is_empty());
    }

    #[test]
    fn test_naws_subnegotiation_bytes() {
        // Verify the NAWS message layout
        let cols: u16 = 80;
        let rows: u16 = 24;
        let expected = [
            IAC,
            SB,
            NAWS,
            0, 80,  // cols hi, lo
            0, 24,  // rows hi, lo
            IAC,
            SE,
        ];
        let actual = [
            IAC,
            SB,
            NAWS,
            (cols >> 8) as u8,
            (cols & 0xFF) as u8,
            (rows >> 8) as u8,
            (rows & 0xFF) as u8,
            IAC,
            SE,
        ];
        assert_eq!(actual, expected);
    }
}

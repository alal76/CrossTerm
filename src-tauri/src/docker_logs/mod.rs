/// Docker container log streaming via the Engine API's
/// `GET /containers/{id}/logs?follow=true` endpoint — a plain streamed
/// HTTP/1.1 response, either over the local Unix socket
/// (`/var/run/docker.sock`) or a remote `tcp://host:port` daemon.
///
/// No Docker SDK crate is used: this is a small enough slice of HTTP/1.1
/// (one GET request, chunked response body, no need for redirects/keep-
/// alive/etc.) that hand-rolling it avoids pulling in a full HTTP client
/// stack for a single request/response shape. The two things worth getting
/// right are documented in Docker's own API reference and are stable
/// across API versions: chunked transfer-encoding framing (RFC 9112 §7.1),
/// and — only when the container was *not* created with a TTY — Docker's
/// own 8-byte stream-multiplexing frame header
/// (`[stream_type, 0, 0, 0, size_be_u32]` followed by `size` payload
/// bytes) that interleaves stdout/stderr on one connection.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DockerLogsError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Docker API error: {0}")]
    Api(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for DockerLogsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLogsConfig {
    /// Unix socket path, e.g. "/var/run/docker.sock" — takes priority over
    /// host/port when set.
    pub socket_path: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub container_id: String,
    /// Whether the container was created with a TTY — determines whether
    /// the log stream is demultiplexed (Docker's 8-byte frame header) or
    /// raw bytes.
    pub tty: bool,
    pub tail: Option<u32>,
    pub timestamps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLogsSession {
    pub id: String,
    pub container_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLogLine {
    pub session_id: String,
    pub stream: String, // "stdout" | "stderr" | "raw"
    pub data: String,
}

pub struct DockerLogsState {
    sessions: Mutex<HashMap<String, DockerLogsSession>>,
    cancel: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

impl DockerLogsState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()), cancel: Mutex::new(HashMap::new()) }
    }
}

fn build_request(config: &DockerLogsConfig) -> String {
    let tail = config.tail.unwrap_or(200);
    let query = format!(
        "follow=true&stdout=true&stderr=true&tail={tail}&timestamps={}",
        config.timestamps
    );
    format!(
        "GET /v1.41/containers/{}/logs?{query} HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n",
        config.container_id
    )
}

/// Reads and discards the HTTP status line + headers, returning whether
/// the response uses chunked transfer-encoding (Docker's log endpoint
/// always streams, so a `Content-Length` response would be unusual, but
/// the check is cheap and avoids assuming a specific server behavior).
async fn read_http_headers<R: AsyncRead + Unpin>(reader: &mut R) -> Result<bool, DockerLogsError> {
    let mut header_bytes = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            return Err(DockerLogsError::Api("Connection closed before headers completed".into()));
        }
        header_bytes.push(buf[0]);
        if header_bytes.ends_with(b"\r\n\r\n") {
            break;
        }
        if header_bytes.len() > 64 * 1024 {
            return Err(DockerLogsError::Api("Response headers too large".into()));
        }
    }
    let header_text = String::from_utf8_lossy(&header_bytes);
    let status_line = header_text.lines().next().unwrap_or("");
    if !status_line.contains("200") {
        return Err(DockerLogsError::Api(format!("Unexpected response: {status_line}")));
    }
    let chunked = header_text.to_ascii_lowercase().contains("transfer-encoding: chunked");
    Ok(chunked)
}

/// Reads one chunk from a chunked-transfer-encoded body: a hex size line,
/// that many payload bytes, then a trailing CRLF. Returns `None` at the
/// terminating zero-size chunk.
async fn read_one_chunk<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>, DockerLogsError> {
    let mut size_line = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            return Ok(None);
        }
        if buf[0] == b'\n' {
            break;
        }
        if buf[0] != b'\r' {
            size_line.push(buf[0]);
        }
    }
    let size_str = String::from_utf8_lossy(&size_line);
    let size = usize::from_str_radix(size_str.trim(), 16).map_err(|_| DockerLogsError::Api(format!("Bad chunk size: {size_str}")))?;
    if size == 0 {
        return Ok(None);
    }
    let mut payload = vec![0u8; size];
    reader.read_exact(&mut payload).await?;
    let mut crlf = [0u8; 2];
    reader.read_exact(&mut crlf).await?;
    Ok(Some(payload))
}

/// Demultiplexes Docker's non-TTY log frame format from a byte buffer,
/// returning `(stream_name, payload, bytes_consumed)` for however many
/// complete frames are available — a chunk boundary can split a frame
/// mid-header or mid-payload, so the caller re-buffers any leftover bytes.
fn demux_frames(buf: &[u8]) -> (Vec<(&'static str, Vec<u8>)>, usize) {
    let mut frames = Vec::new();
    let mut i = 0;
    while i + 8 <= buf.len() {
        let stream_type = buf[i];
        let size = u32::from_be_bytes([buf[i + 4], buf[i + 5], buf[i + 6], buf[i + 7]]) as usize;
        if i + 8 + size > buf.len() {
            break; // incomplete frame — wait for more data
        }
        let stream = if stream_type == 2 { "stderr" } else { "stdout" };
        frames.push((stream, buf[i + 8..i + 8 + size].to_vec()));
        i += 8 + size;
    }
    (frames, i)
}

#[tauri::command]
pub async fn docker_logs_connect(config: DockerLogsConfig, state: tauri::State<'_, DockerLogsState>, app: AppHandle) -> Result<String, DockerLogsError> {
    let id = Uuid::new_v4().to_string();
    let request = build_request(&config);
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();

    let app_clone = app.clone();
    let id_clone = id.clone();
    let tty = config.tty;

    macro_rules! pump {
        ($stream:expr) => {{
            let mut stream = $stream;
            stream.write_all(request.as_bytes()).await?;
            tokio::spawn(async move {
                let chunked = match read_http_headers(&mut stream).await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = app_clone.emit("docker_logs:error", format!("{}:{e}", id_clone));
                        return;
                    }
                };
                let mut pending = Vec::new();
                loop {
                    tokio::select! {
                        _ = &mut cancel_rx => break,
                        chunk = read_one_chunk(&mut stream), if chunked => {
                            match chunk {
                                Ok(Some(bytes)) => pending.extend_from_slice(&bytes),
                                Ok(None) | Err(_) => break,
                            }
                        }
                    }
                    if tty {
                        if !pending.is_empty() {
                            let _ = app_clone.emit("docker_logs:line", DockerLogLine { session_id: id_clone.clone(), stream: "raw".into(), data: String::from_utf8_lossy(&pending).into_owned() });
                            pending.clear();
                        }
                    } else {
                        let (frames, consumed) = demux_frames(&pending);
                        for (stream_name, payload) in frames {
                            let _ = app_clone.emit("docker_logs:line", DockerLogLine { session_id: id_clone.clone(), stream: stream_name.into(), data: String::from_utf8_lossy(&payload).into_owned() });
                        }
                        pending.drain(..consumed);
                    }
                }
                let _ = app_clone.emit("docker_logs:disconnected", &id_clone);
            });
        }};
    }

    if let Some(socket_path) = &config.socket_path {
        #[cfg(not(unix))]
        return Err(DockerLogsError::ConnectionFailed(format!(
            "Unix socket connections ({socket_path}) aren't supported on Windows — use host/port (Docker's TCP API) instead"
        )));
        #[cfg(unix)]
        {
            let stream = UnixStream::connect(socket_path).await?;
            pump!(stream);
        }
    } else {
        let host = config.host.clone().ok_or_else(|| DockerLogsError::ConnectionFailed("No socket_path or host configured".into()))?;
        let port = config.port.unwrap_or(2375);
        let stream = TcpStream::connect(format!("{host}:{port}")).await?;
        pump!(stream);
    }

    state.sessions.lock().unwrap().insert(id.clone(), DockerLogsSession { id: id.clone(), container_id: config.container_id });
    state.cancel.lock().unwrap().insert(id.clone(), cancel_tx);
    Ok(id)
}

#[tauri::command]
pub fn docker_logs_disconnect(id: String, state: tauri::State<'_, DockerLogsState>) -> Result<(), DockerLogsError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| DockerLogsError::NotFound(id.clone()))?;
    if let Some(tx) = state.cancel.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    }
    Ok(())
}

#[tauri::command]
pub fn docker_logs_list(state: tauri::State<'_, DockerLogsState>) -> Vec<DockerLogsSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request_includes_container_id_and_query_params() {
        let config = DockerLogsConfig { socket_path: None, host: None, port: None, container_id: "abc123".into(), tty: false, tail: Some(50), timestamps: true };
        let req = build_request(&config);
        assert!(req.starts_with("GET /v1.41/containers/abc123/logs?"));
        assert!(req.contains("tail=50"));
        assert!(req.contains("timestamps=true"));
        assert!(req.contains("follow=true"));
        assert!(req.ends_with("\r\n\r\n"));
    }

    #[tokio::test]
    async fn test_read_http_headers_detects_chunked() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\n\r\n";
        let mut cursor = std::io::Cursor::new(response.to_vec());
        let chunked = read_http_headers(&mut cursor).await.unwrap();
        assert!(chunked);
    }

    #[tokio::test]
    async fn test_read_http_headers_rejects_non_200() {
        let response = b"HTTP/1.1 404 Not Found\r\n\r\n";
        let mut cursor = std::io::Cursor::new(response.to_vec());
        assert!(read_http_headers(&mut cursor).await.is_err());
    }

    #[tokio::test]
    async fn test_read_one_chunk_roundtrip() {
        let mut body = Vec::new();
        body.extend_from_slice(b"5\r\nhello\r\n");
        body.extend_from_slice(b"0\r\n\r\n");
        let mut cursor = std::io::Cursor::new(body);
        let chunk1 = read_one_chunk(&mut cursor).await.unwrap();
        assert_eq!(chunk1, Some(b"hello".to_vec()));
        let chunk2 = read_one_chunk(&mut cursor).await.unwrap();
        assert_eq!(chunk2, None);
    }

    #[test]
    fn test_demux_frames_single_complete_frame() {
        let mut buf = vec![1u8, 0, 0, 0]; // stdout, size follows
        buf.extend_from_slice(&5u32.to_be_bytes());
        buf.extend_from_slice(b"hello");
        let (frames, consumed) = demux_frames(&buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, "stdout");
        assert_eq!(frames[0].1, b"hello");
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_demux_frames_stderr_and_incomplete_trailing_frame() {
        let mut buf = vec![2u8, 0, 0, 0];
        buf.extend_from_slice(&3u32.to_be_bytes());
        buf.extend_from_slice(b"err");
        // Incomplete second frame — header claims 10 bytes but only 2 are present.
        buf.extend_from_slice(&[1u8, 0, 0, 0, 0, 0, 0, 10, b'a', b'b']);
        let (frames, consumed) = demux_frames(&buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, "stderr");
        assert_eq!(consumed, 8 + 3); // only the complete first frame consumed
    }
}

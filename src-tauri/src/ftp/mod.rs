use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

// ── Error ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum FtpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected: {0}")]
    NotConnected(String),
    #[error("Transfer failed: {0}")]
    TransferFailed(String),
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl Serialize for FtpError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub use_tls: bool,
    pub passive_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FtpEntryType {
    File,
    Directory,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtpEntry {
    pub name: String,
    pub size: u64,
    pub entry_type: FtpEntryType,
    pub modified: Option<String>,
    pub permissions: Option<String>,
}

// ── Control connection ───────────────────────────────────────────────────

pub struct FtpControl {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl FtpControl {
    /// Create an FtpControl from an established TcpStream.
    fn from_stream(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        }
    }

    /// Send a raw command line (appends CRLF).
    async fn send_cmd(&mut self, cmd: &str) -> Result<(), FtpError> {
        let line = format!("{}\r\n", cmd);
        self.writer.write_all(line.as_bytes()).await?;
        Ok(())
    }

    /// Read an FTP response. Handles multi-line responses (RFC 959 §4.2).
    /// A multi-line response starts with "NNN-" and ends with "NNN " (same code).
    async fn read_response(&mut self) -> Result<(u32, String), FtpError> {
        let mut first_line = String::new();
        self.reader.read_line(&mut first_line).await?;
        let first_line = first_line.trim_end_matches('\n').trim_end_matches('\r').to_string();

        if first_line.len() < 3 {
            return Err(FtpError::Protocol(format!(
                "Response too short: {:?}",
                first_line
            )));
        }

        let code_str = &first_line[..3];
        let code: u32 = code_str
            .parse()
            .map_err(|_| FtpError::Protocol(format!("Bad response code: {:?}", first_line)))?;

        // Check if this is a multi-line response ("NNN-...")
        let is_multiline = first_line.len() > 3 && first_line.as_bytes()[3] == b'-';

        if !is_multiline {
            return Ok((code, first_line[4..].to_string()));
        }

        // Consume continuation lines until we see "NNN " (same code, space separator)
        let end_prefix = format!("{} ", code_str);
        let mut text = first_line[4..].to_string();
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).await?;
            let line = line.trim_end_matches('\n').trim_end_matches('\r').to_string();
            if line.starts_with(&end_prefix) {
                text.push('\n');
                text.push_str(&line[4..]);
                break;
            }
            text.push('\n');
            text.push_str(&line);
        }

        Ok((code, text))
    }

    /// Read a response and return an error if the code doesn't match.
    async fn expect(&mut self, expected: u32) -> Result<String, FtpError> {
        let (code, text) = self.read_response().await?;
        if code != expected {
            // Map common codes to richer errors
            match code {
                530 => return Err(FtpError::PermissionDenied(text)),
                550 => return Err(FtpError::PathNotFound(text)),
                _ => {
                    return Err(FtpError::Protocol(format!(
                        "Expected {}, got {} {}",
                        expected, code, text
                    )))
                }
            }
        }
        Ok(text)
    }

    /// Open a passive data connection. Returns a connected TcpStream.
    async fn open_passive(&mut self) -> Result<TcpStream, FtpError> {
        self.send_cmd("PASV").await?;
        let text = self.expect(227).await?;

        // Parse "227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)"
        let start = text
            .find('(')
            .ok_or_else(|| FtpError::Protocol("PASV: no '(' in response".into()))?;
        let end = text
            .find(')')
            .ok_or_else(|| FtpError::Protocol("PASV: no ')' in response".into()))?;
        let inner = &text[start + 1..end];
        let parts: Vec<u8> = inner
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.len() != 6 {
            return Err(FtpError::Protocol(format!(
                "PASV: expected 6 octets, got {}",
                parts.len()
            )));
        }
        let addr = format!("{}.{}.{}.{}:{}", parts[0], parts[1], parts[2], parts[3],
                           (parts[4] as u16) * 256 + parts[5] as u16);
        let data_stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| FtpError::ConnectionFailed(format!("Data channel {}: {}", addr, e)))?;
        Ok(data_stream)
    }
}

// ── State ────────────────────────────────────────────────────────────────

pub struct FtpState {
    pub connections: TokioMutex<HashMap<String, Arc<TokioMutex<FtpControl>>>>,
}

impl FtpState {
    pub fn new() -> Self {
        Self {
            connections: TokioMutex::new(HashMap::new()),
        }
    }
}

// ── LIST parsing ─────────────────────────────────────────────────────────

/// Parse one line of Unix-format `LIST` output into an `FtpEntry`.
///
/// Format: `<perms> <links> <user> <group> <size> <month> <day> <time/year> <name>`
/// Example: `drwxr-xr-x  2 user group     4096 Jan 15 12:00 Documents`
fn parse_list_line(line: &str) -> Option<FtpEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Collect all whitespace-separated tokens for the first 8 fixed fields.
    // Unix ls format: permissions links user group size month day time/year name...
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 9 {
        return None;
    }

    let permissions = fields[0].to_string(); // drwxr-xr-x
    // fields[1] = links, [2] = user, [3] = group
    let size_str = fields[4];               // size
    let month = fields[5];                  // Jan
    let day = fields[6];                    // 15
    let time_or_year = fields[7];           // 12:00 or 2023

    // The filename is everything after the first 8 whitespace-separated tokens.
    // Walk through the original line skipping 8 tokens to preserve spaces in filenames.
    let name_part = {
        let mut remaining = line;
        for _ in 0..8 {
            remaining = remaining.trim_start_matches(|c: char| c.is_ascii_whitespace());
            let end = remaining.find(|c: char| c.is_ascii_whitespace()).unwrap_or(remaining.len());
            remaining = &remaining[end..];
        }
        remaining.trim()
    };

    if name_part.is_empty() {
        return None;
    }

    // Skip "." and ".." entries
    if name_part == "." || name_part == ".." {
        return None;
    }

    let size: u64 = size_str.parse().unwrap_or(0);

    let entry_type = match permissions.chars().next()? {
        'd' => FtpEntryType::Directory,
        'l' => FtpEntryType::Link,
        _ => FtpEntryType::File,
    };

    // For symlinks strip the " -> target" portion to get the real name
    let name = if matches!(entry_type, FtpEntryType::Link) {
        name_part
            .split(" -> ")
            .next()
            .unwrap_or(name_part)
            .to_string()
    } else {
        name_part.to_string()
    };

    let modified = Some(format!("{} {} {}", month, day, time_or_year));

    Some(FtpEntry {
        name,
        size,
        entry_type,
        modified,
        permissions: Some(permissions),
    })
}

// ── Tauri Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ftp_connect(
    config: FtpConfig,
    state: tauri::State<'_, FtpState>,
) -> Result<String, FtpError> {
    if config.host.is_empty() {
        return Err(FtpError::ConnectionFailed("Host cannot be empty".into()));
    }

    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| FtpError::ConnectionFailed(format!("TCP connect to {}: {}", addr, e)))?;

    let mut ctrl = FtpControl::from_stream(stream);

    // Read server welcome (220)
    let (code, text) = ctrl.read_response().await?;
    if code != 220 {
        return Err(FtpError::ConnectionFailed(format!(
            "Expected 220 welcome, got {} {}",
            code, text
        )));
    }

    // USER
    let username = config
        .username
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    ctrl.send_cmd(&format!("USER {}", username)).await?;
    let (user_code, user_text) = ctrl.read_response().await?;
    match user_code {
        230 => {
            // Already logged in (e.g. anonymous with no password prompt)
        }
        331 => {
            // Password required
            let password = config.password.clone().unwrap_or_default();
            ctrl.send_cmd(&format!("PASS {}", password)).await?;
            let (pass_code, pass_text) = ctrl.read_response().await?;
            if pass_code != 230 {
                return Err(FtpError::PermissionDenied(format!(
                    "Login failed: {} {}",
                    pass_code, pass_text
                )));
            }
        }
        _ => {
            return Err(FtpError::ConnectionFailed(format!(
                "USER rejected: {} {}",
                user_code, user_text
            )));
        }
    }

    // TYPE I (binary mode)
    ctrl.send_cmd("TYPE I").await?;
    ctrl.expect(200).await?;

    let id = Uuid::new_v4().to_string();
    let ctrl_arc = Arc::new(TokioMutex::new(ctrl));
    state.connections.lock().await.insert(id.clone(), ctrl_arc);
    Ok(id)
}

#[tauri::command]
pub async fn ftp_disconnect(
    conn_id: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let ctrl_arc = {
        let mut conns = state.connections.lock().await;
        conns
            .remove(&conn_id)
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    // Best-effort QUIT; ignore errors since we're disconnecting anyway.
    let mut ctrl = ctrl_arc.lock().await;
    let _ = ctrl.send_cmd("QUIT").await;
    Ok(())
}

#[tauri::command]
pub async fn ftp_list(
    conn_id: String,
    path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<Vec<FtpEntry>, FtpError> {
    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    let mut ctrl = ctrl_arc.lock().await;

    // CWD to target directory
    if !path.is_empty() && path != "/" {
        ctrl.send_cmd(&format!("CWD {}", path)).await?;
        let (code, text) = ctrl.read_response().await?;
        if code != 250 {
            return Err(FtpError::PathNotFound(format!(
                "CWD {} failed: {} {}",
                path, code, text
            )));
        }
    }

    // Open passive data channel
    let mut data_stream = ctrl.open_passive().await?;

    // LIST command
    ctrl.send_cmd("LIST").await?;
    // Expect 150 or 125 (data connection open / transfer starting)
    let (list_code, list_text) = ctrl.read_response().await?;
    if list_code != 150 && list_code != 125 {
        return Err(FtpError::TransferFailed(format!(
            "LIST failed: {} {}",
            list_code, list_text
        )));
    }

    // Read all data from the data channel
    let mut data = String::new();
    data_stream.read_to_string(&mut data).await?;
    drop(data_stream);

    // Read the 226 Transfer complete response
    ctrl.expect(226).await?;

    // Parse listing lines
    let entries: Vec<FtpEntry> = data
        .lines()
        .filter_map(parse_list_line)
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn ftp_upload(
    conn_id: String,
    local_path: String,
    remote_path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    if local_path.is_empty() || remote_path.is_empty() {
        return Err(FtpError::InvalidOperation("Paths cannot be empty".into()));
    }

    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    // Read local file
    let file_data = tokio::fs::read(&local_path)
        .await
        .map_err(|e| FtpError::TransferFailed(format!("Read local file {}: {}", local_path, e)))?;

    let mut ctrl = ctrl_arc.lock().await;

    // Open passive data channel
    let mut data_stream = ctrl.open_passive().await?;

    // STOR command
    ctrl.send_cmd(&format!("STOR {}", remote_path)).await?;
    let (code, text) = ctrl.read_response().await?;
    if code != 150 && code != 125 {
        return Err(FtpError::TransferFailed(format!(
            "STOR failed: {} {}",
            code, text
        )));
    }

    // Write file data
    data_stream
        .write_all(&file_data)
        .await
        .map_err(|e| FtpError::TransferFailed(format!("Write data: {}", e)))?;
    drop(data_stream);

    // Expect 226 transfer complete
    ctrl.expect(226).await?;

    Ok(())
}

#[tauri::command]
pub async fn ftp_download(
    conn_id: String,
    remote_path: String,
    local_path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    if remote_path.is_empty() || local_path.is_empty() {
        return Err(FtpError::InvalidOperation("Paths cannot be empty".into()));
    }

    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    let mut ctrl = ctrl_arc.lock().await;

    // Open passive data channel
    let mut data_stream = ctrl.open_passive().await?;

    // RETR command
    ctrl.send_cmd(&format!("RETR {}", remote_path)).await?;
    let (code, text) = ctrl.read_response().await?;
    if code != 150 && code != 125 {
        return Err(FtpError::TransferFailed(format!(
            "RETR failed: {} {}",
            code, text
        )));
    }

    // Read all data from the data channel
    let mut data = Vec::new();
    data_stream.read_to_end(&mut data).await?;
    drop(data_stream);

    // Expect 226 transfer complete
    ctrl.expect(226).await?;

    // Write to local file
    tokio::fs::write(&local_path, &data)
        .await
        .map_err(|e| FtpError::TransferFailed(format!("Write local file {}: {}", local_path, e)))?;

    Ok(())
}

#[tauri::command]
pub async fn ftp_mkdir(
    conn_id: String,
    path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    if path.is_empty() {
        return Err(FtpError::InvalidOperation("Path cannot be empty".into()));
    }

    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    let mut ctrl = ctrl_arc.lock().await;
    ctrl.send_cmd(&format!("MKD {}", path)).await?;
    ctrl.expect(257).await?;
    Ok(())
}

#[tauri::command]
pub async fn ftp_delete(
    conn_id: String,
    path: String,
    entry_type: FtpEntryType,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    if path.is_empty() {
        return Err(FtpError::InvalidOperation("Path cannot be empty".into()));
    }

    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    let mut ctrl = ctrl_arc.lock().await;
    match entry_type {
        FtpEntryType::Directory => {
            ctrl.send_cmd(&format!("RMD {}", path)).await?;
            ctrl.expect(250).await?;
        }
        FtpEntryType::File | FtpEntryType::Link => {
            ctrl.send_cmd(&format!("DELE {}", path)).await?;
            ctrl.expect(250).await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn ftp_rename(
    conn_id: String,
    old_path: String,
    new_path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    if old_path.is_empty() || new_path.is_empty() {
        return Err(FtpError::InvalidOperation("Paths cannot be empty".into()));
    }

    let ctrl_arc = {
        let conns = state.connections.lock().await;
        conns
            .get(&conn_id)
            .cloned()
            .ok_or_else(|| FtpError::NotConnected(conn_id.clone()))?
    };

    let mut ctrl = ctrl_arc.lock().await;
    ctrl.send_cmd(&format!("RNFR {}", old_path)).await?;
    ctrl.expect(350).await?;
    ctrl.send_cmd(&format!("RNTO {}", new_path)).await?;
    ctrl.expect(250).await?;
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_line_directory() {
        let line = "drwxr-xr-x  2 user group     4096 Jan 15 12:00 Documents";
        let entry = parse_list_line(line).unwrap();
        assert_eq!(entry.name, "Documents");
        assert_eq!(entry.size, 4096);
        assert!(matches!(entry.entry_type, FtpEntryType::Directory));
        assert_eq!(entry.permissions.as_deref(), Some("drwxr-xr-x"));
        assert_eq!(entry.modified.as_deref(), Some("Jan 15 12:00"));
    }

    #[test]
    fn test_parse_list_line_file() {
        let line = "-rw-r--r--  1 user group   102400 Feb 28 09:30 file.txt";
        let entry = parse_list_line(line).unwrap();
        assert_eq!(entry.name, "file.txt");
        assert_eq!(entry.size, 102400);
        assert!(matches!(entry.entry_type, FtpEntryType::File));
    }

    #[test]
    fn test_parse_list_line_symlink() {
        let line = "lrwxrwxrwx  1 user group       10 Mar  1 10:00 link -> target";
        let entry = parse_list_line(line).unwrap();
        assert_eq!(entry.name, "link");
        assert!(matches!(entry.entry_type, FtpEntryType::Link));
    }

    #[test]
    fn test_parse_list_line_dot_entries_skipped() {
        assert!(parse_list_line("drwxr-xr-x  2 user group 4096 Jan 1 12:00 .").is_none());
        assert!(parse_list_line("drwxr-xr-x  2 user group 4096 Jan 1 12:00 ..").is_none());
    }

    #[test]
    fn test_ftp_entry_types_serialize() {
        let file_json = serde_json::to_string(&FtpEntryType::File).unwrap();
        assert_eq!(file_json, "\"file\"");

        let dir_json = serde_json::to_string(&FtpEntryType::Directory).unwrap();
        assert_eq!(dir_json, "\"directory\"");

        let link_json = serde_json::to_string(&FtpEntryType::Link).unwrap();
        assert_eq!(link_json, "\"link\"");
    }

    #[test]
    fn test_ftp_state_new() {
        let _state = FtpState::new();
        // Just verify construction doesn't panic
    }
}

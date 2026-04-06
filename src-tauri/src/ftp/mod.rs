use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
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
}

impl Serialize for FtpError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

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

#[allow(dead_code)]
#[derive(Debug)]
pub struct FtpConnection {
    pub id: String,
    pub config: FtpConfig,
    pub connected: bool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct FtpState {
    pub connections: Mutex<HashMap<String, FtpConnection>>,
}

impl FtpState {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn ftp_connect(
    config: FtpConfig,
    state: tauri::State<'_, FtpState>,
) -> Result<String, FtpError> {
    // Validate host is not empty
    if config.host.is_empty() {
        return Err(FtpError::ConnectionFailed("Host cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();

    // In a full implementation, establish the actual FTP connection here
    // using an FTP client library. For now, we store the connection metadata.
    let conn = FtpConnection {
        id: id.clone(),
        config,
        connected: true,
    };

    state.connections.lock().unwrap().insert(id.clone(), conn);
    Ok(id)
}

#[tauri::command]
pub async fn ftp_disconnect(
    conn_id: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let mut conns = state.connections.lock().unwrap();
    match conns.remove(&conn_id) {
        Some(_) => Ok(()),
        None => Err(FtpError::NotConnected(conn_id)),
    }
}

#[tauri::command]
pub async fn ftp_list(
    conn_id: String,
    path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<Vec<FtpEntry>, FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }

    // In a full implementation, list directory contents via FTP protocol.
    // Return empty list as placeholder.
    let _ = path;
    Ok(vec![])
}

#[tauri::command]
pub async fn ftp_upload(
    conn_id: String,
    local_path: String,
    remote_path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }

    // Validate paths
    if local_path.is_empty() || remote_path.is_empty() {
        return Err(FtpError::InvalidOperation("Paths cannot be empty".into()));
    }

    // In a full implementation, upload the file via FTP protocol.
    Ok(())
}

#[tauri::command]
pub async fn ftp_download(
    conn_id: String,
    remote_path: String,
    local_path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }

    if remote_path.is_empty() || local_path.is_empty() {
        return Err(FtpError::InvalidOperation("Paths cannot be empty".into()));
    }

    // In a full implementation, download the file via FTP protocol.
    Ok(())
}

#[tauri::command]
pub async fn ftp_mkdir(
    conn_id: String,
    path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }
    let _ = path;
    Ok(())
}

#[tauri::command]
pub async fn ftp_delete(
    conn_id: String,
    path: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }
    let _ = path;
    Ok(())
}

#[tauri::command]
pub async fn ftp_rename(
    conn_id: String,
    from: String,
    to: String,
    state: tauri::State<'_, FtpState>,
) -> Result<(), FtpError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&conn_id) {
        return Err(FtpError::NotConnected(conn_id));
    }
    let _ = (from, to);
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ftp_connection_lifecycle() {
        let state = FtpState::new();
        let id = Uuid::new_v4().to_string();

        let conn = FtpConnection {
            id: id.clone(),
            config: FtpConfig {
                host: "ftp.example.com".to_string(),
                port: 21,
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                use_tls: false,
                passive_mode: true,
            },
            connected: true,
        };

        state.connections.lock().unwrap().insert(id.clone(), conn);
        assert!(state.connections.lock().unwrap().contains_key(&id));

        state.connections.lock().unwrap().remove(&id);
        assert!(!state.connections.lock().unwrap().contains_key(&id));
    }

    #[test]
    fn test_ftp_list_mock() {
        let entries = vec![
            FtpEntry {
                name: "documents".to_string(),
                size: 0,
                entry_type: FtpEntryType::Directory,
                modified: Some("2025-01-01T00:00:00Z".to_string()),
                permissions: Some("drwxr-xr-x".to_string()),
            },
            FtpEntry {
                name: "readme.txt".to_string(),
                size: 1024,
                entry_type: FtpEntryType::File,
                modified: Some("2025-01-01T00:00:00Z".to_string()),
                permissions: Some("-rw-r--r--".to_string()),
            },
        ];

        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0].entry_type, FtpEntryType::Directory));
        assert!(matches!(entries[1].entry_type, FtpEntryType::File));
    }

    #[test]
    fn test_ftp_entry_types() {
        let file_json = serde_json::to_string(&FtpEntryType::File).unwrap();
        assert_eq!(file_json, "\"file\"");

        let dir_json = serde_json::to_string(&FtpEntryType::Directory).unwrap();
        assert_eq!(dir_json, "\"directory\"");

        let link_json = serde_json::to_string(&FtpEntryType::Link).unwrap();
        assert_eq!(link_json, "\"link\"");
    }
}

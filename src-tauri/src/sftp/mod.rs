use chrono::Utc;
use serde::{Deserialize, Serialize};
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, RwLock};
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SftpError {
    #[error("SFTP session not found: {0}")]
    NotFound(String),
    #[error("SSH connection not found: {0}")]
    SshNotFound(String),
    #[error("SFTP error: {0}")]
    Sftp(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Transfer cancelled")]
    Cancelled,
}

impl Serialize for SftpError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<russh::Error> for SftpError {
    fn from(e: russh::Error) -> Self {
        SftpError::Sftp(e.to_string())
    }
}

impl From<russh_sftp::client::error::Error> for SftpError {
    fn from(e: russh_sftp::client::error::Error) -> Self {
        SftpError::Sftp(e.to_string())
    }
}

impl From<std::io::Error> for SftpError {
    fn from(e: std::io::Error) -> Self {
        SftpError::Io(e.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
    pub permissions: Option<String>,
    pub owner: Option<u32>,
    pub group: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStat {
    pub size: u64,
    pub is_dir: bool,
    pub permissions: Option<String>,
    pub modified: Option<String>,
    pub accessed: Option<String>,
    pub owner: Option<u32>,
    pub group: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpSessionInfo {
    pub session_id: String,
    pub connection_id: String,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
struct TransferProgressEvent {
    transfer_id: String,
    session_id: String,
    filename: String,
    bytes_transferred: u64,
    total_bytes: u64,
    direction: String,
}

#[derive(Clone, Serialize)]
struct TransferCompleteEvent {
    transfer_id: String,
    session_id: String,
    filename: String,
    direction: String,
    success: bool,
    error: Option<String>,
}

// ── Internal session ────────────────────────────────────────────────────

struct SftpInternalSession {
    info: SftpSessionInfo,
    sftp: SftpSession,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SftpState {
    sessions: Arc<RwLock<HashMap<String, Arc<TokioMutex<SftpInternalSession>>>>>,
}

impl SftpState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn format_permissions(attrs: &russh_sftp::protocol::FileAttributes) -> Option<String> {
    attrs.permissions.map(|p| format!("{:o}", p))
}

fn timestamp_to_iso(secs: Option<u32>) -> Option<String> {
    secs.and_then(|s| {
        chrono::DateTime::from_timestamp(s as i64, 0)
            .map(|dt| dt.to_rfc3339())
    })
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn sftp_open(
    ssh_state: tauri::State<'_, crate::ssh::SshState>,
    state: tauri::State<'_, SftpState>,
    connection_id: String,
) -> Result<String, SftpError> {
    let session_id = Uuid::new_v4().to_string();

    // Get the SSH connection handle
    let ssh_connections = ssh_state.connections.read().await;
    let ssh_conn = ssh_connections
        .get(&connection_id)
        .ok_or_else(|| SftpError::SshNotFound(connection_id.clone()))?
        .clone();

    let ssh_conn_locked = ssh_conn.lock().await;

    // Open a new channel for SFTP
    let channel = ssh_conn_locked
        .handle
        .channel_open_session()
        .await
        .map_err(|e| SftpError::Sftp(format!("Failed to open SFTP channel: {}", e)))?;

    // Request SFTP subsystem
    channel
        .request_subsystem(false, "sftp")
        .await
        .map_err(|e| SftpError::Sftp(format!("Failed to request SFTP subsystem: {}", e)))?;

    // Create SFTP session
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| SftpError::Sftp(format!("Failed to initialize SFTP: {}", e)))?;

    let internal = SftpInternalSession {
        info: SftpSessionInfo {
            session_id: session_id.clone(),
            connection_id,
            created_at: Utc::now().to_rfc3339(),
        },
        sftp,
    };

    let mut sessions = state.sessions.write().await;
    sessions.insert(session_id.clone(), Arc::new(TokioMutex::new(internal)));

    Ok(session_id)
}

#[tauri::command]
pub async fn sftp_close(
    state: tauri::State<'_, SftpState>,
    session_id: String,
) -> Result<(), SftpError> {
    let mut sessions = state.sessions.write().await;
    sessions
        .remove(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id))?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_list(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
) -> Result<Vec<FileEntry>, SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    let entries = session.sftp.read_dir(&path).await?;

    let mut result = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let attrs = entry.metadata();
        let is_dir = attrs.is_dir();
        let size = attrs.size.unwrap_or(0);
        let modified = timestamp_to_iso(attrs.mtime);
        let permissions = format_permissions(&attrs);
        let owner = attrs.uid;
        let group = attrs.gid;

        result.push(FileEntry {
            name,
            is_dir,
            size,
            modified,
            permissions,
            owner,
            group,
        });
    }

    result.sort_by(|a, b| {
        if a.is_dir && !b.is_dir {
            std::cmp::Ordering::Less
        } else if !a.is_dir && b.is_dir {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok(result)
}

#[tauri::command]
pub async fn sftp_stat(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
) -> Result<FileStat, SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    let attrs = session.sftp.metadata(&path).await?;

    Ok(FileStat {
        size: attrs.size.unwrap_or(0),
        is_dir: attrs.is_dir(),
        permissions: format_permissions(&attrs),
        modified: timestamp_to_iso(attrs.mtime),
        accessed: timestamp_to_iso(attrs.atime),
        owner: attrs.uid,
        group: attrs.gid,
    })
}

#[tauri::command]
pub async fn sftp_mkdir(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
) -> Result<(), SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    session.sftp.create_dir(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_rmdir(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
) -> Result<(), SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    session.sftp.remove_dir(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_delete(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
) -> Result<(), SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    session.sftp.remove_file(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_rename(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    session.sftp.rename(&old_path, &new_path).await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_read_file(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    remote_path: String,
) -> Result<Vec<u8>, SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    let data = session.sftp.read(&remote_path).await?;
    Ok(data)
}

#[tauri::command]
pub async fn sftp_write_file(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    remote_path: String,
    data: Vec<u8>,
) -> Result<(), SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session = session.lock().await;
    session.sftp.write(&remote_path, &data).await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_upload(
    app_handle: AppHandle,
    state: tauri::State<'_, SftpState>,
    session_id: String,
    local_path: String,
    remote_path: String,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let data = tokio::fs::read(&local_path).await?;
    let total_bytes = data.len() as u64;
    let filename = std::path::Path::new(&local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // Emit initial progress
    let _ = app_handle.emit(
        "sftp:transfer_progress",
        TransferProgressEvent {
            transfer_id: transfer_id.clone(),
            session_id: session_id.clone(),
            filename: filename.clone(),
            bytes_transferred: 0,
            total_bytes,
            direction: "upload".into(),
        },
    );

    let session_locked = session.lock().await;
    let result = session_locked.sftp.write(&remote_path, &data).await;

    match result {
        Ok(()) => {
            let _ = app_handle.emit(
                "sftp:transfer_progress",
                TransferProgressEvent {
                    transfer_id: transfer_id.clone(),
                    session_id: session_id.clone(),
                    filename: filename.clone(),
                    bytes_transferred: total_bytes,
                    total_bytes,
                    direction: "upload".into(),
                },
            );
            let _ = app_handle.emit(
                "sftp:transfer_complete",
                TransferCompleteEvent {
                    transfer_id: transfer_id.clone(),
                    session_id: session_id.clone(),
                    filename,
                    direction: "upload".into(),
                    success: true,
                    error: None,
                },
            );
            Ok(transfer_id)
        }
        Err(e) => {
            let err_msg = e.to_string();
            let _ = app_handle.emit(
                "sftp:transfer_complete",
                TransferCompleteEvent {
                    transfer_id: transfer_id.clone(),
                    session_id: session_id.clone(),
                    filename,
                    direction: "upload".into(),
                    success: false,
                    error: Some(err_msg.clone()),
                },
            );
            Err(SftpError::Sftp(err_msg))
        }
    }
}

#[tauri::command]
pub async fn sftp_download(
    app_handle: AppHandle,
    state: tauri::State<'_, SftpState>,
    session_id: String,
    remote_path: String,
    local_path: String,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let session_locked = session.lock().await;
    let data = session_locked.sftp.read(&remote_path).await?;
    let total_bytes = data.len() as u64;
    drop(session_locked);

    let _ = app_handle.emit(
        "sftp:transfer_progress",
        TransferProgressEvent {
            transfer_id: transfer_id.clone(),
            session_id: session_id.clone(),
            filename: filename.clone(),
            bytes_transferred: total_bytes,
            total_bytes,
            direction: "download".into(),
        },
    );

    tokio::fs::write(&local_path, &data).await?;

    let _ = app_handle.emit(
        "sftp:transfer_complete",
        TransferCompleteEvent {
            transfer_id: transfer_id.clone(),
            session_id: session_id.clone(),
            filename,
            direction: "download".into(),
            success: true,
            error: None,
        },
    );

    Ok(transfer_id)
}

// ── BE-SFTP-02: SCP transfer fallback ───────────────────────────────────

#[tauri::command]
pub async fn sftp_scp_upload(
    ssh_state: tauri::State<'_, crate::ssh::SshState>,
    app_handle: AppHandle,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let data = tokio::fs::read(&local_path).await?;
    let total_bytes = data.len() as u64;
    let filename = std::path::Path::new(&local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let connections = ssh_state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SftpError::SshNotFound(connection_id.clone()))?
        .clone();

    let conn_locked = conn.lock().await;
    let channel = conn_locked
        .handle
        .channel_open_session()
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP channel open failed: {}", e)))?;

    // SCP protocol: exec "scp -t <remote_path>"
    channel
        .exec(true, format!("scp -t {}", remote_path).as_bytes())
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP exec failed: {}", e)))?;

    // Send SCP header: C0644 <size> <filename>\n
    let header = format!("C0644 {} {}\n", total_bytes, filename);
    channel
        .data(header.as_bytes())
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP header send failed: {}", e)))?;

    // Send file data
    channel
        .data(&data[..])
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP data send failed: {}", e)))?;

    // Send null byte to indicate end of file
    channel
        .data(&[0u8][..])
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP EOF send failed: {}", e)))?;

    let _ = app_handle.emit(
        "sftp:transfer_complete",
        TransferCompleteEvent {
            transfer_id: transfer_id.clone(),
            session_id: connection_id,
            filename,
            direction: "scp_upload".into(),
            success: true,
            error: None,
        },
    );

    Ok(transfer_id)
}

#[tauri::command]
pub async fn sftp_scp_download(
    ssh_state: tauri::State<'_, crate::ssh::SshState>,
    app_handle: AppHandle,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let connections = ssh_state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SftpError::SshNotFound(connection_id.clone()))?
        .clone();

    let conn_locked = conn.lock().await;
    let channel = conn_locked
        .handle
        .channel_open_session()
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP channel open failed: {}", e)))?;

    // SCP protocol: exec "scp -f <remote_path>"
    channel
        .exec(true, format!("scp -f {}", remote_path).as_bytes())
        .await
        .map_err(|e| SftpError::Sftp(format!("SCP exec failed: {}", e)))?;

    drop(conn_locked);

    // Collect all data from the channel
    use russh::ChannelMsg;
    let mut ch = channel;
    let mut output = Vec::new();
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => {
                output.extend_from_slice(&data);
            }
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }

    // Parse SCP response header: skip the C<mode> <size> <name>\n prefix
    let header_end = output.iter().position(|&b| b == b'\n').unwrap_or(0);
    let file_data = if header_end + 1 < output.len() {
        &output[header_end + 1..]
    } else {
        &output[..]
    };

    tokio::fs::write(&local_path, file_data).await?;

    let _ = app_handle.emit(
        "sftp:transfer_complete",
        TransferCompleteEvent {
            transfer_id: transfer_id.clone(),
            session_id: connection_id,
            filename,
            direction: "scp_download".into(),
            success: true,
            error: None,
        },
    );

    Ok(transfer_id)
}

// ── BE-SFTP-03: Bandwidth throttling ────────────────────────────────────

#[tauri::command]
pub async fn sftp_upload_throttled(
    app_handle: AppHandle,
    state: tauri::State<'_, SftpState>,
    session_id: String,
    local_path: String,
    remote_path: String,
    max_bytes_per_sec: Option<u64>,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let data = tokio::fs::read(&local_path).await?;
    let total_bytes = data.len() as u64;
    let filename = std::path::Path::new(&local_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // If throttling is enabled, write in chunks with delays (token bucket)
    if let Some(rate_limit) = max_bytes_per_sec {
        if rate_limit > 0 {
            let chunk_size = (rate_limit / 10).max(1024) as usize; // 100ms intervals
            let interval = std::time::Duration::from_millis(100);
            let mut offset = 0usize;

            let session_locked = session.lock().await;
            while offset < data.len() {
                let end = (offset + chunk_size).min(data.len());
                session_locked
                    .sftp
                    .write(&remote_path, &data[offset..end])
                    .await
                    .map_err(|e| SftpError::Sftp(e.to_string()))?;

                offset = end;

                let _ = app_handle.emit(
                    "sftp:transfer_progress",
                    TransferProgressEvent {
                        transfer_id: transfer_id.clone(),
                        session_id: session_id.clone(),
                        filename: filename.clone(),
                        bytes_transferred: offset as u64,
                        total_bytes,
                        direction: "upload".into(),
                    },
                );

                if offset < data.len() {
                    tokio::time::sleep(interval).await;
                }
            }
        } else {
            let session_locked = session.lock().await;
            session_locked
                .sftp
                .write(&remote_path, &data)
                .await
                .map_err(|e| SftpError::Sftp(e.to_string()))?;
        }
    } else {
        let session_locked = session.lock().await;
        session_locked
            .sftp
            .write(&remote_path, &data)
            .await
            .map_err(|e| SftpError::Sftp(e.to_string()))?;
    }

    let _ = app_handle.emit(
        "sftp:transfer_complete",
        TransferCompleteEvent {
            transfer_id: transfer_id.clone(),
            session_id: session_id.clone(),
            filename,
            direction: "upload".into(),
            success: true,
            error: None,
        },
    );

    Ok(transfer_id)
}

#[tauri::command]
pub async fn sftp_download_throttled(
    app_handle: AppHandle,
    state: tauri::State<'_, SftpState>,
    session_id: String,
    remote_path: String,
    local_path: String,
    max_bytes_per_sec: Option<u64>,
) -> Result<String, SftpError> {
    let transfer_id = Uuid::new_v4().to_string();
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let filename = std::path::Path::new(&remote_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let session_locked = session.lock().await;
    let data = session_locked.sftp.read(&remote_path).await?;
    let total_bytes = data.len() as u64;
    drop(session_locked);

    // Apply throttling to the write to disk
    if let Some(rate_limit) = max_bytes_per_sec {
        if rate_limit > 0 {
            let chunk_size = (rate_limit / 10).max(1024) as usize;
            let interval = std::time::Duration::from_millis(100);
            let mut file = tokio::fs::File::create(&local_path).await?;
            let mut offset = 0usize;

            while offset < data.len() {
                let end = (offset + chunk_size).min(data.len());
                tokio::io::AsyncWriteExt::write_all(&mut file, &data[offset..end]).await?;
                offset = end;

                let _ = app_handle.emit(
                    "sftp:transfer_progress",
                    TransferProgressEvent {
                        transfer_id: transfer_id.clone(),
                        session_id: session_id.clone(),
                        filename: filename.clone(),
                        bytes_transferred: offset as u64,
                        total_bytes,
                        direction: "download".into(),
                    },
                );

                if offset < data.len() {
                    tokio::time::sleep(interval).await;
                }
            }
        } else {
            tokio::fs::write(&local_path, &data).await?;
        }
    } else {
        tokio::fs::write(&local_path, &data).await?;
    }

    let _ = app_handle.emit(
        "sftp:transfer_complete",
        TransferCompleteEvent {
            transfer_id: transfer_id.clone(),
            session_id: session_id.clone(),
            filename,
            direction: "download".into(),
            success: true,
            error: None,
        },
    );

    Ok(transfer_id)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_state_new() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = SftpState::new();
            let sessions = state.sessions.read().await;
            assert!(sessions.is_empty());
        });
    }

    #[test]
    fn test_sftp_error_display() {
        let err = SftpError::NotFound("sess-1".into());
        assert_eq!(err.to_string(), "SFTP session not found: sess-1");

        let err = SftpError::SshNotFound("conn-1".into());
        assert_eq!(err.to_string(), "SSH connection not found: conn-1");

        let err = SftpError::Cancelled;
        assert_eq!(err.to_string(), "Transfer cancelled");
    }

    #[test]
    fn test_sftp_error_serialize() {
        let err = SftpError::NotFound("x".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"SFTP session not found: x\"");
    }

    #[test]
    fn test_file_entry_serde() {
        let entry = FileEntry {
            name: "test.txt".into(),
            is_dir: false,
            size: 1024,
            modified: Some("2024-01-01T00:00:00Z".into()),
            permissions: Some("644".into()),
            owner: Some(1000),
            group: Some(1000),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FileEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test.txt");
        assert_eq!(deserialized.size, 1024);
    }

    #[test]
    fn test_transfer_progress_event_serde() {
        let event = TransferProgressEvent {
            transfer_id: "xfer-1".into(),
            session_id: "sess-1".into(),
            filename: "file.bin".into(),
            bytes_transferred: 500,
            total_bytes: 1000,
            direction: "upload".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("bytes_transferred"));
        assert!(json.contains("500"));
    }

    #[test]
    fn test_transfer_complete_event_serde() {
        let event = TransferCompleteEvent {
            transfer_id: "xfer-1".into(),
            session_id: "sess-1".into(),
            filename: "file.bin".into(),
            direction: "download".into(),
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    // ── BE-SFTP-03: Throttle chunk calculation ──────────────────────

    #[test]
    fn test_throttle_chunk_size_calculation() {
        let rate: u64 = 1_000_000; // 1MB/s
        let chunk_size = (rate / 10).max(1024) as usize;
        assert_eq!(chunk_size, 100_000); // 100KB chunks at 100ms intervals

        let low_rate: u64 = 100; // Very low rate
        let low_chunk = (low_rate / 10).max(1024) as usize;
        assert_eq!(low_chunk, 1024); // Minimum 1KB
    }

    #[test]
    fn test_sftp_session_info_serde() {
        let info = SftpSessionInfo {
            session_id: "s1".into(),
            connection_id: "c1".into(),
            created_at: "2024-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let d: SftpSessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d.session_id, "s1");
    }
}

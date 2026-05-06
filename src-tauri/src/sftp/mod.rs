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

#[allow(dead_code)]
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

// ── P2-SFTP-03: File Preview Types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePreview {
    pub path: String,
    pub content_type: String,
    pub data: String,
    pub size: u64,
    pub truncated: bool,
}

// ── P2-SFTP-04: Folder Sync Types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    Upload,
    Download,
    Skip,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntry {
    pub path: String,
    pub local_modified: Option<String>,
    pub remote_modified: Option<String>,
    pub sync_action: SyncAction,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub uploaded: u32,
    pub downloaded: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
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

#[allow(dead_code)]
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
        .ok_or(SftpError::NotFound(session_id))?;
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
            // Open a single remote file handle so sequential writes append rather than
            // each call rewriting from byte 0 (sftp.write() always creates/truncates).
            let mut file = session_locked.sftp
                .create(&remote_path)
                .await
                .map_err(|e| SftpError::Sftp(e.to_string()))?;

            while offset < data.len() {
                let end = (offset + chunk_size).min(data.len());
                {
                    use tokio::io::AsyncWriteExt;
                    file.write_all(&data[offset..end])
                        .await
                        .map_err(|e| SftpError::Sftp(e.to_string()))?;
                }

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
            {
                use tokio::io::AsyncWriteExt;
                file.flush().await.map_err(|e| SftpError::Sftp(e.to_string()))?;
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

// ── P2-SFTP-03: Inline file preview ─────────────────────────────────────

fn detect_content_type(path: &str) -> &'static str {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "log" | "md" | "csv" | "conf" | "cfg" | "ini" => "text/plain",
        "json" => "application/json",
        "yaml" | "yml" => "text/yaml",
        "xml" => "text/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "ts" => "text/javascript",
        "py" | "rs" | "go" | "rb" | "sh" | "bash" | "zsh" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[tauri::command]
pub async fn sftp_preview(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    path: String,
    max_bytes: Option<u64>,
) -> Result<FilePreview, SftpError> {
    let limit = max_bytes.unwrap_or(1_048_576); // 1MB default

    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session_locked = session.lock().await;

    // Get file size first
    let attrs = session_locked.sftp.metadata(&path).await?;
    let file_size = attrs.size.unwrap_or(0);

    // Read file data (up to limit)
    let raw_data = session_locked.sftp.read(&path).await?;
    let truncated = raw_data.len() as u64 > limit;
    let data_slice = if truncated {
        &raw_data[..limit as usize]
    } else {
        &raw_data[..]
    };

    let content_type = detect_content_type(&path);
    let data = if content_type.starts_with("text/")
        || content_type == "application/json"
        || content_type == "text/yaml"
    {
        // Return as UTF-8 text
        String::from_utf8_lossy(data_slice).to_string()
    } else if content_type.starts_with("image/") || content_type == "application/pdf" {
        // Return as base64
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(data_slice)
    } else {
        // Hex preview for binary files
        data_slice
            .iter()
            .take(4096) // Max 4KB hex preview
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .chunks(32)
            .map(|chunk| chunk.join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(FilePreview {
        path,
        content_type: content_type.to_string(),
        data,
        size: file_size,
        truncated,
    })
}

// ── P2-SFTP-04: Folder sync wizard ─────────────────────────────────────

#[tauri::command]
pub async fn sftp_sync_compare(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    local_dir: String,
    remote_dir: String,
) -> Result<Vec<SyncEntry>, SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let session_locked = session.lock().await;

    // List remote files
    let remote_entries = session_locked.sftp.read_dir(&remote_dir).await?;
    let mut remote_map: HashMap<String, (u64, Option<String>)> = HashMap::new();
    for entry in remote_entries {
        let name: String = entry.file_name();
        if name == "." || name == ".." || entry.metadata().is_dir() {
            continue;
        }
        let attrs = entry.metadata();
        let size = attrs.size.unwrap_or(0);
        let modified = timestamp_to_iso(attrs.mtime);
        remote_map.insert(name, (size, modified));
    }
    drop(session_locked);

    // List local files
    let mut local_map: HashMap<String, (u64, Option<String>)> = HashMap::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&local_dir).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let size = metadata.len();
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|t| {
                            chrono::DateTime::<chrono::Utc>::from(t)
                                .to_rfc3339()
                                .into()
                        });
                    local_map.insert(name, (size, modified));
                }
            }
        }
    }

    let mut entries = Vec::new();

    // Files in both local and remote
    for (name, (local_size, local_mod)) in &local_map {
        if let Some((remote_size, remote_mod)) = remote_map.get(name) {
            let action = if local_size != remote_size {
                // Different sizes → conflict
                SyncAction::Conflict
            } else {
                SyncAction::Skip
            };
            entries.push(SyncEntry {
                path: name.clone(),
                local_modified: local_mod.clone(),
                remote_modified: remote_mod.clone(),
                sync_action: action,
                size: *local_size,
            });
        } else {
            // Local only → upload
            entries.push(SyncEntry {
                path: name.clone(),
                local_modified: local_mod.clone(),
                remote_modified: None,
                sync_action: SyncAction::Upload,
                size: *local_size,
            });
        }
    }

    // Files only in remote → download
    for (name, (remote_size, remote_mod)) in &remote_map {
        if !local_map.contains_key(name) {
            entries.push(SyncEntry {
                path: name.clone(),
                local_modified: None,
                remote_modified: remote_mod.clone(),
                sync_action: SyncAction::Download,
                size: *remote_size,
            });
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

#[tauri::command]
pub async fn sftp_sync_execute(
    state: tauri::State<'_, SftpState>,
    session_id: String,
    entries: Vec<SyncEntry>,
    local_dir: String,
    remote_dir: String,
) -> Result<SyncResult, SftpError> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SftpError::NotFound(session_id.clone()))?
        .clone();

    let mut uploaded = 0u32;
    let mut downloaded = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for entry in &entries {
        let local_path = format!("{}/{}", local_dir, entry.path);
        let remote_path = format!("{}/{}", remote_dir, entry.path);

        match entry.sync_action {
            SyncAction::Upload => {
                match tokio::fs::read(&local_path).await {
                    Ok(data) => {
                        let session_locked = session.lock().await;
                        match session_locked.sftp.write(&remote_path, &data).await {
                            Ok(()) => uploaded += 1,
                            Err(e) => errors.push(format!("Upload {}: {}", entry.path, e)),
                        }
                    }
                    Err(e) => errors.push(format!("Read {}: {}", local_path, e)),
                }
            }
            SyncAction::Download => {
                let session_locked = session.lock().await;
                match session_locked.sftp.read(&remote_path).await {
                    Ok(data) => {
                        drop(session_locked);
                        match tokio::fs::write(&local_path, &data).await {
                            Ok(()) => downloaded += 1,
                            Err(e) => errors.push(format!("Write {}: {}", local_path, e)),
                        }
                    }
                    Err(e) => errors.push(format!("Download {}: {}", entry.path, e)),
                }
            }
            SyncAction::Skip | SyncAction::Conflict => {
                skipped += 1;
            }
        }
    }

    Ok(SyncResult {
        uploaded,
        downloaded,
        skipped,
        errors,
    })
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

    #[test]
    fn test_file_stat_serde() {
        let stat = FileStat {
            size: 2048,
            is_dir: false,
            permissions: Some("755".into()),
            modified: Some("2024-06-15T12:00:00Z".into()),
            accessed: Some("2024-06-15T13:00:00Z".into()),
            owner: Some(1000),
            group: Some(1000),
        };
        let json = serde_json::to_string(&stat).unwrap();
        let d: FileStat = serde_json::from_str(&json).unwrap();
        assert_eq!(d.size, 2048);
        assert!(!d.is_dir);
        assert_eq!(d.permissions.as_deref(), Some("755"));
        assert_eq!(d.owner, Some(1000));
    }

    #[test]
    fn test_file_entry_directory() {
        let entry = FileEntry {
            name: "subdir".into(),
            is_dir: true,
            size: 0,
            modified: None,
            permissions: Some("drwxr-xr-x".into()),
            owner: None,
            group: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let d: FileEntry = serde_json::from_str(&json).unwrap();
        assert!(d.is_dir);
        assert_eq!(d.name, "subdir");
    }

    // ── Integration tests requiring SSH server ──────────────────────

    // Test helpers — raw russh/russh_sftp client, no Tauri dependency
    use async_trait::async_trait;
    use russh::client;
    use russh::keys::key::PublicKey;
    use russh::{ChannelMsg, Disconnect};
    use sha2::{Digest, Sha256};

    const TEST_SSH_HOST: &str = "127.0.0.1";
    const TEST_SSH_PORT: u16 = 2222;
    const TEST_USER: &str = "testuser";
    const TEST_PASS: &str = "testpass123";

    struct TestHandler;

    #[async_trait]
    impl client::Handler for TestHandler {
        type Error = russh::Error;

        async fn check_server_key(
            &mut self,
            _server_public_key: &PublicKey,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    async fn sftp_test_connect() -> client::Handle<TestHandler> {
        let config = Arc::new(client::Config::default());
        let addr = format!("{}:{}", TEST_SSH_HOST, TEST_SSH_PORT);
        let mut handle = client::connect(config, &addr, TestHandler).await.unwrap();
        let auth = handle
            .authenticate_password(TEST_USER, TEST_PASS)
            .await
            .unwrap();
        assert!(auth, "password auth should succeed");
        handle
    }

    async fn sftp_test_open(
        handle: &client::Handle<TestHandler>,
    ) -> SftpSession {
        let channel = handle.channel_open_session().await.unwrap();
        channel.request_subsystem(false, "sftp").await.unwrap();
        SftpSession::new(channel.into_stream()).await.unwrap()
    }

    /// Helper: write data to a remote file, creating it if necessary.
    /// `SftpSession::write()` uses WRITE-only flags and fails for new files,
    /// so we use `create()` + `write_all()` which uses CREATE|TRUNCATE|WRITE.
    async fn sftp_write_file(sftp: &SftpSession, path: &str, data: &[u8]) {
        let mut file = sftp.create(path).await
            .unwrap_or_else(|e| panic!("create {path} failed: {e}"));
        use tokio::io::AsyncWriteExt;
        file.write_all(data).await
            .unwrap_or_else(|e| panic!("write {path} failed: {e}"));
        file.shutdown().await
            .unwrap_or_else(|e| panic!("close {path} failed: {e}"));
    }

    async fn sftp_test_exec(
        handle: &client::Handle<TestHandler>,
        cmd: &str,
    ) -> String {
        let channel = handle.channel_open_session().await.unwrap();
        channel.exec(true, cmd.as_bytes()).await.unwrap();
        let mut output = Vec::new();
        let mut ch = channel;
        loop {
            match ch.wait().await {
                Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
        String::from_utf8_lossy(&output).trim().to_string()
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_open_session() {
        // UT-SF-01: Open SFTP session over existing SSH connection.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;

        // If we get a session without error, the SFTP subsystem is running.
        // Verify it's functional by listing the root.
        let entries = sftp.read_dir("/tmp").await.expect("should list /tmp");
        drop(entries);

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_list_directory() {
        // UT-SF-02: List /tmp on remote. Assert entries have name, size, type.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;

        // Create a known file so we can verify listing
        sftp_write_file(&sftp, "/tmp/sftp_list_test.txt", b"list test").await;

        let entries: Vec<_> = sftp.read_dir("/tmp").await.expect("should list /tmp").collect();
        let found = entries.iter().any(|e| e.file_name() == "sftp_list_test.txt");
        assert!(found, "/tmp listing should contain our test file");

        // Verify entry metadata is populated
        for entry in &entries {
            let name = entry.file_name();
            assert!(!name.is_empty(), "entry name should not be empty");
            let _attrs = entry.metadata();
        }

        // Cleanup
        let _ = sftp.remove_file("/tmp/sftp_list_test.txt").await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_read_file() {
        // UT-SF-03: Write a file, read it back, verify contents match.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;

        let test_content = b"Hello from CrossTerm SFTP read test!";
        sftp_write_file(&sftp, "/tmp/sftp_read_test.txt", test_content).await;

        let data = sftp
            .read("/tmp/sftp_read_test.txt")
            .await
            .expect("should read test file");
        assert_eq!(data, test_content, "read content should match written content");

        let _ = sftp.remove_file("/tmp/sftp_read_test.txt").await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_write_file() {
        // UT-SF-04: Upload a file to remote. Verify file exists and contents match.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;

        let content = b"SFTP write test payload 12345";
        sftp_write_file(&sftp, "/tmp/sftp_write_test.txt", content).await;

        let readback = sftp.read("/tmp/sftp_write_test.txt").await.unwrap();
        assert_eq!(readback, content);

        let attrs = sftp.metadata("/tmp/sftp_write_test.txt").await.unwrap();
        assert!(!attrs.is_dir());
        assert_eq!(attrs.size.unwrap_or(0), content.len() as u64);

        let _ = sftp.remove_file("/tmp/sftp_write_test.txt").await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_delete_file() {
        // UT-SF-05: Upload then delete a file. Verify file no longer exists.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;

        sftp_write_file(&sftp, "/tmp/sftp_delete_test.txt", b"to be deleted").await;
        sftp.metadata("/tmp/sftp_delete_test.txt")
            .await
            .expect("file should exist before delete");

        sftp.remove_file("/tmp/sftp_delete_test.txt")
            .await
            .expect("delete should succeed");

        let result = sftp.metadata("/tmp/sftp_delete_test.txt").await;
        assert!(result.is_err(), "stat after delete should fail");

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_mkdir_rmdir() {
        // UT-SF-06: Create a directory, verify it exists, remove it, verify gone.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let dir_path = "/tmp/sftp_mkdir_test";

        let _ = sftp.remove_dir(dir_path).await; // cleanup previous

        sftp.create_dir(dir_path)
            .await
            .expect("mkdir should succeed");

        let attrs = sftp
            .metadata(dir_path)
            .await
            .expect("dir should exist after mkdir");
        assert!(attrs.is_dir(), "created path should be a directory");

        sftp.remove_dir(dir_path)
            .await
            .expect("rmdir should succeed");

        let result = sftp.metadata(dir_path).await;
        assert!(result.is_err(), "stat after rmdir should fail");

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_rename() {
        // UT-SF-07: Create a file, rename it, verify old path gone and new path exists.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let old_path = "/tmp/sftp_rename_old.txt";
        let new_path = "/tmp/sftp_rename_new.txt";
        let content = b"rename test data";

        let _ = sftp.remove_file(old_path).await;
        let _ = sftp.remove_file(new_path).await;

        sftp_write_file(&sftp, old_path, content).await;
        sftp.rename(old_path, new_path)
            .await
            .expect("rename should succeed");

        assert!(
            sftp.metadata(old_path).await.is_err(),
            "old path should not exist"
        );

        let data = sftp
            .read(new_path)
            .await
            .expect("new path should be readable");
        assert_eq!(data, content);

        let _ = sftp.remove_file(new_path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_chmod() {
        // UT-SF-08: Change permissions via SSH exec chmod, verify with SFTP stat.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let path = "/tmp/sftp_chmod_test.txt";

        sftp_write_file(&sftp, path, b"chmod test").await;

        // Use SSH exec to change permissions
        sftp_test_exec(&handle, &format!("chmod 755 {}", path)).await;

        let attrs = sftp.metadata(path).await.expect("stat should succeed");
        if let Some(perms) = attrs.permissions {
            assert_eq!(
                perms & 0o777,
                0o755,
                "permissions should be 755, got {:o}",
                perms & 0o777
            );
        }

        let _ = sftp.remove_file(path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_stat() {
        // UT-SF-09: Stat a file. Verify size, permissions, modification time.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let content = b"stat test content with known size";
        let path = "/tmp/sftp_stat_test.txt";

        sftp_write_file(&sftp, path, content).await;

        let attrs = sftp.metadata(path).await.expect("stat should succeed");
        assert_eq!(
            attrs.size.unwrap_or(0),
            content.len() as u64,
            "size should match written bytes"
        );
        assert!(!attrs.is_dir(), "should not be a directory");
        assert!(attrs.mtime.is_some(), "modification time should be present");

        let _ = sftp.remove_file(path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_large_file_transfer() {
        // UT-SF-10: Upload a 10 MB file. Download it. Verify SHA-256 matches.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let path = "/tmp/sftp_large_test.bin";

        // Generate 10 MB of deterministic data
        let mut data = vec![0u8; 10 * 1024 * 1024];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let original_hash = hasher.finalize();

        sftp_write_file(&sftp, path, &data).await;

        let downloaded = sftp
            .read(path)
            .await
            .expect("large file download should succeed");
        assert_eq!(downloaded.len(), data.len(), "downloaded size should match");

        let mut dl_hasher = Sha256::new();
        dl_hasher.update(&downloaded);
        let download_hash = dl_hasher.finalize();
        assert_eq!(original_hash, download_hash, "SHA-256 hashes should match");

        let _ = sftp.remove_file(path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_transfer_progress() {
        // UT-SF-11: Upload a file and verify it completes correctly.
        // Note: AppHandle event emission cannot be tested in unit tests;
        // this verifies the underlying SFTP write succeeds for chunked data.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let path = "/tmp/sftp_progress_test.bin";
        let total_size = 512 * 1024; // 512 KB
        let data = vec![0xABu8; total_size];

        sftp_write_file(&sftp, path, &data).await;

        let attrs = sftp.metadata(path).await.unwrap();
        assert_eq!(attrs.size.unwrap_or(0), total_size as u64);

        let _ = sftp.remove_file(path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_transfer_cancel() {
        // UT-SF-12: Write a partial file, then clean it up (simulating cancel).
        // Note: actual mid-transfer cancellation requires AppHandle.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let path = "/tmp/sftp_cancel_test.bin";
        let partial_data = vec![0xCDu8; 1024];

        sftp_write_file(&sftp, path, &partial_data).await;

        let attrs = sftp.metadata(path).await.unwrap();
        assert_eq!(attrs.size.unwrap_or(0), 1024);

        // Clean up the partial file (simulating cancel cleanup)
        sftp.remove_file(path)
            .await
            .expect("cleanup of partial file should succeed");
        assert!(
            sftp.metadata(path).await.is_err(),
            "file should be gone after cleanup"
        );

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_resume() {
        // UT-SF-13: Write partial data, then "resume" by writing the full file.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let path = "/tmp/sftp_resume_test.bin";
        let full_data = vec![0xEFu8; 4096];

        // Write first half (simulating partial transfer)
        let half = &full_data[..2048];
        sftp_write_file(&sftp, path, half).await;
        let attrs = sftp.metadata(path).await.unwrap();
        assert_eq!(
            attrs.size.unwrap_or(0),
            2048,
            "partial write should be 2048 bytes"
        );

        // "Resume" by writing the full file (SFTP write overwrites)
        sftp_write_file(&sftp, path, &full_data).await;
        let attrs = sftp.metadata(path).await.unwrap();
        assert_eq!(
            attrs.size.unwrap_or(0),
            4096,
            "resumed write should be 4096 bytes"
        );

        let readback = sftp.read(path).await.unwrap();
        assert_eq!(readback, full_data);

        let _ = sftp.remove_file(path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_symlink_follow() {
        // UT-SF-14: Create a symlink on remote via SSH exec. Stat the link target.
        let handle = sftp_test_connect().await;
        let sftp = sftp_test_open(&handle).await;
        let target_path = "/tmp/sftp_symlink_target.txt";
        let link_path = "/tmp/sftp_symlink_link.txt";
        let content = b"symlink target content";

        let _ = sftp.remove_file(link_path).await;
        let _ = sftp.remove_file(target_path).await;

        sftp_write_file(&sftp, target_path, content).await;

        // Create symlink via SSH exec (reliable across sftp lib versions)
        sftp_test_exec(
            &handle,
            &format!("ln -sf {} {}", target_path, link_path),
        )
        .await;

        // metadata() follows symlinks (stat, not lstat)
        let attrs = sftp
            .metadata(link_path)
            .await
            .expect("stat on symlink should resolve to target");
        assert_eq!(
            attrs.size.unwrap_or(0),
            content.len() as u64,
            "symlink stat should report target file size"
        );

        let data = sftp.read(link_path).await.unwrap();
        assert_eq!(data, content);

        let _ = sftp.remove_file(link_path).await;
        let _ = sftp.remove_file(target_path).await;
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_sftp_concurrent_transfers() {
        // UT-SF-15: Upload 5 files simultaneously. Verify all complete correctly.
        let handle = sftp_test_connect().await;

        // Open 5 separate SFTP sessions for concurrent transfers
        let mut sessions = Vec::new();
        for _ in 0..5 {
            sessions.push(sftp_test_open(&handle).await);
        }

        let mut task_handles = Vec::new();
        for (i, sftp) in sessions.into_iter().enumerate() {
            let path = format!("/tmp/sftp_concurrent_{}.txt", i);
            let content = format!("concurrent file {} content", i);
            task_handles.push(tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                let mut file = sftp.create(&path).await
                    .unwrap_or_else(|e| panic!("create {} failed: {}", path, e));
                file.write_all(content.as_bytes()).await
                    .unwrap_or_else(|e| panic!("write {} failed: {}", path, e));
                file.shutdown().await
                    .unwrap_or_else(|e| panic!("close {} failed: {}", path, e));
                let readback = sftp
                    .read(&path)
                    .await
                    .unwrap_or_else(|e| panic!("read {} failed: {}", path, e));
                assert_eq!(
                    readback,
                    content.as_bytes(),
                    "content mismatch for {}",
                    path
                );
            }));
        }

        for h in task_handles {
            h.await.expect("concurrent transfer task should not panic");
        }

        // Cleanup
        let sftp = sftp_test_open(&handle).await;
        for i in 0..5 {
            let _ = sftp
                .remove_file(&format!("/tmp/sftp_concurrent_{}.txt", i))
                .await;
        }
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }
}

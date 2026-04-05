use chrono::Utc;
use russh::client;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
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

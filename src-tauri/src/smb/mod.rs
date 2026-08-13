/// SMB/CIFS file browser using the system smbclient binary.
/// Parses smbclient output to provide directory listing and file operations
/// without adding a native SMB crate dependency.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SmbError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("smbclient binary not found — install samba-client")]
    BinaryNotFound,
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Command error: {0}")]
    CommandError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for SmbError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmbConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub share: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmbSession {
    pub id: String,
    pub host: String,
    pub share: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SmbEntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmbEntry {
    pub name: String,
    pub entry_type: SmbEntryType,
    pub size: u64,
    pub modified: Option<String>,
}

pub struct SmbState {
    sessions: Mutex<HashMap<String, SmbConfig>>,
}

impl SmbState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

fn smbclient_path() -> Result<std::path::PathBuf, SmbError> {
    which::which("smbclient").map_err(|_| SmbError::BinaryNotFound)
}

fn build_auth_args(cfg: &SmbConfig) -> Vec<String> {
    let mut args = vec![];
    if let Some(u) = &cfg.username {
        args.extend_from_slice(&["-U".to_string(), u.clone()]);
    }
    if let Some(d) = &cfg.domain {
        args.extend_from_slice(&["-W".to_string(), d.clone()]);
    }
    if cfg.port != 445 {
        args.extend_from_slice(&["-p".to_string(), cfg.port.to_string()]);
    }
    args
}

fn share_path(cfg: &SmbConfig) -> String {
    format!("//{}/{}", cfg.host, cfg.share)
}

fn parse_ls_output(output: &str) -> Vec<SmbEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Domain=") { continue; }
        // smbclient ls format: "  name               D        0  Mon Jan  1 00:00:00 2020"
        let mut parts = line.splitn(2, ' ');
        let name = parts.next().unwrap_or("").trim().to_string();
        if name.is_empty() || name == "." || name == ".." { continue; }
        let rest = parts.next().unwrap_or("").trim();
        let is_dir = rest.starts_with('D');
        let size_str = rest.chars().skip(1).collect::<String>().split_whitespace().next()
            .unwrap_or("0").to_string();
        let size = size_str.parse().unwrap_or(0);
        entries.push(SmbEntry {
            name,
            entry_type: if is_dir { SmbEntryType::Directory } else { SmbEntryType::File },
            size,
            modified: None,
        });
    }
    entries
}

#[tauri::command]
pub async fn smb_connect(
    config: SmbConfig,
    state: tauri::State<'_, SmbState>,
) -> Result<String, SmbError> {
    let smbclient = smbclient_path()?;
    let share = share_path(&config);
    let mut args = build_auth_args(&config);
    args.extend_from_slice(&[share, "-c".to_string(), "quit".to_string()]);

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &config.password {
        cmd.env("PASSWD", pw);
    }

    let output = cmd.output().await?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("NT_STATUS_LOGON_FAILURE") {
        return Err(SmbError::AuthFailed);
    }
    if !output.status.success() && !stderr.contains("NT_STATUS_OK") {
        return Err(SmbError::CommandError(stderr.to_string()));
    }

    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), config);
    Ok(id)
}

#[tauri::command]
pub async fn smb_list_dir(
    id: String,
    path: String,
    state: tauri::State<'_, SmbState>,
) -> Result<Vec<SmbEntry>, SmbError> {
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SmbError::NotFound(id.clone()))?;

    let smbclient = smbclient_path()?;
    let share = share_path(&cfg);
    let ls_cmd = if path.is_empty() || path == "/" {
        "ls".to_string()
    } else {
        format!("cd \"{path}\"; ls")
    };

    let mut args = build_auth_args(&cfg);
    args.extend_from_slice(&[share, "-c".to_string(), ls_cmd]);

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &cfg.password { cmd.env("PASSWD", pw); }

    let output = cmd.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(parse_ls_output(&stdout))
}

#[tauri::command]
pub async fn smb_list_shares(
    host: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<Vec<String>, SmbError> {
    let smbclient = smbclient_path()?;
    let mut args = vec!["-L".to_string(), host.clone(), "-N".to_string()];
    if let Some(u) = &username {
        args.extend_from_slice(&["-U".to_string(), u.clone()]);
    }

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &password { cmd.env("PASSWD", pw); }

    let output = cmd.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let shares: Vec<String> = stdout.lines()
        .filter(|l| l.contains("Disk") || l.contains("IPC"))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    Ok(shares)
}

#[tauri::command]
pub fn smb_disconnect(id: String, state: tauri::State<'_, SmbState>) -> Result<(), SmbError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| SmbError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn smb_list_sessions(state: tauri::State<'_, SmbState>) -> Vec<SmbSession> {
    state.sessions.lock().unwrap().iter().map(|(id, cfg)| SmbSession {
        id: id.clone(), host: cfg.host.clone(), share: cfg.share.clone(),
    }).collect()
}

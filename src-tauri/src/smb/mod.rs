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
        // Skips blank lines, the "Domain=..." banner, and smbclient's
        // trailing disk-usage summary (e.g. "9994224 blocks of size 1024.
        // 5000000 blocks available") — without this last check that line
        // parsed as a bogus zero-byte file entry named after the block count.
        if line.is_empty() || line.starts_with("Domain=") || line.contains("blocks of size") { continue; }
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

/// Errors smbclient reports as text on stderr rather than a nonzero exit
/// code — its own docs and behavior are inconsistent about which is used,
/// so both the exit status and this substring check are needed together.
fn check_smbclient_stderr(stderr: &str) -> Result<(), SmbError> {
    if stderr.contains("NT_STATUS_LOGON_FAILURE") {
        return Err(SmbError::AuthFailed);
    }
    if stderr.contains("NT_STATUS") && !stderr.contains("NT_STATUS_OK") {
        return Err(SmbError::CommandError(stderr.trim().to_string()));
    }
    Ok(())
}

/// Downloads `remote_path` (relative to the connected share) to `local_path`
/// on this machine. smbclient does the file I/O itself — the frontend picks
/// `local_path` via a save dialog and passes it straight through, so large
/// files don't get shuttled through the IPC boundary as a byte array.
#[tauri::command]
pub async fn smb_get(
    id: String,
    remote_path: String,
    local_path: String,
    state: tauri::State<'_, SmbState>,
) -> Result<(), SmbError> {
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SmbError::NotFound(id.clone()))?;

    let smbclient = smbclient_path()?;
    let share = share_path(&cfg);
    let get_cmd = format!("get \"{remote_path}\" \"{local_path}\"");
    let mut args = build_auth_args(&cfg);
    args.extend_from_slice(&[share, "-c".to_string(), get_cmd]);

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &cfg.password { cmd.env("PASSWD", pw); }

    let output = cmd.output().await?;
    check_smbclient_stderr(&String::from_utf8_lossy(&output.stderr))?;
    if !tokio::fs::try_exists(&local_path).await.unwrap_or(false) {
        return Err(SmbError::PathNotFound(remote_path));
    }
    Ok(())
}

/// Uploads `local_path` on this machine to `remote_path` on the connected
/// share.
#[tauri::command]
pub async fn smb_put(
    id: String,
    local_path: String,
    remote_path: String,
    state: tauri::State<'_, SmbState>,
) -> Result<(), SmbError> {
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SmbError::NotFound(id.clone()))?;
    if !tokio::fs::try_exists(&local_path).await.unwrap_or(false) {
        return Err(SmbError::PathNotFound(local_path));
    }

    let smbclient = smbclient_path()?;
    let share = share_path(&cfg);
    let put_cmd = format!("put \"{local_path}\" \"{remote_path}\"");
    let mut args = build_auth_args(&cfg);
    args.extend_from_slice(&[share, "-c".to_string(), put_cmd]);

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &cfg.password { cmd.env("PASSWD", pw); }

    let output = cmd.output().await?;
    check_smbclient_stderr(&String::from_utf8_lossy(&output.stderr))
}

#[tauri::command]
pub async fn smb_delete(
    id: String,
    path: String,
    state: tauri::State<'_, SmbState>,
) -> Result<(), SmbError> {
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SmbError::NotFound(id.clone()))?;

    let smbclient = smbclient_path()?;
    let share = share_path(&cfg);
    let del_cmd = format!("del \"{path}\"");
    let mut args = build_auth_args(&cfg);
    args.extend_from_slice(&[share, "-c".to_string(), del_cmd]);

    let mut cmd = Command::new(&smbclient);
    cmd.args(&args);
    if let Some(pw) = &cfg.password { cmd.env("PASSWD", pw); }

    let output = cmd.output().await?;
    check_smbclient_stderr(&String::from_utf8_lossy(&output.stderr))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ls_output() {
        let output = "\
  .                                   D        0  Mon Jan  1 00:00:00 2024
  ..                                  D        0  Mon Jan  1 00:00:00 2024
  Documents                           D        0  Tue Feb  2 10:00:00 2024
  notes.txt                           A      128  Wed Mar  3 11:00:00 2024

\t\t9994224 blocks of size 1024. 5000000 blocks available";
        let entries = parse_ls_output(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Documents");
        assert_eq!(entries[0].entry_type, SmbEntryType::Directory);
        assert_eq!(entries[1].name, "notes.txt");
        assert_eq!(entries[1].entry_type, SmbEntryType::File);
        assert_eq!(entries[1].size, 128);
    }

    #[test]
    fn test_check_smbclient_stderr_ok_on_empty_or_status_ok() {
        assert!(check_smbclient_stderr("").is_ok());
        assert!(check_smbclient_stderr("NT_STATUS_OK").is_ok());
    }

    #[test]
    fn test_check_smbclient_stderr_auth_failure() {
        let err = check_smbclient_stderr("session setup failed: NT_STATUS_LOGON_FAILURE").unwrap_err();
        assert!(matches!(err, SmbError::AuthFailed));
    }

    #[test]
    fn test_check_smbclient_stderr_generic_failure() {
        let err = check_smbclient_stderr("NT_STATUS_ACCESS_DENIED opening remote file").unwrap_err();
        assert!(matches!(err, SmbError::CommandError(_)));
    }

    #[test]
    fn test_build_auth_args() {
        let cfg = SmbConfig {
            host: "192.168.0.30".to_string(),
            port: 445,
            username: Some("alal".to_string()),
            password: Some("hunter2".to_string()),
            domain: Some("WORKGROUP".to_string()),
            share: "public".to_string(),
        };
        let args = build_auth_args(&cfg);
        assert_eq!(args, vec!["-U", "alal", "-W", "WORKGROUP"]);
        assert_eq!(share_path(&cfg), "//192.168.0.30/public");
    }

    #[test]
    fn test_build_auth_args_includes_nonstandard_port() {
        let cfg = SmbConfig {
            host: "h".to_string(),
            port: 1445,
            username: None,
            password: None,
            domain: None,
            share: "s".to_string(),
        };
        let args = build_auth_args(&cfg);
        assert_eq!(args, vec!["-p", "1445"]);
    }
}

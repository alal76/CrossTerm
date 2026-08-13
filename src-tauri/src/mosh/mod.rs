use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MoshError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Mosh binary not found — install mosh on the client")]
    BinaryNotFound,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for MoshError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub identity_file: Option<String>,
    /// Optional UDP port range override (e.g. "60001:60010")
    pub udp_port_range: Option<String>,
    /// Extra SSH options forwarded to the mosh-server negotiation
    pub ssh_options: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoshConnectionInfo {
    pub id: String,
    pub host: String,
    pub username: String,
    pub pid: u32,
}

struct MoshProcess {
    info: MoshConnectionInfo,
    child: tokio::process::Child,
}

pub struct MoshState {
    processes: Mutex<HashMap<String, MoshProcess>>,
}

impl MoshState {
    pub fn new() -> Self {
        Self { processes: Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn mosh_connect(
    config: MoshConfig,
    state: tauri::State<'_, MoshState>,
) -> Result<String, MoshError> {
    let mosh_bin = which::which("mosh").map_err(|_| MoshError::BinaryNotFound)?;
    let id = Uuid::new_v4().to_string();

    let mut cmd = tokio::process::Command::new(mosh_bin);
    cmd.arg("--ssh").arg(format!(
        "ssh -p {}{}",
        config.port,
        config.identity_file.as_deref()
            .map(|f| format!(" -i {f}"))
            .unwrap_or_default()
    ));
    if let Some(range) = &config.udp_port_range {
        cmd.arg("--port").arg(range);
    }
    cmd.arg(format!("{}@{}", config.username, config.host));

    let child = cmd.spawn()?;
    let pid = child.id().unwrap_or(0);

    let info = MoshConnectionInfo {
        id: id.clone(),
        host: config.host.clone(),
        username: config.username.clone(),
        pid,
    };

    state.processes.lock().unwrap().insert(id.clone(), MoshProcess { info, child });
    Ok(id)
}

#[tauri::command]
pub async fn mosh_disconnect(
    id: String,
    state: tauri::State<'_, MoshState>,
) -> Result<(), MoshError> {
    let mut map = state.processes.lock().unwrap();
    let proc = map.remove(&id).ok_or_else(|| MoshError::NotFound(id.clone()))?;
    drop(proc.child); // SIGCHLD cleanup
    Ok(())
}

#[tauri::command]
pub fn mosh_list(
    state: tauri::State<'_, MoshState>,
) -> Vec<MoshConnectionInfo> {
    state.processes.lock().unwrap().values().map(|p| p.info.clone()).collect()
}

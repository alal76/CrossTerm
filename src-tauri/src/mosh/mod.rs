/// Mosh (mobile shell) client — shells out to the system `mosh` binary,
/// which itself SSHes in to start `mosh-server` and then speaks the UDP
/// SSP protocol directly to the far end.
///
/// mosh-client is an interactive terminal client: it puts the local TTY
/// into raw mode, does its own local echo/prediction, and reacts to
/// SIGWINCH — none of which works if its stdio is a plain pipe. It has to
/// be attached to a real PTY, exactly like the local-shell terminals in
/// `crate::terminal`, so this mirrors that module's reader-thread pattern
/// instead of the bare `Child` + no-stdio-at-all approach this used to take.
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MoshError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Mosh binary not found — install mosh on the client")]
    BinaryNotFound,
    #[error("PTY error: {0}")]
    Pty(String),
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
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoshConnectionInfo {
    pub id: String,
    pub host: String,
    pub username: String,
}

#[derive(Clone, Serialize)]
struct MoshOutputEvent {
    id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct MoshExitEvent {
    id: String,
}

struct MoshSession {
    info: MoshConnectionInfo,
    master_write: Arc<Mutex<Box<dyn Write + Send>>>,
    master_pty: Box<dyn MasterPty + Send>,
    shutdown: Arc<AtomicBool>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
}

pub struct MoshState {
    sessions: Mutex<HashMap<String, MoshSession>>,
}

impl MoshState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

fn build_ssh_arg(config: &MoshConfig) -> String {
    let mut ssh = format!("ssh -p {}", config.port);
    if let Some(identity) = &config.identity_file {
        ssh.push_str(&format!(" -i {identity}"));
    }
    if let Some(extra) = &config.ssh_options {
        ssh.push(' ');
        ssh.push_str(extra);
    }
    ssh
}

#[tauri::command]
pub fn mosh_connect(
    app_handle: AppHandle,
    config: MoshConfig,
    state: tauri::State<'_, MoshState>,
) -> Result<String, MoshError> {
    let mosh_bin = which::which("mosh").map_err(|_| MoshError::BinaryNotFound)?;
    let cols = config.cols.unwrap_or(80);
    let rows = config.rows.unwrap_or(24);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| MoshError::Pty(e.to_string()))?;

    let mut cmd = CommandBuilder::new(mosh_bin);
    cmd.arg("--ssh");
    cmd.arg(build_ssh_arg(&config));
    if let Some(range) = &config.udp_port_range {
        cmd.arg("--port");
        cmd.arg(range);
    }
    cmd.arg(format!("{}@{}", config.username, config.host));

    pair.slave
        .spawn_command(cmd)
        .map_err(|e| MoshError::Pty(e.to_string()))?;
    drop(pair.slave);

    let id = Uuid::new_v4().to_string();
    let info = MoshConnectionInfo {
        id: id.clone(),
        host: config.host.clone(),
        username: config.username.clone(),
    };

    let master_write: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(
        pair.master.take_writer().map_err(|e| MoshError::Pty(e.to_string()))?,
    ));

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| MoshError::Pty(e.to_string()))?;
    let event_id = id.clone();
    let handle = app_handle.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    let reader_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = handle.emit("mosh:exit", MoshExitEvent { id: event_id.clone() });
                    break;
                }
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = handle.emit("mosh:output", MoshOutputEvent { id: event_id.clone(), data: text });
                }
                Err(_) => {
                    if shutdown_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = handle.emit("mosh:exit", MoshExitEvent { id: event_id.clone() });
                    break;
                }
            }
        }
    });

    let session = MoshSession {
        info,
        master_write,
        master_pty: pair.master,
        shutdown,
        reader_handle: Some(reader_handle),
    };

    state.sessions.lock().unwrap().insert(id.clone(), session);
    Ok(id)
}

#[tauri::command]
pub fn mosh_write(
    id: String,
    data: String,
    state: tauri::State<'_, MoshState>,
) -> Result<(), MoshError> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&id).ok_or_else(|| MoshError::NotFound(id.clone()))?;
    let mut writer = session.master_write.lock().unwrap();
    writer.write_all(data.as_bytes())?;
    writer.flush()?;
    Ok(())
}

#[tauri::command]
pub fn mosh_resize(
    id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, MoshState>,
) -> Result<(), MoshError> {
    let sessions = state.sessions.lock().unwrap();
    let session = sessions.get(&id).ok_or_else(|| MoshError::NotFound(id.clone()))?;
    session
        .master_pty
        .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| MoshError::Pty(e.to_string()))
}

#[tauri::command]
pub fn mosh_disconnect(
    id: String,
    state: tauri::State<'_, MoshState>,
) -> Result<(), MoshError> {
    let mut sessions = state.sessions.lock().unwrap();
    let mut session = sessions.remove(&id).ok_or_else(|| MoshError::NotFound(id.clone()))?;
    session.shutdown.store(true, Ordering::Relaxed);
    drop(session.master_write);
    drop(session.master_pty);
    if let Some(handle) = session.reader_handle.take() {
        let _ = handle.join();
    }
    Ok(())
}

#[tauri::command]
pub fn mosh_list(state: tauri::State<'_, MoshState>) -> Vec<MoshConnectionInfo> {
    state.sessions.lock().unwrap().values().map(|s| s.info.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ssh_arg_default_port_only() {
        let config = MoshConfig {
            host: "192.168.0.20".into(), port: 22, username: "alal".into(),
            identity_file: None, udp_port_range: None, ssh_options: None,
            cols: None, rows: None,
        };
        assert_eq!(build_ssh_arg(&config), "ssh -p 22");
    }

    #[test]
    fn test_build_ssh_arg_includes_identity_and_extra_options() {
        let config = MoshConfig {
            host: "192.168.0.20".into(), port: 2222, username: "alal".into(),
            identity_file: Some("/home/alal/.ssh/id_ed25519".into()),
            udp_port_range: None,
            ssh_options: Some("-o StrictHostKeyChecking=no".into()),
            cols: None, rows: None,
        };
        assert_eq!(
            build_ssh_arg(&config),
            "ssh -p 2222 -i /home/alal/.ssh/id_ed25519 -o StrictHostKeyChecking=no"
        );
    }
}

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
    do_mosh_write(id, data, state.inner())
}

fn do_mosh_write(id: String, data: String, state: &MoshState) -> Result<(), MoshError> {
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
    do_mosh_resize(id, cols, rows, state.inner())
}

fn do_mosh_resize(id: String, cols: u16, rows: u16, state: &MoshState) -> Result<(), MoshError> {
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
    do_mosh_disconnect(id, state.inner())
}

fn do_mosh_disconnect(id: String, state: &MoshState) -> Result<(), MoshError> {
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
    do_mosh_list(state.inner())
}

fn do_mosh_list(state: &MoshState) -> Vec<MoshConnectionInfo> {
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

    #[test]
    fn test_build_ssh_arg_with_port_range_and_identity_only() {
        let config = MoshConfig {
            host: "h".into(), port: 22, username: "u".into(),
            identity_file: Some("/key".into()), udp_port_range: Some("60000:60010".into()),
            ssh_options: None, cols: Some(120), rows: Some(40),
        };
        // udp_port_range doesn't affect build_ssh_arg (handled separately in mosh_connect)
        assert_eq!(build_ssh_arg(&config), "ssh -p 22 -i /key");
    }

    #[test]
    fn test_mosh_error_display_and_serialize() {
        assert_eq!(
            MoshError::NotFound("x".into()).to_string(),
            "Connection not found: x"
        );
        assert_eq!(
            MoshError::BinaryNotFound.to_string(),
            "Mosh binary not found — install mosh on the client"
        );
        assert_eq!(MoshError::Pty("boom".into()).to_string(), "PTY error: boom");

        let json = serde_json::to_string(&MoshError::NotFound("abc".into())).unwrap();
        assert_eq!(json, "\"Connection not found: abc\"");
    }

    #[test]
    fn test_mosh_config_serde_roundtrip() {
        let config = MoshConfig {
            host: "10.0.0.1".into(), port: 22, username: "alal".into(),
            identity_file: None, udp_port_range: Some("60000:60010".into()),
            ssh_options: None, cols: Some(100), rows: Some(30),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: MoshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.host, "10.0.0.1");
        assert_eq!(parsed.udp_port_range.as_deref(), Some("60000:60010"));
        assert_eq!(parsed.cols, Some(100));
    }

    /// Builds a real `MoshSession` backed by an actual local PTY running a
    /// plain shell (not the `mosh` binary — that's exercised only in
    /// `mosh_connect`, which additionally requires the binary to be
    /// installed and a live SSH target). This lets `mosh_write`,
    /// `mosh_resize`, `mosh_disconnect`, and `mosh_list`'s session-management
    /// logic be exercised for real, mirroring `terminal::tests::create_test_session`.
    fn create_test_mosh_session(state: &MoshState) -> MoshConnectionInfo {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .expect("openpty");

        let shell = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
        let cmd = CommandBuilder::new(shell);
        pair.slave.spawn_command(cmd).expect("spawn shell");
        drop(pair.slave);

        let id = Uuid::new_v4().to_string();
        let info = MoshConnectionInfo {
            id: id.clone(),
            host: "test-host".into(),
            username: "test-user".into(),
        };

        let master_write: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().expect("writer")));
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        let session = MoshSession {
            info: info.clone(),
            master_write,
            master_pty: pair.master,
            shutdown,
            reader_handle: Some(reader_handle),
        };
        state.sessions.lock().unwrap().insert(id, session);
        info
    }

    #[test]
    fn test_mosh_write_and_list_and_disconnect() {
        let state = MoshState::new();
        let info = create_test_mosh_session(&state);

        assert_eq!(do_mosh_list(&state).len(), 1);
        assert_eq!(do_mosh_list(&state)[0].id, info.id);

        do_mosh_write(info.id.clone(), "echo hi\n".into(), &state).expect("write ok");

        do_mosh_resize(info.id.clone(), 100, 40, &state).expect("resize ok");

        do_mosh_disconnect(info.id.clone(), &state).expect("disconnect ok");
        assert!(do_mosh_list(&state).is_empty());

        // Operating on a disconnected/unknown session should error.
        let err = do_mosh_write(info.id.clone(), "x".into(), &state).unwrap_err();
        assert!(matches!(err, MoshError::NotFound(_)));
        let err = do_mosh_resize(info.id.clone(), 10, 10, &state).unwrap_err();
        assert!(matches!(err, MoshError::NotFound(_)));
        let err = do_mosh_disconnect(info.id, &state).unwrap_err();
        assert!(matches!(err, MoshError::NotFound(_)));
    }

    #[test]
    fn test_mosh_write_unknown_session() {
        let state = MoshState::new();
        let err = do_mosh_write("nope".into(), "x".into(), &state).unwrap_err();
        assert!(matches!(err, MoshError::NotFound(_)));
    }

    #[test]
    fn test_mosh_list_empty_by_default() {
        let state = MoshState::new();
        assert!(do_mosh_list(&state).is_empty());
    }
}

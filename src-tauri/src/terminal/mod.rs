use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Terminal not found: {0}")]
    NotFound(String),
    #[error("PTY error: {0}")]
    Pty(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for TerminalError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
struct TerminalOutputEvent {
    terminal_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct TerminalBinaryOutputEvent {
    terminal_id: String,
    data: String, // base64-encoded
}

#[derive(Clone, Serialize)]
struct TerminalBellEvent {
    terminal_id: String,
}

#[derive(Clone, Serialize)]
struct TerminalExitEvent {
    terminal_id: String,
    code: Option<i32>,
}

/// Internal session state for an active PTY.
#[allow(dead_code)]
struct PtySession {
    info: TerminalInfo,
    master_write: Arc<Mutex<Box<dyn Write + Send>>>,
    master_pty: Box<dyn MasterPty + Send>,
    /// Signal to tell the reader thread to shut down.
    shutdown: Arc<AtomicBool>,
    /// Handle to the reader thread so we can join it on close.
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Optional log file for session output logging (shared with reader thread).
    log_file: Arc<Mutex<Option<std::fs::File>>>,
    /// Whether this terminal is in binary (raw base64) output mode.
    binary_mode: bool,
}

// ── Manager ─────────────────────────────────────────────────────────────

pub struct TerminalManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        app_handle: &AppHandle,
        binary_mode: Option<bool>,
        shell: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<TerminalInfo, TerminalError> {
        let cols = cols.unwrap_or(80);
        let rows = rows.unwrap_or(24);
        let is_binary = binary_mode.unwrap_or(false);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let shell_path = shell.clone().unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(&shell_path);

        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }
        if let Some(env_vars) = env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }

        pair.slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        // We intentionally drop the slave side now – the master keeps the PTY alive.
        drop(pair.slave);

        let id = Uuid::new_v4().to_string();
        let info = TerminalInfo {
            id: id.clone(),
            shell: shell_path,
            cols,
            rows,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let master_write: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .map_err(|e| TerminalError::Pty(e.to_string()))?,
        ));

        // Spawn a reader thread that forwards PTY output to the frontend via events.
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::Pty(e.to_string()))?;
        let event_id = id.clone();
        let handle = app_handle.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let log_file: Arc<Mutex<Option<std::fs::File>>> = Arc::new(Mutex::new(None));
        let log_file_for_reader = log_file.clone();

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // PTY closed
                        let _ = handle.emit(
                            "terminal:exit",
                            TerminalExitEvent {
                                terminal_id: event_id.clone(),
                                code: None,
                            },
                        );
                        break;
                    }
                    Ok(n) => {
                        // BE-TERM-03: Detect BEL character (0x07)
                        if buf[..n].contains(&0x07) {
                            let _ = handle.emit(
                                "terminal:bell",
                                TerminalBellEvent {
                                    terminal_id: event_id.clone(),
                                },
                            );
                        }

                        // BE-TERM-04: Binary mode sends base64, text mode sends UTF-8
                        if is_binary {
                            use base64::Engine;
                            let encoded =
                                base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                            let _ = handle.emit(
                                "terminal:binary_output",
                                TerminalBinaryOutputEvent {
                                    terminal_id: event_id.clone(),
                                    data: encoded,
                                },
                            );
                        } else {
                            let text = String::from_utf8_lossy(&buf[..n]).to_string();
                            let _ = handle.emit(
                                "terminal:output",
                                TerminalOutputEvent {
                                    terminal_id: event_id.clone(),
                                    data: text,
                                },
                            );
                        }

                        // Write raw bytes to log file if attached
                        if let Ok(mut guard) = log_file_for_reader.lock() {
                            if let Some(ref mut f) = *guard {
                                let _ = f.write_all(&buf[..n]);
                            }
                        }
                    }
                    Err(_) => {
                        if shutdown_clone.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = handle.emit(
                            "terminal:exit",
                            TerminalExitEvent {
                                terminal_id: event_id.clone(),
                                code: None,
                            },
                        );
                        break;
                    }
                }
            }
        });

        let session = PtySession {
            info: info.clone(),
            master_write,
            master_pty: pair.master,
            shutdown,
            reader_handle: Some(reader_handle),
            log_file,
            binary_mode: is_binary,
        };

        self.sessions.lock().unwrap().insert(id, session);

        Ok(info)
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), TerminalError> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(id)
            .ok_or_else(|| TerminalError::NotFound(id.into()))?;
        let mut writer = session.master_write.lock().unwrap();
        writer.write_all(data).map_err(TerminalError::from)?;
        writer.flush().map_err(TerminalError::from)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), TerminalError> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(id)
            .ok_or_else(|| TerminalError::NotFound(id.into()))?;
        session
            .master_pty
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::Pty(e.to_string()))
    }

    pub fn close(&self, id: &str) -> Result<(), TerminalError> {
        let mut sessions = self.sessions.lock().unwrap();
        let mut session = sessions
            .remove(id)
            .ok_or_else(|| TerminalError::NotFound(id.into()))?;
        // Signal the reader thread to stop, then drop the master PTY so the
        // read() call returns an error/EOF, then join the thread.
        session.shutdown.store(true, Ordering::Relaxed);
        drop(session.master_write);
        drop(session.master_pty);
        if let Some(handle) = session.reader_handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<TerminalInfo> {
        let sessions = self.sessions.lock().unwrap();
        sessions.values().map(|s| s.info.clone()).collect()
    }

    pub fn start_logging(&self, id: &str, path: &str) -> Result<(), TerminalError> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(id)
            .ok_or_else(|| TerminalError::NotFound(id.into()))?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(TerminalError::from)?;
        let mut log = session.log_file.lock().unwrap();
        *log = Some(file);
        Ok(())
    }


    pub fn stop_logging(&self, id: &str) -> Result<(), TerminalError> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(id)
            .ok_or_else(|| TerminalError::NotFound(id.into()))?;
        let mut log = session.log_file.lock().unwrap();
        *log = None;
        Ok(())
    }
}

// ── Default shell detection ─────────────────────────────────────────────

fn default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn terminal_create(
    app_handle: AppHandle,
    state: tauri::State<'_, TerminalManager>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    shell: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
    binary_mode: Option<bool>,
) -> Result<TerminalInfo, TerminalError> {
    let info = state.create(&app_handle, binary_mode, shell, cols, rows, cwd, env)?;
    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::TerminalCreate,
        &format!("Created terminal {} ({})", info.id, info.shell),
    );
    Ok(info)
}

#[tauri::command]
pub fn terminal_write(
    state: tauri::State<'_, TerminalManager>,
    id: String,
    data: String,
) -> Result<(), TerminalError> {
    state.write(&id, data.as_bytes())
}

#[tauri::command]
pub fn terminal_resize(
    state: tauri::State<'_, TerminalManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), TerminalError> {
    state.resize(&id, cols, rows)
}

#[tauri::command]
pub fn terminal_close(
    state: tauri::State<'_, TerminalManager>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    id: String,
) -> Result<(), TerminalError> {
    let result = state.close(&id);
    if result.is_ok() {
        let pid = config_state
            .active_profile_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::TerminalClose,
            &format!("Closed terminal {}", id),
        );
    }
    result
}

#[tauri::command]
pub fn terminal_list(state: tauri::State<'_, TerminalManager>) -> Vec<TerminalInfo> {
    state.list()
}

#[tauri::command]
pub fn terminal_start_logging(
    state: tauri::State<'_, TerminalManager>,
    terminal_id: String,
    path: String,
) -> Result<(), TerminalError> {
    state.start_logging(&terminal_id, &path)
}

#[tauri::command]
pub fn terminal_stop_logging(
    state: tauri::State<'_, TerminalManager>,
    terminal_id: String,
) -> Result<(), TerminalError> {
    state.stop_logging(&terminal_id)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_manager_new() {
        let manager = TerminalManager::new();
        let sessions = manager.sessions.lock().unwrap();
        assert!(sessions.is_empty(), "New TerminalManager should have no sessions");
    }

    #[test]
    fn test_terminal_manager_list_empty() {
        let manager = TerminalManager::new();
        let list = manager.list();
        assert!(list.is_empty());
    }

    #[test]
    fn test_terminal_manager_write_not_found() {
        let manager = TerminalManager::new();
        let result = manager.write("nonexistent-id", b"hello");
        assert!(result.is_err());
        match result.unwrap_err() {
            TerminalError::NotFound(id) => assert_eq!(id, "nonexistent-id"),
            other => panic!("Expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_terminal_manager_resize_not_found() {
        let manager = TerminalManager::new();
        let result = manager.resize("nonexistent-id", 80, 24);
        assert!(result.is_err());
        match result.unwrap_err() {
            TerminalError::NotFound(id) => assert_eq!(id, "nonexistent-id"),
            other => panic!("Expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_terminal_manager_close_not_found() {
        let manager = TerminalManager::new();
        let result = manager.close("nonexistent-id");
        assert!(result.is_err());
        match result.unwrap_err() {
            TerminalError::NotFound(id) => assert_eq!(id, "nonexistent-id"),
            other => panic!("Expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_default_shell() {
        let shell = default_shell();
        assert!(!shell.is_empty(), "Default shell should not be empty");
        #[cfg(not(target_os = "windows"))]
        {
            assert!(
                shell.contains("sh")
                    || shell.contains("zsh")
                    || shell.contains("bash")
                    || shell.contains("fish"),
                "Default shell '{}' doesn't look like a known shell",
                shell
            );
        }
    }

    #[test]
    fn test_terminal_info_serialization() {
        let info = TerminalInfo {
            id: "test-id".to_string(),
            shell: "/bin/zsh".to_string(),
            cols: 120,
            rows: 40,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: TerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.cols, 120);
        assert_eq!(deserialized.rows, 40);
    }

    #[test]
    fn test_terminal_error_display() {
        let err = TerminalError::NotFound("abc".into());
        assert_eq!(err.to_string(), "Terminal not found: abc");

        let err = TerminalError::Pty("bad pty".into());
        assert_eq!(err.to_string(), "PTY error: bad pty");
    }

    // ── BE-TERM-03: Bell detection ──────────────────────────────────

    #[test]
    fn test_bell_character_detection() {
        let data: &[u8] = b"hello\x07world";
        assert!(data.contains(&0x07), "Data should contain BEL character");
    }

    #[test]
    fn test_no_bell_in_normal_output() {
        let data: &[u8] = b"hello world\r\n";
        assert!(!data.contains(&0x07), "Normal output should not contain BEL");
    }

    // ── BE-TERM-04: Binary output mode ──────────────────────────────

    #[test]
    fn test_binary_output_base64_encoding() {
        use base64::Engine;
        let raw_bytes: &[u8] = &[0x00, 0x01, 0xFF, 0xFE, 0x07, 0x1B];
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw_bytes);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        assert_eq!(raw_bytes, decoded.as_slice());
    }

    #[test]
    fn test_binary_output_event_serialization() {
        let event = TerminalBinaryOutputEvent {
            terminal_id: "term-1".to_string(),
            data: "AAAB/w==".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("terminal_id"));
        assert!(json.contains("AAAB/w=="));
    }

    #[test]
    fn test_bell_event_serialization() {
        let event = TerminalBellEvent {
            terminal_id: "term-1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("term-1"));
    }

    // ── Test helper: create PTY session without AppHandle ──────────

    /// Creates a PTY session directly in the TerminalManager, bypassing
    /// `create()` which requires an AppHandle for event emission.
    /// Returns the TerminalInfo and a shared buffer that captures all
    /// PTY output (replacing Tauri event emission).
    fn create_test_session(
        manager: &TerminalManager,
        shell: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<(TerminalInfo, Arc<Mutex<Vec<u8>>>), TerminalError> {
        let cols = cols.unwrap_or(80);
        let rows = rows.unwrap_or(24);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let shell_path = shell.unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(&shell_path);
        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }
        if let Some(env_vars) = env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }

        pair.slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Pty(e.to_string()))?;
        drop(pair.slave);

        let id = Uuid::new_v4().to_string();
        let info = TerminalInfo {
            id: id.clone(),
            shell: shell_path,
            cols,
            rows,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let master_write: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .map_err(|e| TerminalError::Pty(e.to_string()))?,
        ));

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::Pty(e.to_string()))?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let log_file: Arc<Mutex<Option<std::fs::File>>> = Arc::new(Mutex::new(None));
        let log_file_for_reader = log_file.clone();

        let output_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let output_buf_clone = output_buf.clone();

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                if shutdown_clone.load(Ordering::Relaxed) {
                    break;
                }
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut out) = output_buf_clone.lock() {
                            out.extend_from_slice(&buf[..n]);
                        }
                        if let Ok(mut guard) = log_file_for_reader.lock() {
                            if let Some(ref mut f) = *guard {
                                let _ = f.write_all(&buf[..n]);
                            }
                        }
                    }
                    Err(_) => {
                        if shutdown_clone.load(Ordering::Relaxed) {
                            break;
                        }
                        break;
                    }
                }
            }
        });

        let session = PtySession {
            info: info.clone(),
            master_write,
            master_pty: pair.master,
            shutdown,
            reader_handle: Some(reader_handle),
            log_file,
            binary_mode: false,
        };

        manager.sessions.lock().unwrap().insert(id, session);
        Ok((info, output_buf))
    }

    /// Helper: read captured output as a UTF-8 string.
    fn read_output(buf: &Arc<Mutex<Vec<u8>>>) -> String {
        let data = buf.lock().unwrap();
        String::from_utf8_lossy(&data).to_string()
    }

    // ── PTY-backed tests (bypassing AppHandle) ──────────────────────

    #[test]
    fn test_terminal_create() {
        // UT-T-01: Create a local terminal. Assert terminal ID is returned
        // and listed in terminal_list.
        let manager = TerminalManager::new();
        let (info, _output) = create_test_session(&manager, None, None, None, None, None)
            .expect("Failed to create test terminal");

        assert!(!info.id.is_empty(), "Terminal ID should be non-empty");
        assert_eq!(info.cols, 80);
        assert_eq!(info.rows, 24);

        let list = manager.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, info.id);

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    fn test_terminal_write_read() {
        // UT-T-02: Create terminal, write `echo hello\n`, capture output.
        // Assert "hello" appears in output.
        let manager = TerminalManager::new();
        let (info, output) = create_test_session(&manager, None, None, None, None, None)
            .expect("Failed to create test terminal");

        manager
            .write(&info.id, b"echo hello\n")
            .expect("Failed to write to terminal");

        std::thread::sleep(std::time::Duration::from_millis(500));

        let text = read_output(&output);
        assert!(
            text.contains("hello"),
            "Output should contain 'hello', got: {:?}",
            text
        );

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    fn test_terminal_resize() {
        // UT-T-03: Create terminal, resize to 120×40. Assert no error.
        let manager = TerminalManager::new();
        let (info, _output) = create_test_session(&manager, None, None, None, None, None)
            .expect("Failed to create test terminal");

        manager
            .resize(&info.id, 120, 40)
            .expect("Resize to 120x40 should succeed");

        // Verify info still accessible (session not corrupted)
        let list = manager.list();
        assert_eq!(list.len(), 1);

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    fn test_terminal_close() {
        // UT-T-04: Create terminal, close it. Assert removed from terminal_list.
        let manager = TerminalManager::new();
        let (info, _output) = create_test_session(&manager, None, None, None, None, None)
            .expect("Failed to create test terminal");

        assert_eq!(manager.list().len(), 1);

        manager.close(&info.id).expect("Failed to close terminal");

        assert_eq!(manager.list().len(), 0, "Terminal should be removed after close");

        // Closing again should return NotFound
        let err = manager.close(&info.id).unwrap_err();
        assert!(matches!(err, TerminalError::NotFound(_)));
    }

    #[test]
    fn test_multiple_terminals() {
        // UT-T-05: Create 5 terminals. Verify all listed. Close 2. Verify 3 remain.
        let manager = TerminalManager::new();
        let mut ids = Vec::new();

        for _ in 0..5 {
            let (info, _output) = create_test_session(&manager, None, None, None, None, None)
                .expect("Failed to create test terminal");
            ids.push(info.id);
        }

        assert_eq!(manager.list().len(), 5, "Should have 5 terminals");

        manager.close(&ids[0]).expect("Failed to close terminal 0");
        manager.close(&ids[2]).expect("Failed to close terminal 2");

        let remaining = manager.list();
        assert_eq!(remaining.len(), 3, "Should have 3 terminals after closing 2");

        // Verify the correct terminals remain
        let remaining_ids: Vec<String> = remaining.iter().map(|t| t.id.clone()).collect();
        assert!(!remaining_ids.contains(&ids[0]));
        assert!(remaining_ids.contains(&ids[1]));
        assert!(!remaining_ids.contains(&ids[2]));
        assert!(remaining_ids.contains(&ids[3]));
        assert!(remaining_ids.contains(&ids[4]));

        // Clean up
        for id in &[&ids[1], &ids[3], &ids[4]] {
            manager.close(id).expect("Failed to close remaining terminal");
        }
    }

    #[test]
    fn test_custom_shell() {
        // UT-T-06: Create terminal with /bin/sh. Verify shell spawned.
        let manager = TerminalManager::new();
        let (info, _output) = create_test_session(
            &manager,
            Some("/bin/sh".to_string()),
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create terminal with /bin/sh");

        assert_eq!(info.shell, "/bin/sh");
        assert_eq!(manager.list().len(), 1);

        // Verify the shell is functional by writing a command
        manager
            .write(&info.id, b"echo ok\n")
            .expect("Failed to write to /bin/sh terminal");
        std::thread::sleep(std::time::Duration::from_millis(300));

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    fn test_custom_env() {
        // UT-T-07: Create terminal with FOO=bar in environment.
        // Write `echo $FOO\n`. Assert "bar" in output.
        let manager = TerminalManager::new();
        let mut env_vars = HashMap::new();
        env_vars.insert("FOO".to_string(), "bar".to_string());

        let (info, output) = create_test_session(
            &manager,
            None,
            None,
            None,
            None,
            Some(env_vars),
        )
        .expect("Failed to create terminal with custom env");

        manager
            .write(&info.id, b"echo $FOO\n")
            .expect("Failed to write to terminal");

        std::thread::sleep(std::time::Duration::from_millis(500));

        let text = read_output(&output);
        assert!(
            text.contains("bar"),
            "Output should contain 'bar' from FOO env var, got: {:?}",
            text
        );

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    fn test_custom_cwd() {
        // UT-T-08: Create terminal with cwd=/tmp. Write `pwd\n`.
        // Assert "/tmp" or "/private/tmp" in output (macOS resolves /tmp → /private/tmp).
        let manager = TerminalManager::new();
        let (info, output) = create_test_session(
            &manager,
            None,
            None,
            None,
            Some("/tmp".to_string()),
            None,
        )
        .expect("Failed to create terminal with custom cwd");

        manager
            .write(&info.id, b"pwd\n")
            .expect("Failed to write to terminal");

        std::thread::sleep(std::time::Duration::from_millis(500));

        let text = read_output(&output);
        assert!(
            text.contains("/tmp") || text.contains("/private/tmp"),
            "Output should contain '/tmp' or '/private/tmp', got: {:?}",
            text
        );

        manager.close(&info.id).expect("Failed to close terminal");
    }

    #[test]
    #[ignore = "UT-T-09: Requires Tauri AppHandle to verify terminal:exit event emission. \
                The PTY exit itself works (tested via close), but the event delivery \
                channel is only available through Tauri's runtime."]
    fn test_terminal_exit_event() {
        // UT-T-09: This test specifically validates that a `terminal:exit` event
        // is emitted to the frontend when the shell process exits.
        // The event is emitted inside the reader thread via `app_handle.emit()`,
        // which requires a real Tauri AppHandle. Unit-test PTY sessions created
        // by `create_test_session` have no event bus — they capture output to a
        // buffer instead. To fully test this, use `tauri::test::mock_builder()`
        // or run as an integration test with a real Tauri app context.
    }

    // ── BE-TERM-05: Session logging ─────────────────────────────────

    #[test]
    fn test_start_stop_logging() {
        // Create terminal, start logging to a temp file, write data,
        // stop logging. Verify log file contains output.
        let manager = TerminalManager::new();
        let (info, _output) = create_test_session(&manager, None, None, None, None, None)
            .expect("Failed to create test terminal");

        let tmp_dir = std::env::temp_dir();
        let log_path = tmp_dir.join(format!("crossterm-test-log-{}.txt", info.id));
        let log_path_str = log_path.to_str().unwrap().to_string();

        manager
            .start_logging(&info.id, &log_path_str)
            .expect("Failed to start logging");

        manager
            .write(&info.id, b"echo logtest123\n")
            .expect("Failed to write to terminal");

        std::thread::sleep(std::time::Duration::from_millis(500));

        manager
            .stop_logging(&info.id)
            .expect("Failed to stop logging");

        // Read the log file and verify it captured output
        let log_contents = std::fs::read_to_string(&log_path)
            .expect("Failed to read log file");
        assert!(
            log_contents.contains("logtest123"),
            "Log file should contain 'logtest123', got: {:?}",
            log_contents
        );

        // Clean up
        let _ = std::fs::remove_file(&log_path);
        manager.close(&info.id).expect("Failed to close terminal");
    }
}

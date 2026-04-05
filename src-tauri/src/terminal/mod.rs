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
struct TerminalExitEvent {
    terminal_id: String,
    code: Option<i32>,
}

/// Internal session state for an active PTY.
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

    pub fn create(
        &self,
        app_handle: &AppHandle,
        shell: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
    ) -> Result<TerminalInfo, TerminalError> {
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
                        // Best-effort UTF-8 – replace invalid sequences.
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = handle.emit(
                            "terminal:output",
                            TerminalOutputEvent {
                                terminal_id: event_id.clone(),
                                data: text,
                            },
                        );
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
pub fn terminal_create(
    app_handle: AppHandle,
    state: tauri::State<'_, TerminalManager>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    shell: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
) -> Result<TerminalInfo, TerminalError> {
    let info = state.create(&app_handle, shell, cols, rows, cwd, env)?;
    let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
    crate::audit::append_event(&pid, crate::audit::AuditEventType::TerminalCreate, &format!("Created terminal {} ({})", info.id, info.shell));
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
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::TerminalClose, &format!("Closed terminal {}", id));
    }
    result
}

#[tauri::command]
pub fn terminal_list(
    state: tauri::State<'_, TerminalManager>,
) -> Vec<TerminalInfo> {
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
            // On unix-like systems, should be a path or shell name
            assert!(
                shell.contains("sh") || shell.contains("zsh") || shell.contains("bash") || shell.contains("fish"),
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

    // ── Integration tests requiring AppHandle ───────────────────────────

    #[test]
    #[ignore = "Requires Tauri AppHandle for event emission — run as integration test"]
    fn test_create_and_list_terminal() {
        // Would test: create a terminal, verify list() returns it,
        // verify the returned TerminalInfo has correct shell/cols/rows.
    }

    #[test]
    #[ignore = "Requires Tauri AppHandle for event emission — run as integration test"]
    fn test_create_write_close_lifecycle() {
        // Would test: create terminal, write data to it, close it,
        // verify it's removed from the manager's session map.
    }

    #[test]
    #[ignore = "Requires Tauri AppHandle for event emission — run as integration test"]
    fn test_resize_active_terminal() {
        // Would test: create terminal, resize to 200x50, verify no error.
    }
}

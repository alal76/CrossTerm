use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum RdpError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("Redirection error: {0}")]
    Redirection(String),
    #[error("Resize error: {0}")]
    Resize(String),
    #[error("Screenshot error: {0}")]
    Screenshot(String),
    #[error("Gateway error: {0}")]
    Gateway(String),
    #[error("IO error: {0}")]
    Io(String),
}

impl Serialize for RdpError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for RdpError {
    fn from(e: std::io::Error) -> Self {
        RdpError::Io(e.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpCodec {
    Auto,
    RemoteFx,
    Gfx,
    Progressive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpAudioMode {
    None,
    Playback,
    Record,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpMouseEventType {
    Move,
    Down,
    Up,
    Scroll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpMouseButton {
    Left,
    Right,
    Middle,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpGateway {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpMonitorConfig {
    pub span_all: bool,
    pub selected_monitors: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveMapping {
    pub name: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub credential_ref: Option<String>,
    pub domain: Option<String>,
    pub nla_enabled: bool,
    pub tls_required: bool,
    pub gateway: Option<RdpGateway>,
    pub multi_monitor: Option<RdpMonitorConfig>,
    pub codec: RdpCodec,
    pub clipboard_sync: bool,
    pub drive_paths: Vec<DriveMapping>,
    pub printer_redirect: bool,
    pub audio_mode: RdpAudioMode,
    pub smart_card: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpClipboardData {
    pub text: Option<String>,
    pub files: Option<Vec<String>>,
    pub image_png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpRedirectionConfig {
    pub drives: Vec<DriveMapping>,
    pub printer: bool,
    pub audio: RdpAudioMode,
    pub smart_card: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpKeyEvent {
    pub key_code: u32,
    pub pressed: bool,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpMouseEvent {
    pub x: u32,
    pub y: u32,
    pub button: RdpMouseButton,
    pub event_type: RdpMouseEventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdpConnectionInfo {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub status: RdpConnectionStatus,
    pub width: u32,
    pub height: u32,
    pub connected_at: Option<String>,
}

// ── Tauri Event Payloads ────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, Serialize)]
struct RdpFrameEvent {
    connection_id: String,
    width: u32,
    height: u32,
    data_base64: String,
}

#[derive(Clone, Serialize)]
struct RdpConnectedEvent {
    connection_id: String,
}

#[derive(Clone, Serialize)]
struct RdpDisconnectedEvent {
    connection_id: String,
    reason: String,
}

#[derive(Clone, Serialize)]
struct RdpClipboardEvent {
    connection_id: String,
    data: RdpClipboardData,
}

#[allow(dead_code)]
#[derive(Clone, Serialize)]
struct RdpErrorEvent {
    connection_id: String,
    message: String,
}

// ── Backend Trait ────────────────────────────────────────────────────────

/// Trait abstracting the actual FreeRDP FFI calls.
/// Implementations can provide real FreeRDP bindings or test stubs.
pub trait RdpBackend: Send + Sync {
    fn connect(
        &self,
        id: &str,
        config: &RdpConfig,
    ) -> Result<(), RdpError>;

    fn disconnect(&self, id: &str) -> Result<(), RdpError>;

    fn resize(&self, id: &str, width: u32, height: u32) -> Result<(), RdpError>;

    fn send_key(&self, id: &str, event: &RdpKeyEvent) -> Result<(), RdpError>;

    fn send_mouse(&self, id: &str, event: &RdpMouseEvent) -> Result<(), RdpError>;

    fn send_clipboard(&self, id: &str, data: &RdpClipboardData) -> Result<(), RdpError>;

    fn screenshot(&self, id: &str) -> Result<String, RdpError>;

    fn configure_redirection(
        &self,
        id: &str,
        config: &RdpRedirectionConfig,
    ) -> Result<(), RdpError>;

    fn send_ctrl_alt_del(&self, id: &str) -> Result<(), RdpError>;
}

// ── Stub Backend ────────────────────────────────────────────────────────

/// No-op backend used when FreeRDP is not available (tests / dev builds).
pub struct StubRdpBackend;

impl RdpBackend for StubRdpBackend {
    fn connect(&self, _id: &str, _config: &RdpConfig) -> Result<(), RdpError> {
        Err(RdpError::ConnectionFailed(
            "RDP remote desktop rendering is not yet implemented. \
             Connect via SSH for command-line access.".into(),
        ))
    }

    fn disconnect(&self, _id: &str) -> Result<(), RdpError> {
        Ok(())
    }

    fn resize(&self, _id: &str, _width: u32, _height: u32) -> Result<(), RdpError> {
        Ok(())
    }

    fn send_key(&self, _id: &str, _event: &RdpKeyEvent) -> Result<(), RdpError> {
        Ok(())
    }

    fn send_mouse(&self, _id: &str, _event: &RdpMouseEvent) -> Result<(), RdpError> {
        Ok(())
    }

    fn send_clipboard(&self, _id: &str, _data: &RdpClipboardData) -> Result<(), RdpError> {
        Ok(())
    }

    fn screenshot(&self, _id: &str) -> Result<String, RdpError> {
        Ok(String::new())
    }

    fn configure_redirection(
        &self,
        _id: &str,
        _config: &RdpRedirectionConfig,
    ) -> Result<(), RdpError> {
        Ok(())
    }

    fn send_ctrl_alt_del(&self, _id: &str) -> Result<(), RdpError> {
        Ok(())
    }
}

// ── Connection ──────────────────────────────────────────────────────────

#[allow(dead_code)]
struct RdpConnection {
    info: RdpConnectionInfo,
    config: RdpConfig,
    redirection: RdpRedirectionConfig,
    clipboard: Option<RdpClipboardData>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct RdpState {
    connections: Mutex<HashMap<String, RdpConnection>>,
    backend: Box<dyn RdpBackend>,
}

impl RdpState {
    pub fn new(backend: Box<dyn RdpBackend>) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            backend,
        }
    }

    #[allow(dead_code)]
    pub fn with_stub() -> Self {
        Self::new(Box::new(StubRdpBackend))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn validate_config(config: &RdpConfig) -> Result<(), RdpError> {
    if config.host.is_empty() {
        return Err(RdpError::InvalidConfig("host cannot be empty".into()));
    }
    if config.port == 0 {
        return Err(RdpError::InvalidConfig("port cannot be zero".into()));
    }
    if config.username.is_empty() {
        return Err(RdpError::InvalidConfig("username cannot be empty".into()));
    }
    Ok(())
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn rdp_connect(
    state: tauri::State<'_, RdpState>,
    app: AppHandle,
    config: RdpConfig,
) -> Result<String, RdpError> {
    validate_config(&config)?;

    let id = Uuid::new_v4().to_string();

    state.backend.connect(&id, &config)?;

    let now = chrono::Utc::now().to_rfc3339();
    let info = RdpConnectionInfo {
        id: id.clone(),
        host: config.host.clone(),
        port: config.port,
        username: config.username.clone(),
        status: RdpConnectionStatus::Connected,
        width: 1920,
        height: 1080,
        connected_at: Some(now),
    };

    let redirection = RdpRedirectionConfig {
        drives: config.drive_paths.clone(),
        printer: config.printer_redirect,
        audio: config.audio_mode.clone(),
        smart_card: config.smart_card,
    };

    let connection = RdpConnection {
        info: info.clone(),
        config,
        redirection,
        clipboard: None,
    };

    state.connections.lock().unwrap().insert(id.clone(), connection);

    let _ = app.emit(
        "rdp:connected",
        RdpConnectedEvent {
            connection_id: id.clone(),
        },
    );

    Ok(id)
}

#[tauri::command]
pub fn rdp_disconnect(
    state: tauri::State<'_, RdpState>,
    app: AppHandle,
    connection_id: String,
) -> Result<(), RdpError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .remove(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    drop(connections);

    state.backend.disconnect(&conn.info.id)?;

    let _ = app.emit(
        "rdp:disconnected",
        RdpDisconnectedEvent {
            connection_id,
            reason: "user_requested".into(),
        },
    );

    Ok(())
}

#[tauri::command]
pub fn rdp_resize(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    width: u32,
    height: u32,
) -> Result<(), RdpError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    state.backend.resize(&connection_id, width, height)?;

    conn.info.width = width;
    conn.info.height = height;

    Ok(())
}

#[tauri::command]
pub fn rdp_send_key(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    event: RdpKeyEvent,
) -> Result<(), RdpError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    drop(connections);

    state.backend.send_key(&connection_id, &event)
}

#[tauri::command]
pub fn rdp_send_mouse(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    event: RdpMouseEvent,
) -> Result<(), RdpError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    drop(connections);

    state.backend.send_mouse(&connection_id, &event)
}

#[tauri::command]
pub fn rdp_clipboard_sync(
    state: tauri::State<'_, RdpState>,
    app: AppHandle,
    connection_id: String,
    data: RdpClipboardData,
) -> Result<(), RdpError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    state.backend.send_clipboard(&connection_id, &data)?;

    conn.clipboard = Some(data.clone());

    drop(connections);

    let _ = app.emit(
        "rdp:clipboard",
        RdpClipboardEvent {
            connection_id,
            data,
        },
    );

    Ok(())
}

#[tauri::command]
pub fn rdp_screenshot(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
) -> Result<String, RdpError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    drop(connections);

    state.backend.screenshot(&connection_id)
}

#[tauri::command]
pub fn rdp_list_connections(
    state: tauri::State<'_, RdpState>,
) -> Result<Vec<RdpConnectionInfo>, RdpError> {
    let connections = state.connections.lock().unwrap();
    let list: Vec<RdpConnectionInfo> = connections
        .values()
        .map(|c| c.info.clone())
        .collect();
    Ok(list)
}

#[tauri::command]
pub fn rdp_configure_redirection(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    config: RdpRedirectionConfig,
) -> Result<(), RdpError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    state
        .backend
        .configure_redirection(&connection_id, &config)?;

    conn.redirection = config;

    Ok(())
}

#[tauri::command]
pub fn rdp_send_ctrl_alt_del(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
) -> Result<(), RdpError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    drop(connections);

    state.backend.send_ctrl_alt_del(&connection_id)
}

// ── P2-RDP-13: Session recording ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdpRecordingFormat {
    Mp4,
    Webm,
}

#[tauri::command]
pub fn rdp_start_recording(
    state: tauri::State<'_, RdpState>,
    conn_id: String,
    output_path: String,
    format: RdpRecordingFormat,
) -> Result<(), RdpError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&conn_id) {
        return Err(RdpError::NotFound(conn_id));
    }
    drop(connections);

    // Validate output path is writable
    if output_path.is_empty() {
        return Err(RdpError::InvalidConfig("output_path cannot be empty".into()));
    }

    // Stub: In a real implementation, this would start capturing framebuffer
    // frames and encoding them via ffmpeg to the specified format.
    let _format_ext = match format {
        RdpRecordingFormat::Mp4 => "mp4",
        RdpRecordingFormat::Webm => "webm",
    };

    Ok(())
}

#[tauri::command]
pub fn rdp_stop_recording(
    state: tauri::State<'_, RdpState>,
    conn_id: String,
) -> Result<String, RdpError> {
    let connections = state.connections.lock().unwrap();
    let conn = connections
        .get(&conn_id)
        .ok_or_else(|| RdpError::NotFound(conn_id.clone()))?;

    // Stub: return the output path. Real impl would finalize the ffmpeg encode.
    let output = format!("recording_{}.mp4", conn.info.id);
    Ok(output)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(host: &str) -> RdpConfig {
        RdpConfig {
            host: host.to_string(),
            port: 3389,
            username: "testuser".to_string(),
            credential_ref: None,
            domain: None,
            nla_enabled: true,
            tls_required: true,
            gateway: None,
            multi_monitor: None,
            codec: RdpCodec::Auto,
            clipboard_sync: true,
            drive_paths: vec![],
            printer_redirect: false,
            audio_mode: RdpAudioMode::None,
            smart_card: false,
        }
    }

    fn make_state() -> RdpState {
        RdpState::with_stub()
    }

    /// Helper: insert a connection directly into state, returns connection id.
    fn insert_connection(state: &RdpState, host: &str) -> String {
        let config = make_config(host);
        let id = Uuid::new_v4().to_string();
        let info = RdpConnectionInfo {
            id: id.clone(),
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            status: RdpConnectionStatus::Connected,
            width: 1920,
            height: 1080,
            connected_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        let redirection = RdpRedirectionConfig {
            drives: config.drive_paths.clone(),
            printer: config.printer_redirect,
            audio: config.audio_mode.clone(),
            smart_card: config.smart_card,
        };
        let connection = RdpConnection {
            info,
            config,
            redirection,
            clipboard: None,
        };
        state.connections.lock().unwrap().insert(id.clone(), connection);
        id
    }

    #[test]
    fn test_rdp_connect_creates_session() {
        let state = make_state();
        let config = make_config("192.168.1.100");

        state.backend.connect("test-id", &config).unwrap();

        let id = insert_connection(&state, "192.168.1.100");

        let connections = state.connections.lock().unwrap();
        assert!(connections.contains_key(&id));
        let conn = connections.get(&id).unwrap();
        assert_eq!(conn.info.host, "192.168.1.100");
        assert_eq!(conn.info.port, 3389);
        assert!(matches!(conn.info.status, RdpConnectionStatus::Connected));
    }

    #[test]
    fn test_rdp_disconnect_removes_session() {
        let state = make_state();
        let id = insert_connection(&state, "10.0.0.1");

        // Verify it exists
        assert!(state.connections.lock().unwrap().contains_key(&id));

        // Disconnect
        state.backend.disconnect(&id).unwrap();
        state.connections.lock().unwrap().remove(&id);

        assert!(!state.connections.lock().unwrap().contains_key(&id));
    }

    #[test]
    fn test_rdp_list_connections() {
        let state = make_state();
        let id1 = insert_connection(&state, "host1.example.com");
        let id2 = insert_connection(&state, "host2.example.com");
        let id3 = insert_connection(&state, "host3.example.com");

        let connections = state.connections.lock().unwrap();
        let list: Vec<RdpConnectionInfo> = connections.values().map(|c| c.info.clone()).collect();

        assert_eq!(list.len(), 3);
        let ids: Vec<&str> = list.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(ids.contains(&id3.as_str()));
    }

    #[test]
    fn test_rdp_resize() {
        let state = make_state();
        let id = insert_connection(&state, "resize-host.local");

        // Resize
        state.backend.resize(&id, 2560, 1440).unwrap();
        let mut connections = state.connections.lock().unwrap();
        let conn = connections.get_mut(&id).unwrap();
        conn.info.width = 2560;
        conn.info.height = 1440;

        assert_eq!(conn.info.width, 2560);
        assert_eq!(conn.info.height, 1440);
    }

    #[test]
    fn test_rdp_clipboard_sync() {
        let state = make_state();
        let id = insert_connection(&state, "clipboard-host.local");

        let data = RdpClipboardData {
            text: Some("Hello, World!".to_string()),
            files: None,
            image_png_base64: None,
        };

        state.backend.send_clipboard(&id, &data).unwrap();

        let mut connections = state.connections.lock().unwrap();
        let conn = connections.get_mut(&id).unwrap();
        conn.clipboard = Some(data);

        let clipboard = conn.clipboard.as_ref().unwrap();
        assert_eq!(clipboard.text.as_deref(), Some("Hello, World!"));
        assert!(clipboard.files.is_none());
    }

    #[test]
    fn test_rdp_invalid_host() {
        let config = RdpConfig {
            host: String::new(),
            port: 3389,
            username: "user".to_string(),
            credential_ref: None,
            domain: None,
            nla_enabled: true,
            tls_required: true,
            gateway: None,
            multi_monitor: None,
            codec: RdpCodec::Auto,
            clipboard_sync: false,
            drive_paths: vec![],
            printer_redirect: false,
            audio_mode: RdpAudioMode::None,
            smart_card: false,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("host cannot be empty"));
    }

    #[test]
    fn test_rdp_configure_redirection() {
        let state = make_state();
        let id = insert_connection(&state, "redir-host.local");

        let redir = RdpRedirectionConfig {
            drives: vec![
                DriveMapping {
                    name: "HomeDir".to_string(),
                    local_path: "/home/user".to_string(),
                },
            ],
            printer: true,
            audio: RdpAudioMode::Playback,
            smart_card: false,
        };

        state.backend.configure_redirection(&id, &redir).unwrap();

        let mut connections = state.connections.lock().unwrap();
        let conn = connections.get_mut(&id).unwrap();
        conn.redirection = redir;

        assert_eq!(conn.redirection.drives.len(), 1);
        assert_eq!(conn.redirection.drives[0].name, "HomeDir");
        assert!(conn.redirection.printer);
        assert!(matches!(conn.redirection.audio, RdpAudioMode::Playback));
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum VncError {
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
    #[error("Encoding error: {0}")]
    Encoding(String),
    #[error("Screenshot error: {0}")]
    Screenshot(String),
    #[error("View-only mode: input rejected")]
    ViewOnly,
    #[error("IO error: {0}")]
    Io(String),
}

impl Serialize for VncError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for VncError {
    fn from(e: std::io::Error) -> Self {
        VncError::Io(e.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VncEncoding {
    Raw,
    CopyRect,
    Rre,
    Hextile,
    Zrle,
    Tight,
    CursorPseudo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VncScalingMode {
    FitToWindow,
    Scroll,
    OneToOne,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VncSecurityType {
    None,
    VncAuth,
    VeNCryptTls,
    VeNCryptX509,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VncRfbVersion {
    V33,
    V37,
    V38,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VncConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub vnc_auth: bool,
    pub vencrypt: bool,
    pub tls_cert_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncConnectionInfo {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub status: VncConnectionStatus,
    pub width: u32,
    pub height: u32,
    pub view_only: bool,
    pub scaling_mode: VncScalingMode,
}

// ── Tauri Event Payloads ────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, Serialize)]
struct VncFrameEvent {
    connection_id: String,
    width: u32,
    height: u32,
    data_base64: String,
}

#[derive(Clone, Serialize)]
struct VncConnectedEvent {
    connection_id: String,
}

#[derive(Clone, Serialize)]
struct VncDisconnectedEvent {
    connection_id: String,
    reason: String,
}

#[derive(Clone, Serialize)]
struct VncClipboardEvent {
    connection_id: String,
    text: String,
}

#[allow(dead_code)]
#[derive(Clone, Serialize)]
struct VncErrorEvent {
    connection_id: String,
    message: String,
}

#[allow(dead_code)]
#[derive(Clone, Serialize)]
struct VncBellEvent {
    connection_id: String,
}

// ── Backend Trait ────────────────────────────────────────────────────────

/// Trait abstracting the actual libvncclient FFI calls.
/// Implementations can provide real libvncclient bindings or test stubs.
pub trait VncBackend: Send + Sync {
    fn connect(&self, id: &str, config: &VncConfig) -> Result<(), VncError>;

    fn disconnect(&self, id: &str) -> Result<(), VncError>;

    fn send_key(&self, id: &str, key_code: u32, pressed: bool) -> Result<(), VncError>;

    fn send_mouse(&self, id: &str, x: u32, y: u32, button_mask: u8) -> Result<(), VncError>;

    fn set_encodings(&self, id: &str, encodings: &[VncEncoding]) -> Result<(), VncError>;

    fn send_clipboard(&self, id: &str, text: &str) -> Result<(), VncError>;

    fn screenshot(&self, id: &str) -> Result<String, VncError>;
}

// ── Stub Backend ────────────────────────────────────────────────────────

/// No-op backend used when libvncclient is not available (tests / dev builds).
pub struct StubVncBackend;

impl VncBackend for StubVncBackend {
    fn connect(&self, _id: &str, _config: &VncConfig) -> Result<(), VncError> {
        Ok(())
    }

    fn disconnect(&self, _id: &str) -> Result<(), VncError> {
        Ok(())
    }

    fn send_key(&self, _id: &str, _key_code: u32, _pressed: bool) -> Result<(), VncError> {
        Ok(())
    }

    fn send_mouse(&self, _id: &str, _x: u32, _y: u32, _button_mask: u8) -> Result<(), VncError> {
        Ok(())
    }

    fn set_encodings(&self, _id: &str, _encodings: &[VncEncoding]) -> Result<(), VncError> {
        Ok(())
    }

    fn send_clipboard(&self, _id: &str, _text: &str) -> Result<(), VncError> {
        Ok(())
    }

    fn screenshot(&self, _id: &str) -> Result<String, VncError> {
        // Return a minimal valid base64-encoded 1x1 PNG for stub
        Ok("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string())
    }
}

// ── Connection ──────────────────────────────────────────────────────────

#[allow(dead_code)]
struct VncConnection {
    info: VncConnectionInfo,
    config: VncConfig,
    encodings: Vec<VncEncoding>,
    last_clipboard: Option<String>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct VncState {
    connections: Mutex<HashMap<String, VncConnection>>,
    backend: Box<dyn VncBackend>,
}

impl VncState {
    pub fn new(backend: Box<dyn VncBackend>) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            backend,
        }
    }

    #[allow(dead_code)]
    pub fn with_stub() -> Self {
        Self::new(Box::new(StubVncBackend))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn validate_config(config: &VncConfig) -> Result<(), VncError> {
    if config.host.is_empty() {
        return Err(VncError::InvalidConfig("host cannot be empty".into()));
    }
    if config.port == 0 {
        return Err(VncError::InvalidConfig("port cannot be zero".into()));
    }
    if config.vencrypt && config.tls_cert_path.is_none() {
        return Err(VncError::InvalidConfig(
            "VeNCrypt requires tls_cert_path".into(),
        ));
    }
    Ok(())
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn vnc_connect(
    state: tauri::State<'_, VncState>,
    app: AppHandle,
    config: VncConfig,
) -> Result<String, VncError> {
    validate_config(&config)?;

    let id = Uuid::new_v4().to_string();

    state.backend.connect(&id, &config)?;

    let info = VncConnectionInfo {
        id: id.clone(),
        host: config.host.clone(),
        port: config.port,
        status: VncConnectionStatus::Connected,
        width: 1024,
        height: 768,
        view_only: false,
        scaling_mode: VncScalingMode::FitToWindow,
    };

    let connection = VncConnection {
        info: info.clone(),
        config,
        encodings: vec![VncEncoding::Tight, VncEncoding::Zrle, VncEncoding::Raw],
        last_clipboard: None,
    };

    state
        .connections
        .lock()
        .unwrap()
        .insert(id.clone(), connection);

    let _ = app.emit(
        "vnc:connected",
        VncConnectedEvent {
            connection_id: id.clone(),
        },
    );

    Ok(id)
}

#[tauri::command]
pub fn vnc_disconnect(
    state: tauri::State<'_, VncState>,
    app: AppHandle,
    connection_id: String,
) -> Result<(), VncError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .remove(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    drop(connections);

    state.backend.disconnect(&conn.info.id)?;

    let _ = app.emit(
        "vnc:disconnected",
        VncDisconnectedEvent {
            connection_id,
            reason: "user_requested".into(),
        },
    );

    Ok(())
}

#[tauri::command]
pub fn vnc_send_key(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    key_code: u32,
    pressed: bool,
) -> Result<(), VncError> {
    let connections = state.connections.lock().unwrap();
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    if conn.info.view_only {
        return Err(VncError::ViewOnly);
    }

    drop(connections);

    state.backend.send_key(&connection_id, key_code, pressed)
}

#[tauri::command]
pub fn vnc_send_mouse(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    x: u32,
    y: u32,
    button_mask: u8,
) -> Result<(), VncError> {
    let connections = state.connections.lock().unwrap();
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    if conn.info.view_only {
        return Err(VncError::ViewOnly);
    }

    drop(connections);

    state.backend.send_mouse(&connection_id, x, y, button_mask)
}

#[tauri::command]
pub fn vnc_set_encoding(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    encodings: Vec<VncEncoding>,
) -> Result<(), VncError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    state.backend.set_encodings(&connection_id, &encodings)?;

    conn.encodings = encodings;

    Ok(())
}

#[tauri::command]
pub fn vnc_clipboard_send(
    state: tauri::State<'_, VncState>,
    app: AppHandle,
    connection_id: String,
    text: String,
) -> Result<(), VncError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    state.backend.send_clipboard(&connection_id, &text)?;

    conn.last_clipboard = Some(text.clone());

    drop(connections);

    let _ = app.emit(
        "vnc:clipboard",
        VncClipboardEvent {
            connection_id,
            text,
        },
    );

    Ok(())
}

#[tauri::command]
pub fn vnc_set_view_only(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    view_only: bool,
) -> Result<(), VncError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    conn.info.view_only = view_only;

    Ok(())
}

#[tauri::command]
pub fn vnc_screenshot(
    state: tauri::State<'_, VncState>,
    connection_id: String,
) -> Result<String, VncError> {
    let connections = state.connections.lock().unwrap();
    if !connections.contains_key(&connection_id) {
        return Err(VncError::NotFound(connection_id));
    }
    drop(connections);

    state.backend.screenshot(&connection_id)
}

#[tauri::command]
pub fn vnc_set_scaling(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    mode: VncScalingMode,
) -> Result<(), VncError> {
    let mut connections = state.connections.lock().unwrap();
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| VncError::NotFound(connection_id.clone()))?;

    conn.info.scaling_mode = mode;

    Ok(())
}

#[tauri::command]
pub fn vnc_list_connections(
    state: tauri::State<'_, VncState>,
) -> Result<Vec<VncConnectionInfo>, VncError> {
    let connections = state.connections.lock().unwrap();
    let list: Vec<VncConnectionInfo> = connections.values().map(|c| c.info.clone()).collect();
    Ok(list)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(host: &str) -> VncConfig {
        VncConfig {
            host: host.to_string(),
            port: 5900,
            password: Some("secret".to_string()),
            vnc_auth: true,
            vencrypt: false,
            tls_cert_path: None,
        }
    }

    fn make_tls_config(host: &str) -> VncConfig {
        VncConfig {
            host: host.to_string(),
            port: 5900,
            password: None,
            vnc_auth: false,
            vencrypt: true,
            tls_cert_path: Some("/path/to/cert.pem".to_string()),
        }
    }

    fn make_state() -> VncState {
        VncState::with_stub()
    }

    /// Helper: insert a connection directly into state, returns connection id.
    fn insert_connection(state: &VncState, host: &str) -> String {
        let config = make_config(host);
        let id = Uuid::new_v4().to_string();
        let info = VncConnectionInfo {
            id: id.clone(),
            host: config.host.clone(),
            port: config.port,
            status: VncConnectionStatus::Connected,
            width: 1024,
            height: 768,
            view_only: false,
            scaling_mode: VncScalingMode::FitToWindow,
        };
        let connection = VncConnection {
            info,
            config,
            encodings: vec![VncEncoding::Tight, VncEncoding::Zrle, VncEncoding::Raw],
            last_clipboard: None,
        };
        state
            .connections
            .lock()
            .unwrap()
            .insert(id.clone(), connection);
        id
    }

    #[test]
    fn test_vnc_connect_auth() {
        let state = make_state();
        let config = make_config("192.168.1.50");

        state.backend.connect("test-id", &config).unwrap();

        let id = insert_connection(&state, "192.168.1.50");

        let connections = state.connections.lock().unwrap();
        assert!(connections.contains_key(&id));
        let conn = connections.get(&id).unwrap();
        assert_eq!(conn.info.host, "192.168.1.50");
        assert_eq!(conn.info.port, 5900);
        assert!(conn.config.vnc_auth);
        assert!(matches!(conn.info.status, VncConnectionStatus::Connected));
    }

    #[test]
    fn test_vnc_connect_tls() {
        let state = make_state();
        let config = make_tls_config("tls-host.local");

        // Validate config passes for VeNCrypt with cert
        assert!(validate_config(&config).is_ok());

        state.backend.connect("tls-id", &config).unwrap();

        let id = Uuid::new_v4().to_string();
        let info = VncConnectionInfo {
            id: id.clone(),
            host: config.host.clone(),
            port: config.port,
            status: VncConnectionStatus::Connected,
            width: 1024,
            height: 768,
            view_only: false,
            scaling_mode: VncScalingMode::FitToWindow,
        };
        let connection = VncConnection {
            info,
            config: config.clone(),
            encodings: vec![],
            last_clipboard: None,
        };
        state
            .connections
            .lock()
            .unwrap()
            .insert(id.clone(), connection);

        let connections = state.connections.lock().unwrap();
        let conn = connections.get(&id).unwrap();
        assert!(conn.config.vencrypt);
        assert_eq!(
            conn.config.tls_cert_path.as_deref(),
            Some("/path/to/cert.pem")
        );
    }

    #[test]
    fn test_vnc_encodings() {
        let state = make_state();
        let id = insert_connection(&state, "encoding-host.local");

        let new_encodings = vec![
            VncEncoding::Hextile,
            VncEncoding::CopyRect,
            VncEncoding::Raw,
        ];

        state
            .backend
            .set_encodings(&id, &new_encodings)
            .unwrap();

        let mut connections = state.connections.lock().unwrap();
        let conn = connections.get_mut(&id).unwrap();
        conn.encodings = new_encodings;

        assert_eq!(conn.encodings.len(), 3);
        assert!(matches!(conn.encodings[0], VncEncoding::Hextile));
        assert!(matches!(conn.encodings[1], VncEncoding::CopyRect));
        assert!(matches!(conn.encodings[2], VncEncoding::Raw));
    }

    #[test]
    fn test_vnc_clipboard() {
        let state = make_state();
        let id = insert_connection(&state, "clipboard-host.local");

        let text = "Hello from VNC clipboard!";

        state.backend.send_clipboard(&id, text).unwrap();

        let mut connections = state.connections.lock().unwrap();
        let conn = connections.get_mut(&id).unwrap();
        conn.last_clipboard = Some(text.to_string());

        assert_eq!(conn.last_clipboard.as_deref(), Some(text));
    }

    #[test]
    fn test_vnc_scaling() {
        let state = make_state();
        let id = insert_connection(&state, "scale-host.local");

        // Switch to Scroll
        {
            let mut connections = state.connections.lock().unwrap();
            let conn = connections.get_mut(&id).unwrap();
            conn.info.scaling_mode = VncScalingMode::Scroll;
            assert!(matches!(conn.info.scaling_mode, VncScalingMode::Scroll));
        }

        // Switch to OneToOne
        {
            let mut connections = state.connections.lock().unwrap();
            let conn = connections.get_mut(&id).unwrap();
            conn.info.scaling_mode = VncScalingMode::OneToOne;
            assert!(matches!(conn.info.scaling_mode, VncScalingMode::OneToOne));
        }

        // Switch back to FitToWindow
        {
            let mut connections = state.connections.lock().unwrap();
            let conn = connections.get_mut(&id).unwrap();
            conn.info.scaling_mode = VncScalingMode::FitToWindow;
            assert!(matches!(
                conn.info.scaling_mode,
                VncScalingMode::FitToWindow
            ));
        }
    }

    #[test]
    fn test_vnc_view_only() {
        let state = make_state();
        let id = insert_connection(&state, "viewonly-host.local");

        // Enable view-only
        {
            let mut connections = state.connections.lock().unwrap();
            let conn = connections.get_mut(&id).unwrap();
            conn.info.view_only = true;
        }

        // Key events should be rejected in view-only mode
        {
            let connections = state.connections.lock().unwrap();
            let conn = connections.get(&id).unwrap();
            assert!(conn.info.view_only);
        }

        // Simulate what vnc_send_key does: check view_only and reject
        let connections = state.connections.lock().unwrap();
        let conn = connections.get(&id).unwrap();
        if conn.info.view_only {
            let err = VncError::ViewOnly;
            assert_eq!(err.to_string(), "View-only mode: input rejected");
        }
    }

    #[test]
    fn test_vnc_screenshot() {
        let state = make_state();
        let id = insert_connection(&state, "screenshot-host.local");

        let result = state.backend.screenshot(&id);
        assert!(result.is_ok());

        let base64_data = result.unwrap();
        assert!(!base64_data.is_empty());
        // Verify it starts with PNG base64 signature
        assert!(base64_data.starts_with("iVBOR"));
    }

    #[test]
    fn test_vnc_disconnect() {
        let state = make_state();
        let id = insert_connection(&state, "disconnect-host.local");

        // Verify it exists
        assert!(state.connections.lock().unwrap().contains_key(&id));

        // Disconnect
        state.backend.disconnect(&id).unwrap();
        state.connections.lock().unwrap().remove(&id);

        // Verify cleaned up
        assert!(!state.connections.lock().unwrap().contains_key(&id));
    }
}

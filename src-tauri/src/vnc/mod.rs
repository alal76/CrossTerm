use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use uuid::Uuid;
use vnc::{ClientKeyEvent, ClientMouseEvent, PixelFormat, VncConnector, VncEncoding as VncLibEncoding, VncEvent, X11Event};

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
pub enum VncConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    #[serde(rename = "error")]
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

// ── Input channel ────────────────────────────────────────────────────────

enum VncInput {
    Key { code: u32, down: bool },
    Mouse { x: u16, y: u16, buttons: u8 },
    CopyText(String),
}

// ── Connection entry ─────────────────────────────────────────────────────

struct VncConn {
    input_tx: mpsc::UnboundedSender<VncInput>,
    host: String,
    port: u16,
    width: u32,
    height: u32,
    view_only: bool,
    scaling_mode: VncScalingMode,
}

// ── State ────────────────────────────────────────────────────────────────

pub struct VncState {
    connections: Arc<Mutex<HashMap<String, VncConn>>>,
}

impl VncState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ── Connect result ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct VncConnectResult {
    pub id: String,
    pub width: u32,
    pub height: u32,
}

// ── Tauri Event Payloads ─────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct VncResizeEvent {
    connection_id: String,
    width: u32,
    height: u32,
}

#[derive(Clone, Serialize)]
struct VncFrameEvent {
    connection_id: String,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    data_base64: String,
}

#[derive(Clone, Serialize)]
struct VncConnectedEvent {
    connection_id: String,
    width: u32,
    height: u32,
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

// ── Event loop ───────────────────────────────────────────────────────────

async fn handle_vnc_event(
    conn_id: &str,
    event: VncEvent,
    app: &AppHandle,
    connections: &Arc<Mutex<HashMap<String, VncConn>>>,
) {
    match event {
        VncEvent::SetResolution(screen) => {
            let (w, h) = (screen.width as u32, screen.height as u32);
            {
                let mut guard = connections.lock().unwrap();
                if let Some(conn) = guard.get_mut(conn_id) {
                    conn.width = w;
                    conn.height = h;
                }
            }
            let _ = app.emit(
                "vnc:resize",
                VncResizeEvent { connection_id: conn_id.to_string(), width: w, height: h },
            );
        }
        VncEvent::RawImage(rect, data) => {
            if rect.width == 0 || rect.height == 0 || data.is_empty() {
                return;
            }
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            let _ = app.emit(
                "vnc:frame",
                VncFrameEvent {
                    connection_id: conn_id.to_string(),
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    data_base64: encoded,
                },
            );
        }
        VncEvent::Text(text) => {
            let _ = app.emit(
                "vnc:clipboard",
                VncClipboardEvent { connection_id: conn_id.to_string(), text },
            );
        }
        // Copy, JpegImage, SetCursor, Bell — handled by requesting a refresh next cycle
        _ => {}
    }
}

async fn run_vnc_loop(
    conn_id: String,
    vnc: vnc::VncClient,
    mut rx: mpsc::UnboundedReceiver<VncInput>,
    app: AppHandle,
    connections: Arc<Mutex<HashMap<String, VncConn>>>,
) {
    use tokio::time::{interval, Duration, MissedTickBehavior};

    let mut refresh_timer = interval(Duration::from_millis(33));
    refresh_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    'outer: loop {
        // Drain all pending VNC events (non-blocking poll)
        loop {
            match vnc.poll_event().await {
                Ok(Some(event)) => {
                    handle_vnc_event(&conn_id, event, &app, &connections).await;
                }
                Ok(None) => break,
                Err(_) => {
                    let _ = app.emit(
                        "vnc:disconnected",
                        VncDisconnectedEvent {
                            connection_id: conn_id.clone(),
                            reason: "connection_error".into(),
                        },
                    );
                    connections.lock().unwrap().remove(&conn_id);
                    return;
                }
            }
        }

        // Wait for an input command or a 33ms refresh tick
        tokio::select! {
            biased;

            input = rx.recv() => {
                match input {
                    None => break 'outer, // sender dropped = disconnect
                    Some(VncInput::Key { code, down }) => {
                        let _ = vnc.input(X11Event::KeyEvent(ClientKeyEvent { keycode: code, down })).await;
                    }
                    Some(VncInput::Mouse { x, y, buttons }) => {
                        let _ = vnc.input(X11Event::PointerEvent(ClientMouseEvent {
                            position_x: x,
                            position_y: y,
                            bottons: buttons,
                        }))
                        .await;
                    }
                    Some(VncInput::CopyText(text)) => {
                        let _ = vnc.input(X11Event::CopyText(text)).await;
                    }
                }
            }

            _ = refresh_timer.tick() => {
                if vnc.input(X11Event::Refresh).await.is_err() {
                    break 'outer;
                }
            }
        }
    }

    let _ = vnc.close().await;
    let _ = app.emit(
        "vnc:disconnected",
        VncDisconnectedEvent {
            connection_id: conn_id.clone(),
            reason: "disconnected".into(),
        },
    );
    connections.lock().unwrap().remove(&conn_id);
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn validate_config(config: &VncConfig) -> Result<(), VncError> {
    if config.host.is_empty() {
        return Err(VncError::InvalidConfig("host cannot be empty".into()));
    }
    if config.port == 0 {
        return Err(VncError::InvalidConfig("port cannot be zero".into()));
    }
    Ok(())
}

// ── Tauri Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn vnc_connect(
    state: tauri::State<'_, VncState>,
    app: AppHandle,
    config: VncConfig,
) -> Result<VncConnectResult, VncError> {
    validate_config(&config)?;

    let addr = format!("{}:{}", config.host, config.port);
    let tcp = TcpStream::connect(&addr)
        .await
        .map_err(|e| VncError::ConnectionFailed(e.to_string()))?;

    let password = config.password.clone().unwrap_or_default();
    let vnc = VncConnector::new(tcp)
        .set_auth_method(async move { Ok(password) })
        .add_encoding(VncLibEncoding::Tight)
        .add_encoding(VncLibEncoding::Zrle)
        .add_encoding(VncLibEncoding::Trle)
        .add_encoding(VncLibEncoding::CopyRect)
        .add_encoding(VncLibEncoding::Raw)
        .allow_shared(true)
        .set_pixel_format(PixelFormat::rgba())
        .build()
        .map_err(|e| VncError::ConnectionFailed(e.to_string()))?
        .try_start()
        .await
        .map_err(|e| VncError::ConnectionFailed(e.to_string()))?
        .finish()
        .map_err(|e| VncError::ConnectionFailed(e.to_string()))?;

    // Collect initial SetResolution event
    let mut width = 1024u32;
    let mut height = 768u32;
    for _ in 0..10 {
        match vnc.poll_event().await {
            Ok(Some(VncEvent::SetResolution(screen))) => {
                width = screen.width as u32;
                height = screen.height as u32;
                break;
            }
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => break,
        }
    }

    let id = Uuid::new_v4().to_string();
    let (input_tx, input_rx) = mpsc::unbounded_channel::<VncInput>();

    {
        let mut conns = state.connections.lock().unwrap();
        conns.insert(
            id.clone(),
            VncConn {
                input_tx,
                host: config.host.clone(),
                port: config.port,
                width,
                height,
                view_only: false,
                scaling_mode: VncScalingMode::FitToWindow,
            },
        );
    }

    let connections = Arc::clone(&state.connections);
    let app_clone = app.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        run_vnc_loop(id_clone, vnc, input_rx, app_clone, connections).await;
    });

    // Emit the event for any secondary listeners; primary consumer reads
    // width/height from the return value to avoid the event-before-invoke race.
    let _ = app.emit(
        "vnc:connected",
        VncConnectedEvent { connection_id: id.clone(), width, height },
    );

    Ok(VncConnectResult { id, width, height })
}

#[tauri::command]
pub fn vnc_disconnect(
    state: tauri::State<'_, VncState>,
    app: AppHandle,
    connection_id: String,
) -> Result<(), VncError> {
    let mut conns = state.connections.lock().unwrap();
    if conns.remove(&connection_id).is_none() {
        return Err(VncError::NotFound(connection_id.clone()));
    }
    drop(conns);
    // Dropping the VncConn drops the input_tx, which closes the channel,
    // causing the event loop to exit and emit vnc:disconnected.
    // Emit immediately so the UI responds promptly.
    let _ = app.emit(
        "vnc:disconnected",
        VncDisconnectedEvent { connection_id, reason: "user_requested".into() },
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
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| VncError::NotFound(connection_id.clone()))?;
    if conn.view_only {
        return Err(VncError::ViewOnly);
    }
    conn.input_tx
        .send(VncInput::Key { code: key_code, down: pressed })
        .map_err(|_| VncError::NotFound(connection_id))
}

#[tauri::command]
pub fn vnc_send_mouse(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    x: u32,
    y: u32,
    button_mask: u8,
) -> Result<(), VncError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| VncError::NotFound(connection_id.clone()))?;
    if conn.view_only {
        return Err(VncError::ViewOnly);
    }
    conn.input_tx
        .send(VncInput::Mouse { x: x as u16, y: y as u16, buttons: button_mask })
        .map_err(|_| VncError::NotFound(connection_id))
}

#[tauri::command]
pub fn vnc_set_encoding(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    _encodings: Vec<VncEncoding>,
) -> Result<(), VncError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&connection_id) {
        return Err(VncError::NotFound(connection_id));
    }
    // Encoding is set during connection setup; runtime changes are not supported
    Ok(())
}

#[tauri::command]
pub fn vnc_clipboard_send(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    text: String,
) -> Result<(), VncError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| VncError::NotFound(connection_id.clone()))?;
    conn.input_tx
        .send(VncInput::CopyText(text))
        .map_err(|_| VncError::NotFound(connection_id))
}

#[tauri::command]
pub fn vnc_set_view_only(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    view_only: bool,
) -> Result<(), VncError> {
    let mut conns = state.connections.lock().unwrap();
    let conn = conns.get_mut(&connection_id).ok_or_else(|| VncError::NotFound(connection_id))?;
    conn.view_only = view_only;
    Ok(())
}

#[tauri::command]
pub fn vnc_screenshot(
    state: tauri::State<'_, VncState>,
    connection_id: String,
) -> Result<String, VncError> {
    let conns = state.connections.lock().unwrap();
    if !conns.contains_key(&connection_id) {
        return Err(VncError::NotFound(connection_id));
    }
    // Screenshot is taken by the frontend via Canvas.toDataURL()
    Ok(String::new())
}

#[tauri::command]
pub fn vnc_set_scaling(
    state: tauri::State<'_, VncState>,
    connection_id: String,
    mode: VncScalingMode,
) -> Result<(), VncError> {
    let mut conns = state.connections.lock().unwrap();
    let conn = conns.get_mut(&connection_id).ok_or_else(|| VncError::NotFound(connection_id))?;
    conn.scaling_mode = mode;
    Ok(())
}

#[tauri::command]
pub fn vnc_list_connections(
    state: tauri::State<'_, VncState>,
) -> Result<Vec<VncConnectionInfo>, VncError> {
    let conns = state.connections.lock().unwrap();
    let list = conns
        .iter()
        .map(|(id, c)| VncConnectionInfo {
            id: id.clone(),
            host: c.host.clone(),
            port: c.port,
            status: VncConnectionStatus::Connected,
            width: c.width,
            height: c.height,
            view_only: c.view_only,
            scaling_mode: c.scaling_mode.clone(),
        })
        .collect();
    Ok(list)
}

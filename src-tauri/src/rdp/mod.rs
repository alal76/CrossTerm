use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

use ironrdp::connector::{self, ClientConnector, ConnectionResult, Credentials};
use ironrdp::pdu::gcc::KeyboardType;
use ironrdp::pdu::rdp::capability_sets::MajorPlatformType;
use ironrdp::pdu::rdp::client_info::{PerformanceFlags, TimezoneInfo};
use ironrdp::session::image::DecodedImage;
use ironrdp::session::{ActiveStage, ActiveStageOutput};
use ironrdp_blocking::{connect_begin, connect_finalize, mark_as_upgraded, Framed};
use ironrdp::graphics::image_processing::PixelFormat;
use ironrdp::pdu::geometry::{InclusiveRectangle, Rectangle as _};
use ironrdp::pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp::pdu::input::mouse::{MousePdu, PointerFlags};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConnection, DigitallySignedStruct, Error as TlsError, SignatureScheme};

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

// ── Public config types ─────────────────────────────────────────────────

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
    pub password: String,
    pub credential_ref: Option<String>,
    pub domain: Option<String>,
    pub nla_enabled: bool,
    pub tls_required: bool,
    pub width: Option<u16>,
    pub height: Option<u16>,
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
    pub scan_code: u8,
    pub pressed: bool,
    pub extended: bool,
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

// ── Connect result ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RdpConnectResult {
    pub id: String,
    pub width: u16,
    pub height: u16,
}

// ── Tauri event payloads ────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct RdpConnectedEvent {
    connection_id: String,
    width: u16,
    height: u16,
}

#[derive(Clone, Serialize)]
struct RdpFrameEvent {
    connection_id: String,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    data_base64: String,
}

#[derive(Clone, Serialize)]
struct RdpDisconnectedEvent {
    connection_id: String,
    reason: String,
}

// ── Internal connection types ───────────────────────────────────────────

enum RdpInput {
    FastPath(FastPathInputEvent),
    Disconnect,
}

struct RdpConn {
    input_tx: std::sync::mpsc::Sender<RdpInput>,
    host: String,
    port: u16,
    username: String,
    width: u16,
    height: u16,
}

type ConnMap = Arc<Mutex<HashMap<String, RdpConn>>>;

// ── State ───────────────────────────────────────────────────────────────

pub struct RdpState {
    connections: ConnMap,
}

impl RdpState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ── TLS: no-op certificate verifier ────────────────────────────────────

#[derive(Debug)]
struct NoCertVerifier;

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

// ── CredSSP stub ────────────────────────────────────────────────────────

struct NoopNetworkClient;

impl ironrdp::connector::sspi::network_client::NetworkClient for NoopNetworkClient {
    fn send(
        &self,
        _request: &ironrdp::connector::sspi::NetworkRequest,
    ) -> ironrdp::connector::sspi::Result<Vec<u8>> {
        unreachable!("CredSSP is disabled")
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

fn make_tls_config() -> Arc<rustls::ClientConfig> {
    let mut cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    // CredSSP does not support TLS session resumption.
    cfg.resumption = rustls::client::Resumption::disabled();
    Arc::new(cfg)
}

fn make_connector_config(config: &RdpConfig) -> connector::Config {
    let width = config.width.unwrap_or(1920);
    let height = config.height.unwrap_or(1080);
    connector::Config {
        credentials: Credentials::UsernamePassword {
            username: config.username.clone(),
            password: config.password.clone(),
        },
        domain: config.domain.clone(),
        enable_tls: true,
        enable_credssp: false,
        keyboard_type: KeyboardType::IbmEnhanced,
        keyboard_subtype: 0,
        keyboard_layout: 0,
        keyboard_functional_keys_count: 12,
        ime_file_name: String::new(),
        dig_product_id: String::new(),
        desktop_size: connector::DesktopSize { width, height },
        bitmap: None,
        client_build: 0,
        client_name: "CrossTerm".to_owned(),
        client_dir: "C:\\Windows\\System32\\mstscax.dll".to_owned(),
        #[cfg(target_os = "macos")]
        platform: MajorPlatformType::MACINTOSH,
        #[cfg(target_os = "windows")]
        platform: MajorPlatformType::WINDOWS,
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        platform: MajorPlatformType::UNIX,
        enable_server_pointer: true,
        request_data: None,
        autologon: false,
        enable_audio_playback: false,
        pointer_software_rendering: true,
        performance_flags: PerformanceFlags::default(),
        desktop_scale_factor: 0,
        hardware_id: None,
        license_cache: None,
        timezone_info: TimezoneInfo::default(),
    }
}

/// Extract contiguous RGBA pixels for a rectangle, copying row by row
/// to skip the stride gap that `data_for_rect` would include.
fn extract_rect_rgba(image: &DecodedImage, rect: &InclusiveRectangle) -> Vec<u8> {
    let bpp = image.bytes_per_pixel();
    let stride = image.stride();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let left = rect.left as usize;
    let top = rect.top as usize;
    let data = image.data();
    let mut pixels = Vec::with_capacity(w * h * bpp);
    for row in 0..h {
        let row_start = (top + row) * stride + left * bpp;
        pixels.extend_from_slice(&data[row_start..row_start + w * bpp]);
    }
    pixels
}

fn emit_frame(app: &AppHandle, conn_id: &str, image: &DecodedImage, rect: &InclusiveRectangle) {
    let pixels = extract_rect_rgba(image, rect);
    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&pixels);
    let _ = app.emit(
        "rdp:frame",
        RdpFrameEvent {
            connection_id: conn_id.to_owned(),
            x: rect.left,
            y: rect.top,
            width: rect.width(),
            height: rect.height(),
            data_base64,
        },
    );
}

// ── Session thread ──────────────────────────────────────────────────────

type TlsStream = rustls::StreamOwned<ClientConnection, TcpStream>;
type TlsFramed = Framed<TlsStream>;

fn rdp_thread(
    conn_id: String,
    config: RdpConfig,
    connections: ConnMap,
    app: AppHandle,
    ready_tx: tokio::sync::oneshot::Sender<Result<(u16, u16), String>>,
) {
    // ── Connection phase ────────────────────────────────────────────────
    let result = (|| -> Result<(TlsFramed, ActiveStage, DecodedImage, std::sync::mpsc::Receiver<RdpInput>), String> {
        let addr = format!("{}:{}", config.host, config.port);
        let tcp = TcpStream::connect(&addr).map_err(|e| e.to_string())?;
        let client_addr = tcp.local_addr().map_err(|e| e.to_string())?;

        // 50 ms read timeout so the session loop can check inputs between reads.
        tcp.set_read_timeout(Some(Duration::from_millis(50)))
            .map_err(|e| e.to_string())?;

        let mut framed = Framed::new(tcp);
        let connector_cfg = make_connector_config(&config);
        let mut connector = ClientConnector::new(connector_cfg, client_addr);

        let should_upgrade =
            connect_begin(&mut framed, &mut connector).map_err(|e| e.to_string())?;

        // Extract the TCP stream for TLS upgrade.
        let tcp_stream = framed.into_inner_no_leftover();

        let tls_config = make_tls_config();
        let server_name = ServerName::try_from(config.host.as_str())
            .map_err(|e| e.to_string())?
            .to_owned();
        let tls_client = ClientConnection::new(tls_config, server_name)
            .map_err(|e| e.to_string())?;
        let mut tls_stream = rustls::StreamOwned::new(tls_client, tcp_stream);
        tls_stream.flush().map_err(|e| e.to_string())?;

        let upgraded = mark_as_upgraded(should_upgrade, &mut connector);
        let mut upgraded_framed = Framed::new(tls_stream);

        let connection_result: ConnectionResult = connect_finalize(
            upgraded,
            connector,
            &mut upgraded_framed,
            &mut NoopNetworkClient,
            config.host.clone().into(),
            Vec::new(), // server_public_key — unused when enable_credssp = false
            None,       // no Kerberos config
        )
        .map_err(|e| e.to_string())?;

        let width = connection_result.desktop_size.width;
        let height = connection_result.desktop_size.height;
        let image = DecodedImage::new(PixelFormat::RgbA32, width, height);
        let active_stage = ActiveStage::new(connection_result);

        let (tx, rx) = std::sync::mpsc::channel::<RdpInput>();
        connections.lock().unwrap().insert(
            conn_id.clone(),
            RdpConn {
                input_tx: tx,
                host: config.host.clone(),
                port: config.port,
                username: config.username.clone(),
                width,
                height,
            },
        );

        Ok((upgraded_framed, active_stage, image, rx))
    })();

    match result {
        Ok((mut framed, mut active_stage, mut image, rx)) => {
            let width = image.width();
            let height = image.height();
            let _ = ready_tx.send(Ok((width, height)));

            // ── Active session loop ─────────────────────────────────────
            'outer: loop {
                // Drain pending inputs.
                loop {
                    match rx.try_recv() {
                        Ok(RdpInput::Disconnect) => break 'outer,
                        Ok(RdpInput::FastPath(event)) => {
                            match active_stage.process_fastpath_input(&mut image, &[event]) {
                                Ok(outputs) => {
                                    for out in outputs {
                                        match out {
                                            ActiveStageOutput::ResponseFrame(frame)
                                                if framed.write_all(&frame).is_err() => {
                                                break 'outer;
                                            }
                                            ActiveStageOutput::GraphicsUpdate(rect) => {
                                                emit_frame(&app, &conn_id, &image, &rect);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Err(_) => break 'outer,
                            }
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break 'outer,
                    }
                }

                // Read one PDU from the server (returns WouldBlock/TimedOut on timeout).
                match framed.read_pdu() {
                    Ok((action, payload)) => {
                        match active_stage.process(&mut image, action, &payload) {
                            Ok(outputs) => {
                                for out in outputs {
                                    match out {
                                        ActiveStageOutput::ResponseFrame(frame)
                                            if framed.write_all(&frame).is_err() => {
                                            break 'outer;
                                        }
                                        ActiveStageOutput::GraphicsUpdate(rect) => {
                                            emit_frame(&app, &conn_id, &image, &rect);
                                        }
                                        ActiveStageOutput::Terminate(_) => break 'outer,
                                        // DeactivateAll happens on server-side resize;
                                        // a full reconnect is required to handle it properly.
                                        ActiveStageOutput::DeactivateAll(_) => break 'outer,
                                        _ => {}
                                    }
                                }
                            }
                            Err(_) => break 'outer,
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Timeout — loop back to check inputs.
                    }
                    Err(_) => break 'outer,
                }
            }
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
        }
    }

    // Clean up.
    connections.lock().unwrap().remove(&conn_id);
    let _ = app.emit(
        "rdp:disconnected",
        RdpDisconnectedEvent {
            connection_id: conn_id,
            reason: "session_ended".into(),
        },
    );
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn rdp_connect(
    state: tauri::State<'_, RdpState>,
    app: AppHandle,
    config: RdpConfig,
) -> Result<RdpConnectResult, RdpError> {
    validate_config(&config)?;

    let id = Uuid::new_v4().to_string();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(u16, u16), String>>();

    let id_thread = id.clone();
    let connections = Arc::clone(&state.connections);
    let app_clone = app.clone();

    std::thread::spawn(move || {
        rdp_thread(id_thread, config, connections, app_clone, ready_tx);
    });

    match ready_rx.await {
        Ok(Ok((width, height))) => {
            // Emit the event for any other listeners, but the primary consumer
            // reads width/height from the return value to avoid a race.
            let _ = app.emit(
                "rdp:connected",
                RdpConnectedEvent { connection_id: id.clone(), width, height },
            );
            Ok(RdpConnectResult { id, width, height })
        }
        Ok(Err(e)) => Err(RdpError::ConnectionFailed(e)),
        Err(_) => Err(RdpError::ConnectionFailed("session thread exited".into())),
    }
}

#[tauri::command]
pub fn rdp_disconnect(
    state: tauri::State<'_, RdpState>,
    app: AppHandle,
    connection_id: String,
) -> Result<(), RdpError> {
    let conn = state
        .connections
        .lock()
        .unwrap()
        .remove(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    let _ = conn.input_tx.send(RdpInput::Disconnect);

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
    _width: u32,
    _height: u32,
) -> Result<(), RdpError> {
    if !state.connections.lock().unwrap().contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    // Dynamic resize via Display Control DVC is not yet implemented.
    Ok(())
}

#[tauri::command]
pub fn rdp_send_key(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    event: RdpKeyEvent,
) -> Result<(), RdpError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns
        .get(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    let mut flags = if event.pressed { KeyboardFlags::empty() } else { KeyboardFlags::RELEASE };
    if event.extended {
        flags |= KeyboardFlags::EXTENDED;
    }
    let fp = FastPathInputEvent::KeyboardEvent(flags, event.scan_code);
    conn.input_tx
        .send(RdpInput::FastPath(fp))
        .map_err(|_| RdpError::NotFound(connection_id))
}

#[tauri::command]
pub fn rdp_send_mouse(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    event: RdpMouseEvent,
) -> Result<(), RdpError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns
        .get(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    let flags = match (&event.button, &event.event_type) {
        (_, RdpMouseEventType::Move) => PointerFlags::MOVE,
        (RdpMouseButton::Left, RdpMouseEventType::Down) => {
            PointerFlags::LEFT_BUTTON | PointerFlags::DOWN
        }
        (RdpMouseButton::Left, _) => PointerFlags::LEFT_BUTTON,
        (RdpMouseButton::Right, RdpMouseEventType::Down) => {
            PointerFlags::RIGHT_BUTTON | PointerFlags::DOWN
        }
        (RdpMouseButton::Right, _) => PointerFlags::RIGHT_BUTTON,
        (RdpMouseButton::Middle, RdpMouseEventType::Down) => {
            PointerFlags::MIDDLE_BUTTON_OR_WHEEL | PointerFlags::DOWN
        }
        (RdpMouseButton::Middle, _) => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
        _ => PointerFlags::MOVE,
    };

    let fp = FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        x_position: event.x as u16,
        y_position: event.y as u16,
        number_of_wheel_rotation_units: 0,
    });
    conn.input_tx
        .send(RdpInput::FastPath(fp))
        .map_err(|_| RdpError::NotFound(connection_id))
}

#[tauri::command]
pub fn rdp_send_ctrl_alt_del(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
) -> Result<(), RdpError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns
        .get(&connection_id)
        .ok_or_else(|| RdpError::NotFound(connection_id.clone()))?;

    // Scan codes: Ctrl=0x1d, Alt=0x38, Delete=0x53 (extended)
    let sequence = [
        (KeyboardFlags::empty(), 0x1d_u8),
        (KeyboardFlags::empty(), 0x38_u8),
        (KeyboardFlags::EXTENDED, 0x53_u8),
        (KeyboardFlags::EXTENDED | KeyboardFlags::RELEASE, 0x53_u8),
        (KeyboardFlags::RELEASE, 0x38_u8),
        (KeyboardFlags::RELEASE, 0x1d_u8),
    ];
    for (flags, key_code) in sequence {
        conn.input_tx
            .send(RdpInput::FastPath(FastPathInputEvent::KeyboardEvent(
                flags, key_code,
            )))
            .map_err(|_| RdpError::NotFound(connection_id.clone()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn rdp_clipboard_sync(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    _data: RdpClipboardData,
) -> Result<(), RdpError> {
    if !state.connections.lock().unwrap().contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    // Clipboard via CLIPRDR DVC is not yet wired.
    Ok(())
}

#[tauri::command]
pub fn rdp_screenshot(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
) -> Result<String, RdpError> {
    if !state.connections.lock().unwrap().contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    Ok(String::new())
}

#[tauri::command]
pub fn rdp_list_connections(
    state: tauri::State<'_, RdpState>,
) -> Result<Vec<RdpConnectionInfo>, RdpError> {
    let conns = state.connections.lock().unwrap();
    Ok(conns
        .iter()
        .map(|(id, c)| RdpConnectionInfo {
            id: id.clone(),
            host: c.host.clone(),
            port: c.port,
            username: c.username.clone(),
            status: RdpConnectionStatus::Connected,
            width: u32::from(c.width),
            height: u32::from(c.height),
            connected_at: None,
        })
        .collect())
}

#[tauri::command]
pub fn rdp_configure_redirection(
    state: tauri::State<'_, RdpState>,
    connection_id: String,
    _config: RdpRedirectionConfig,
) -> Result<(), RdpError> {
    if !state.connections.lock().unwrap().contains_key(&connection_id) {
        return Err(RdpError::NotFound(connection_id));
    }
    Ok(())
}

// ── Recording stubs ─────────────────────────────────────────────────────

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
    _format: RdpRecordingFormat,
) -> Result<(), RdpError> {
    if !state.connections.lock().unwrap().contains_key(&conn_id) {
        return Err(RdpError::NotFound(conn_id));
    }
    if output_path.is_empty() {
        return Err(RdpError::InvalidConfig("output_path cannot be empty".into()));
    }
    Ok(())
}

#[tauri::command]
pub fn rdp_stop_recording(
    state: tauri::State<'_, RdpState>,
    conn_id: String,
) -> Result<String, RdpError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns
        .get(&conn_id)
        .ok_or_else(|| RdpError::NotFound(conn_id.clone()))?;
    Ok(format!("recording_{}.mp4", conn.host))
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> RdpConfig {
        RdpConfig {
            host: "192.168.1.100".into(),
            port: 3389,
            username: "user".into(),
            password: "pass".into(),
            credential_ref: None,
            domain: None,
            nla_enabled: false,
            tls_required: true,
            width: None,
            height: None,
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

    #[test]
    fn test_validate_empty_host() {
        let mut c = make_config();
        c.host = String::new();
        assert!(validate_config(&c).is_err());
    }

    #[test]
    fn test_validate_zero_port() {
        let mut c = make_config();
        c.port = 0;
        assert!(validate_config(&c).is_err());
    }

    #[test]
    fn test_validate_empty_username() {
        let mut c = make_config();
        c.username = String::new();
        assert!(validate_config(&c).is_err());
    }

    #[test]
    fn test_validate_ok() {
        assert!(validate_config(&make_config()).is_ok());
    }

    #[test]
    fn test_state_starts_empty() {
        let s = RdpState::new();
        assert!(s.connections.lock().unwrap().is_empty());
    }

    #[test]
    fn test_extract_rect_rgba_size() {
        let img = DecodedImage::new(PixelFormat::RgbA32, 8, 8);
        let rect = InclusiveRectangle {
            left: 2,
            top: 2,
            right: 5,  // width = 4
            bottom: 5, // height = 4
        };
        let pixels = extract_rect_rgba(&img, &rect);
        assert_eq!(pixels.len(), 4 * 4 * 4); // 4x4 rect × 4 bpp
    }
}

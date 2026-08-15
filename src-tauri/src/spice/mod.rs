/// SPICE console viewer, built on the `spice-client` crate (GPL-3.0 —
/// compatible with this project's AGPL-3.0 license via the GPLv3/AGPLv3
/// "bridge" provision in section 13 of both licenses) for wire-protocol
/// parsing, RSA-ticket auth crypto, and image decoding, plus a small
/// amount of orchestration this module owns directly:
///
/// - `spice-client` 0.2.0's public API has no way to supply a ticket
///   password on its native (non-WASM) connection path: `MainChannel::new`
///   always performs the link handshake with no password before the
///   caller gets a chance to set one, and `MainChannel`'s fields are
///   private, so there's no way to inject a pre-authenticated connection
///   either. Verified by reading `channels/main.rs` and
///   `channels/connection.rs` directly (not from docs). `ChannelConnection`
///   — the lower-level connection type every channel wraps — IS fully
///   public, and its own `handshake()` already implements the complete
///   RSA-OAEP ticket exchange keyed off `self.password`. So
///   `AuthedMainChannel` below re-implements only the thin post-handshake
///   orchestration the crate's own `MainChannel` does internally
///   (SPICE_MSG_MAIN_INIT, SPICE_MSG_MAIN_CHANNELS_LIST, ping/pong
///   keepalive), reusing the crate's own public protocol types
///   (`SpiceMsgMainInit` via binrw, `MainChannelMessage`, ...) so the
///   message *parsing* itself isn't reimplemented — only the orchestration
///   needed to run it with a password.
/// - Display/Inputs channels don't need this treatment: per the SPICE
///   protocol, only the Main channel performs the ticket exchange —
///   subsequent channels authenticate via the session's connection_id,
///   which `DisplayChannel::new_with_connection_id` /
///   `InputsChannel::new_with_connection_id` already support directly.
/// - `spice_make_scancode` is ported from spice-gtk's own reference
///   implementation (gtk/spice-util.c) since the crate's
///   `send_key_down`/`send_key_up` send whatever raw u32 scancode the
///   caller passes without applying SPICE's extended-key/key-release
///   encoding itself.
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;

use spice_client::channels::connection::ChannelConnection;
use spice_client::channels::{DisplayChannel, InputsChannel};
use spice_client::{
    ChannelType, DisplaySurface, MainChannelMessage, MouseButton as SpiceMouseButton, SpiceDataHeader, SpiceError,
    SpiceMsgMainInit, SPICE_MSGC_MAIN_ATTACH_CHANNELS, SPICE_MSGC_PONG, SPICE_MSG_DISCONNECTING,
    SPICE_MSG_MAIN_CHANNELS_LIST, SPICE_MSG_PING,
};

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SpiceConsoleError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl Serialize for SpiceConsoleError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<SpiceError> for SpiceConsoleError {
    fn from(e: SpiceError) -> Self {
        SpiceConsoleError::ConnectionFailed(e.to_string())
    }
}

impl From<std::io::Error> for SpiceConsoleError {
    fn from(e: std::io::Error) -> Self {
        SpiceConsoleError::ConnectionFailed(e.to_string())
    }
}

// ── Keyboard scancode conversion (ported from spice-gtk) ────────────────

/// Converts a raw PC/AT (XT set 1) scancode into the wire value SPICE
/// expects: for extended keys (represented here, like spice-gtk's own
/// public API, as `0x100 | base` — i.e. the 0xe0 prefix byte dropped and
/// 0x100 OR'd in), the extended marker moves into the low byte (0xe0) with
/// the real code shifted into the high byte; on key-release, 0x80 (or
/// 0x8000 for extended) is OR'd in. Ported verbatim from spice-gtk's
/// `spice_make_scancode()` (gtk/spice-util.c) since spice-client 0.2.0
/// sends whatever raw value the caller passes without this transform.
fn spice_make_scancode(scancode: u32, release: bool) -> u32 {
    if release {
        if scancode < 0x100 {
            scancode | 0x80
        } else {
            0x80e0 | ((scancode - 0x100) << 8)
        }
    } else if scancode < 0x100 {
        scancode
    } else {
        0xe0 | ((scancode - 0x100) << 8)
    }
}

// ── Thin authenticated Main-channel orchestration ────────────────────────

struct AuthedMainChannel {
    connection: ChannelConnection,
    session_id: Option<u32>,
}

impl AuthedMainChannel {
    async fn connect(host: &str, port: u16, password: Option<String>) -> Result<Self, SpiceError> {
        let mut connection = ChannelConnection::new(host, port, ChannelType::Main, 0).await?;
        if let Some(pw) = password {
            connection.set_password(pw);
        }
        connection.handshake().await?;
        Ok(Self { connection, session_id: None })
    }

    /// Handles messages common to every channel (ping keepalive,
    /// server-initiated disconnect). Returns true if the message was fully
    /// handled and the caller should keep waiting for its own message.
    async fn handle_common(&mut self, header: &SpiceDataHeader, data: &[u8]) -> Result<bool, SpiceError> {
        match header.msg_type {
            t if t == SPICE_MSG_PING => {
                if data.len() >= 4 {
                    let mut pong = Vec::with_capacity(12);
                    pong.extend_from_slice(&data[0..4]);
                    pong.extend_from_slice(data.get(4..12).unwrap_or(&[0u8; 8]));
                    self.connection.send_message(SPICE_MSGC_PONG, &pong).await?;
                }
                Ok(true)
            }
            t if t == SPICE_MSG_DISCONNECTING => Err(SpiceError::ConnectionClosed),
            _ => Ok(false),
        }
    }

    async fn initialize(&mut self) -> Result<(), SpiceError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(SpiceError::Protocol("timed out waiting for SPICE_MSG_MAIN_INIT".into()));
            }
            let (header, data) = self.connection.read_message().await?;
            if self.handle_common(&header, &data).await? {
                continue;
            }
            if header.msg_type == MainChannelMessage::Init as u16 {
                use binrw::BinRead;
                let mut cursor = std::io::Cursor::new(&data);
                let init = SpiceMsgMainInit::read(&mut cursor)
                    .map_err(|e| SpiceError::Protocol(format!("failed to parse SPICE_MSG_MAIN_INIT: {e}")))?;
                self.session_id = Some(init.session_id);
                self.connection.send_message(SPICE_MSGC_MAIN_ATTACH_CHANNELS, &[]).await?;
                return Ok(());
            }
            // Servers may send other messages before Init; keep waiting.
        }
    }

    async fn get_channels_list(&mut self) -> Result<Vec<(ChannelType, u8)>, SpiceError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(SpiceError::Protocol("timed out waiting for SPICE_MSG_MAIN_CHANNELS_LIST".into()));
            }
            let (header, data) = self.connection.read_message().await?;
            if self.handle_common(&header, &data).await? {
                continue;
            }
            if header.msg_type == SPICE_MSG_MAIN_CHANNELS_LIST {
                return Ok(parse_channels_list(&data));
            }
        }
    }

    /// Drains further main-channel traffic (pings, notifications) so the
    /// TCP stream doesn't stall; runs until disconnect or error.
    async fn run(&mut self) -> Result<(), SpiceError> {
        loop {
            let (header, data) = self.connection.read_message().await?;
            self.handle_common(&header, &data).await?;
        }
    }
}

fn parse_channels_list(data: &[u8]) -> Vec<(ChannelType, u8)> {
    let mut channels = Vec::new();
    if data.len() >= 4 {
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut offset = 4;
        for _ in 0..count {
            if offset + 1 >= data.len() {
                break;
            }
            channels.push((ChannelType::from(data[offset]), data[offset + 1]));
            offset += 2;
        }
    }
    channels
}

// ── Session state & Tauri commands ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiceConsoleConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpiceConnectResult {
    pub id: String,
    pub width: u32,
    pub height: u32,
}

enum SpiceInput {
    KeyDown(u32),
    KeyUp(u32),
    MouseMove { x: i32, y: i32 },
    MouseButton { button: u8, pressed: bool },
}

struct SpiceConn {
    input_tx: mpsc::UnboundedSender<SpiceInput>,
    session_handle: JoinHandle<()>,
}

pub struct SpiceState {
    connections: Mutex<HashMap<String, SpiceConn>>,
}

impl SpiceState {
    pub fn new() -> Self {
        Self { connections: Mutex::new(HashMap::new()) }
    }
}

#[derive(Clone, Serialize)]
struct SpiceFrameEvent {
    connection_id: String,
    width: u32,
    height: u32,
    data_base64: String,
}

#[derive(Clone, Serialize)]
struct SpiceDisconnectedEvent {
    connection_id: String,
    reason: String,
}

type FrameSnapshot = Option<(u32, u32, Vec<u8>)>;

#[tauri::command]
pub async fn spice_connect(
    config: SpiceConsoleConfig,
    state: tauri::State<'_, SpiceState>,
    app: AppHandle,
) -> Result<SpiceConnectResult, SpiceConsoleError> {
    let mut main_channel = AuthedMainChannel::connect(&config.host, config.port, config.password.clone()).await?;
    main_channel.initialize().await?;
    let session_id = main_channel.session_id;
    let channels = main_channel.get_channels_list().await?;

    let display_id = channels
        .iter()
        .find(|(t, _)| *t == ChannelType::Display)
        .map(|(_, id)| *id)
        .ok_or_else(|| SpiceConsoleError::Protocol("server did not offer a Display channel".into()))?;
    let mut display_channel =
        DisplayChannel::new_with_connection_id(&config.host, config.port, display_id, session_id).await?;

    let inputs_id = channels.iter().find(|(t, _)| *t == ChannelType::Inputs).map(|(_, id)| *id);
    let inputs_channel = match inputs_id {
        Some(id) => Some(InputsChannel::new_with_connection_id(&config.host, config.port, id, session_id).await?),
        None => None,
    };

    let (frame_tx, frame_rx) = watch::channel::<FrameSnapshot>(None);
    let mut frame_rx_for_wait = frame_rx.clone();
    display_channel.set_update_callback(move |surface: &DisplaySurface| {
        let _ = frame_tx.send(Some((surface.width, surface.height, surface.data.clone())));
    });

    // Wait briefly for the first frame so width/height can be returned
    // synchronously, matching the VNC connect command's contract (avoids
    // an event-before-invoke race on the frontend).
    let (initial_width, initial_height) = match tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if frame_rx_for_wait.changed().await.is_err() {
                return None;
            }
            if let Some((w, h, _)) = frame_rx_for_wait.borrow().clone() {
                return Some((w, h));
            }
        }
    })
    .await
    {
        Ok(Some((w, h))) => (w, h),
        _ => (1024, 768),
    };

    let id = Uuid::new_v4().to_string();
    let (input_tx, input_rx) = mpsc::unbounded_channel::<SpiceInput>();

    let app_for_session = app.clone();
    let id_for_session = id.clone();
    let session_handle = tokio::spawn(async move {
        run_spice_session(id_for_session, app_for_session, main_channel, display_channel, inputs_channel, frame_rx, input_rx).await;
    });

    state.connections.lock().unwrap().insert(id.clone(), SpiceConn { input_tx, session_handle });

    Ok(SpiceConnectResult { id, width: initial_width, height: initial_height })
}

async fn run_spice_session(
    id: String,
    app: AppHandle,
    mut main_channel: AuthedMainChannel,
    mut display_channel: DisplayChannel,
    mut inputs_channel: Option<InputsChannel>,
    mut frame_rx: watch::Receiver<FrameSnapshot>,
    mut input_rx: mpsc::UnboundedReceiver<SpiceInput>,
) {
    // Frame emission, throttled to ~30fps: the update callback fires on
    // every server draw (which can be very frequent during motion/video),
    // but each frame is the *entire* current surface, so emitting on every
    // callback would flood the IPC channel. The watch channel naturally
    // coalesces to "latest surface only" between ticks.
    let app_for_frames = app.clone();
    let id_for_frames = id.clone();
    let frame_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(33));
        loop {
            interval.tick().await;
            if frame_rx.has_changed().unwrap_or(false) {
                if let Some((width, height, data)) = frame_rx.borrow_and_update().clone() {
                    let _ = app_for_frames.emit(
                        "spice:frame",
                        SpiceFrameEvent {
                            connection_id: id_for_frames.clone(),
                            width,
                            height,
                            data_base64: base64::engine::general_purpose::STANDARD.encode(&data),
                        },
                    );
                }
            }
        }
    });

    let input_task = tokio::spawn(async move {
        while let Some(input) = input_rx.recv().await {
            let Some(inputs) = inputs_channel.as_mut() else { continue };
            let result = match input {
                SpiceInput::KeyDown(sc) => inputs.send_key_down(spice_make_scancode(sc, false)).await,
                SpiceInput::KeyUp(sc) => inputs.send_key_up(spice_make_scancode(sc, true)).await,
                SpiceInput::MouseMove { x, y } => inputs.send_mouse_motion(x, y).await,
                SpiceInput::MouseButton { button, pressed } => {
                    let b = match button {
                        0 => SpiceMouseButton::Left,
                        1 => SpiceMouseButton::Middle,
                        2 => SpiceMouseButton::Right,
                        3 => SpiceMouseButton::WheelUp,
                        _ => SpiceMouseButton::WheelDown,
                    };
                    inputs.send_mouse_button(b, pressed).await
                }
            };
            if result.is_err() {
                break;
            }
        }
    });

    let main_run = main_channel.run();
    let display_run = display_channel.run();
    tokio::pin!(main_run);
    tokio::pin!(display_run);
    let reason = tokio::select! {
        r = &mut main_run => format!("main channel closed: {}", format_run_result(r)),
        r = &mut display_run => format!("display channel closed: {}", format_run_result(r)),
    };

    frame_task.abort();
    input_task.abort();

    let _ = app.emit("spice:disconnected", SpiceDisconnectedEvent { connection_id: id, reason });
}

fn format_run_result(r: Result<(), SpiceError>) -> String {
    match r {
        Ok(()) => "closed".to_string(),
        Err(e) => e.to_string(),
    }
}

#[tauri::command]
pub fn spice_disconnect(connection_id: String, state: tauri::State<'_, SpiceState>) -> Result<(), SpiceConsoleError> {
    let conn = state
        .connections
        .lock()
        .unwrap()
        .remove(&connection_id)
        .ok_or(SpiceConsoleError::NotFound(connection_id))?;
    conn.session_handle.abort();
    Ok(())
}

#[tauri::command]
pub fn spice_send_key(
    connection_id: String,
    scancode: u32,
    pressed: bool,
    state: tauri::State<'_, SpiceState>,
) -> Result<(), SpiceConsoleError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| SpiceConsoleError::NotFound(connection_id.clone()))?;
    let msg = if pressed { SpiceInput::KeyDown(scancode) } else { SpiceInput::KeyUp(scancode) };
    conn.input_tx.send(msg).map_err(|_| SpiceConsoleError::Protocol("input channel closed".into()))
}

#[tauri::command]
pub fn spice_send_mouse_move(
    connection_id: String,
    x: i32,
    y: i32,
    state: tauri::State<'_, SpiceState>,
) -> Result<(), SpiceConsoleError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| SpiceConsoleError::NotFound(connection_id.clone()))?;
    conn.input_tx
        .send(SpiceInput::MouseMove { x, y })
        .map_err(|_| SpiceConsoleError::Protocol("input channel closed".into()))
}

#[tauri::command]
pub fn spice_send_mouse_button(
    connection_id: String,
    button: u8,
    pressed: bool,
    state: tauri::State<'_, SpiceState>,
) -> Result<(), SpiceConsoleError> {
    let conns = state.connections.lock().unwrap();
    let conn = conns.get(&connection_id).ok_or_else(|| SpiceConsoleError::NotFound(connection_id.clone()))?;
    conn.input_tx
        .send(SpiceInput::MouseButton { button, pressed })
        .map_err(|_| SpiceConsoleError::Protocol("input channel closed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spice_make_scancode_normal_key_press_is_unmodified() {
        // 'A' key = 0x1e per PC/AT set 1.
        assert_eq!(spice_make_scancode(0x1e, false), 0x1e);
    }

    #[test]
    fn spice_make_scancode_normal_key_release_sets_0x80() {
        assert_eq!(spice_make_scancode(0x1e, true), 0x1e | 0x80);
    }

    #[test]
    fn spice_make_scancode_extended_key_press_moves_code_to_high_byte() {
        // Right Ctrl: raw XT is 0xe0 0x1d, represented here as 0x100 | 0x1d = 0x11d.
        assert_eq!(spice_make_scancode(0x11d, false), 0x1de0);
    }

    #[test]
    fn spice_make_scancode_extended_key_release_matches_spice_gtk_reference() {
        // Verified against spice-gtk's spice_make_scancode(): 0x80e0 | ((0x11d - 0x100) << 8).
        assert_eq!(spice_make_scancode(0x11d, true), 0x9de0);
    }

    #[test]
    fn parse_channels_list_reads_count_and_type_id_pairs() {
        let mut data = vec![2, 0, 0, 0]; // count = 2
        data.push(ChannelType::Display as u8);
        data.push(0); // channel id 0
        data.push(ChannelType::Inputs as u8);
        data.push(1); // channel id 1
        let channels = parse_channels_list(&data);
        assert_eq!(channels, vec![(ChannelType::Display, 0), (ChannelType::Inputs, 1)]);
    }

    #[test]
    fn parse_channels_list_ignores_a_truncated_trailing_entry() {
        let mut data = vec![2, 0, 0, 0]; // claims 2 entries
        data.push(ChannelType::Display as u8);
        data.push(0); // only one full entry actually present
        let channels = parse_channels_list(&data);
        assert_eq!(channels, vec![(ChannelType::Display, 0)]);
    }

    #[test]
    fn parse_channels_list_handles_empty_data() {
        assert_eq!(parse_channels_list(&[]), Vec::new());
    }
}

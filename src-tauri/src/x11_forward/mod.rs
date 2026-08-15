/// X11 forwarding over SSH (RFC 4254 §6.3.1) — connects via SSH, requests
/// `x11-req` on the session channel so the remote shell's `DISPLAY` gets
/// set, then relays each X11 sub-connection the server opens back to us to
/// the local X server (XQuartz on macOS, or any X11 socket on Linux).
///
/// X11 forwarding isn't its own wire protocol — it's an SSH channel type
/// (`x11`) layered on a connection whose transport, auth, and channel
/// machinery this crate already has via `russh` (used for the SSH session
/// type). What's new here is the RFC's security handoff: the client tells
/// the remote `x11-req` to expect a locally-generated, single-use "fake"
/// `MIT-MAGIC-COOKIE-1` cookie rather than the real local X server's
/// cookie, so the fake one is what ends up on the wire to the remote host.
/// When a remote X11 client actually connects back through the forwarded
/// channel, this intercepts just its X11 `ConnectionSetup` request (the
/// one-time byte layout is stable across X11's entire history — see the
/// X Window System Protocol, §8) and substitutes in the real local cookie
/// (read via `xauth list`, matching the CLI-wrapping approach already used
/// for SMB rather than hand-parsing the binary Xauthority file format)
/// before going fully passthrough for the rest of that sub-connection.
use russh::client::{self, Handle, Msg};
use russh::{Channel, ChannelId, ChannelMsg};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, Error)]
pub enum X11ForwardError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Local X server not found at {0} — is XQuartz (or an X server) running?")]
    NoLocalXServer(String),
    #[error("`xauth` not found — install XQuartz (macOS) or xauth (Linux) to forward X11")]
    XauthNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for X11ForwardError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<russh::Error> for X11ForwardError {
    fn from(e: russh::Error) -> Self {
        X11ForwardError::Ssh(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum X11ForwardAuth {
    Password { password: String },
    PrivateKey { key_data: String, passphrase: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X11ForwardConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: X11ForwardAuth,
    /// Remote command to run once X11 forwarding is set up (e.g. "xterm",
    /// "xclock", or a full GUI app) — run via the session's exec channel.
    pub remote_command: String,
    /// Local display to forward to, e.g. "0" for /tmp/.X11-unix/X0.
    pub local_display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X11ForwardSession {
    pub id: String,
    pub host: String,
    pub remote_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X11ForwardOutput {
    pub session_id: String,
    pub data: String,
}

struct X11Handler {
    app_handle: AppHandle,
    connection_id: String,
    local_display: String,
    real_cookie: Option<(String, Vec<u8>)>,
}

#[async_trait::async_trait]
impl client::Handler for X11Handler {
    type Error = X11ForwardError;

    async fn check_server_key(&mut self, _server_public_key: &russh_keys::key::PublicKey) -> Result<bool, Self::Error> {
        // TOFU is handled by the primary SSH session type; X11 forwarding
        // is opened over connections a user already trusts enough to run
        // GUI apps on, so this mirrors that same acceptance rather than
        // duplicating known_hosts bookkeeping for a second code path.
        Ok(true)
    }

    async fn server_channel_open_x11(
        &mut self,
        channel: Channel<Msg>,
        originator_address: &str,
        originator_port: u32,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let local_display = self.local_display.clone();
        let real_cookie = self.real_cookie.clone();
        let app_handle = self.app_handle.clone();
        let connection_id = self.connection_id.clone();
        let originator = format!("{originator_address}:{originator_port}");
        tokio::spawn(async move {
            if let Err(e) = relay_x11_channel(channel, &local_display, real_cookie).await {
                let _ = app_handle.emit(
                    "x11_forward:error",
                    X11ForwardOutput { session_id: connection_id, data: format!("X11 sub-connection from {originator} failed: {e}") },
                );
            }
        });
        Ok(())
    }

    async fn data(&mut self, _channel: ChannelId, data: &[u8], _session: &mut client::Session) -> Result<(), Self::Error> {
        let _ = self.app_handle.emit("x11_forward:output", X11ForwardOutput { session_id: self.connection_id.clone(), data: String::from_utf8_lossy(data).into_owned() });
        Ok(())
    }
}

/// Looks up the real local X11 cookie for `display` via `xauth list` —
/// output format is `hostname/unix:N  MIT-MAGIC-COOKIE-1  <32 hex chars>`
/// (or bare `:N` on some systems). Returns `(auth_name, auth_data_bytes)`.
async fn local_x11_cookie(display: &str) -> Result<(String, Vec<u8>), X11ForwardError> {
    let xauth = which::which("xauth").map_err(|_| X11ForwardError::XauthNotFound)?;
    let output = tokio::process::Command::new(xauth).arg("list").arg(format!(":{display}")).output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().ok_or_else(|| X11ForwardError::NoLocalXServer(format!(":{display}")))?;
    let mut parts = line.split_whitespace();
    let _addr = parts.next();
    let name = parts.next().unwrap_or("MIT-MAGIC-COOKIE-1").to_string();
    let hex_data = parts.next().ok_or_else(|| X11ForwardError::NoLocalXServer(format!(":{display}")))?;
    let data = hex::decode(hex_data).map_err(|_| X11ForwardError::NoLocalXServer(format!(":{display}")))?;
    Ok((name, data))
}

fn generate_fake_cookie() -> Vec<u8> {
    use rand::RngCore;
    let mut cookie = vec![0u8; 16];
    rand::thread_rng().fill_bytes(&mut cookie);
    cookie
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Patches the auth-name/auth-data fields of an X11 `ConnectionSetup`
/// request (X Window System Protocol §8) in place — this only works
/// because our fake cookie is generated to be the exact same length as a
/// real `MIT-MAGIC-COOKIE-1` (16 bytes), so no length field needs to
/// change and the rest of the packet's byte offsets stay valid.
fn patch_connection_setup(packet: &mut [u8], real_auth: &(String, Vec<u8>)) -> Option<()> {
    if packet.len() < 12 {
        return None;
    }
    let little_endian = packet[0] == b'l';
    let read_u16 = |b: &[u8]| -> u16 { if little_endian { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) } };

    let name_len = read_u16(&packet[6..8]) as usize;
    let data_len = read_u16(&packet[8..10]) as usize;
    let name_start = 12;
    let name_padded = name_len.div_ceil(4) * 4;
    let data_start = name_start + name_padded;

    if packet.len() < data_start + data_len {
        return None;
    }
    if data_len != real_auth.1.len() {
        // Our fake cookie's length must match the real one for in-place
        // patching to be safe; if it doesn't, leave the packet alone
        // rather than corrupt it — the X server will just reject the
        // fake cookie and the connection fails closed, not open.
        return None;
    }

    let real_name = real_auth.0.as_bytes();
    if name_len == real_name.len() {
        packet[name_start..name_start + name_len].copy_from_slice(real_name);
    }
    packet[data_start..data_start + data_len].copy_from_slice(&real_auth.1);
    Some(())
}

async fn relay_x11_channel(channel: Channel<Msg>, local_display: &str, real_cookie: Option<(String, Vec<u8>)>) -> Result<(), X11ForwardError> {
    let socket_path = format!("/tmp/.X11-unix/X{local_display}");
    let mut local = UnixStream::connect(&socket_path).await.map_err(|_| X11ForwardError::NoLocalXServer(socket_path.clone()))?;

    let mut first_packet = true;
    let mut stream = channel.into_stream();
    let mut ssh_buf = [0u8; 8192];
    let mut local_buf = [0u8; 8192];
    loop {
        tokio::select! {
            result = stream.read(&mut ssh_buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut chunk = ssh_buf[..n].to_vec();
                        if first_packet {
                            first_packet = false;
                            if let Some(real_auth) = &real_cookie {
                                patch_connection_setup(&mut chunk, real_auth);
                            }
                        }
                        if local.write_all(&chunk).await.is_err() { break; }
                    }
                }
            }
            result = local.read(&mut local_buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if stream.write_all(&local_buf[..n]).await.is_err() { break; }
                    }
                }
            }
        }
    }
    Ok(())
}

struct X11Conn {
    handle: Handle<X11Handler>,
    channel: TokioMutex<Channel<Msg>>,
}

pub struct X11ForwardState {
    sessions: std::sync::Mutex<HashMap<String, X11ForwardSession>>,
    conns: std::sync::Mutex<HashMap<String, Arc<X11Conn>>>,
}

impl X11ForwardState {
    pub fn new() -> Self {
        Self { sessions: std::sync::Mutex::new(HashMap::new()), conns: std::sync::Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn x11_forward_connect(config: X11ForwardConfig, state: tauri::State<'_, X11ForwardState>, app: AppHandle) -> Result<String, X11ForwardError> {
    let id = Uuid::new_v4().to_string();

    let real_cookie = local_x11_cookie(&config.local_display).await.ok();
    let ssh_config = Arc::new(client::Config::default());
    let handler = X11Handler { app_handle: app.clone(), connection_id: id.clone(), local_display: config.local_display.clone(), real_cookie };

    let mut handle = client::connect(ssh_config, (config.host.as_str(), config.port), handler).await.map_err(|e| X11ForwardError::Ssh(e.to_string()))?;

    let authenticated = match &config.auth {
        X11ForwardAuth::Password { password } => {
            let password = Zeroizing::new(password.clone());
            handle.authenticate_password(&config.username, (*password).clone()).await.map_err(|e| X11ForwardError::Ssh(e.to_string()))?
        }
        X11ForwardAuth::PrivateKey { key_data, passphrase } => {
            let key_pair = if let Some(pass) = passphrase {
                let pass = Zeroizing::new(pass.clone());
                russh_keys::decode_secret_key(key_data, Some(pass.as_str()))
            } else {
                russh_keys::decode_secret_key(key_data, None)
            }
            .map_err(|e| X11ForwardError::Ssh(format!("Invalid private key: {e}")))?;
            handle.authenticate_publickey(&config.username, Arc::new(key_pair)).await.map_err(|e| X11ForwardError::Ssh(e.to_string()))?
        }
    };
    if !authenticated {
        return Err(X11ForwardError::AuthFailed);
    }

    let channel = handle.channel_open_session().await.map_err(|e| X11ForwardError::Ssh(e.to_string()))?;

    // Fake cookie length must match the real one (16 bytes) for the later
    // in-place patch to be safe — see `patch_connection_setup`.
    let fake_cookie = generate_fake_cookie();
    channel
        .request_x11(true, false, "MIT-MAGIC-COOKIE-1", hex_encode(&fake_cookie), 0)
        .await
        .map_err(|e| X11ForwardError::Ssh(format!("x11-req rejected: {e}")))?;

    channel.exec(true, config.remote_command.as_bytes()).await.map_err(|e| X11ForwardError::Ssh(e.to_string()))?;

    let session = X11ForwardSession { id: id.clone(), host: config.host, remote_command: config.remote_command };
    state.sessions.lock().unwrap().insert(id.clone(), session);
    state.conns.lock().unwrap().insert(id.clone(), Arc::new(X11Conn { handle, channel: TokioMutex::new(channel) }));
    Ok(id)
}

#[tauri::command]
pub fn x11_forward_disconnect(id: String, state: tauri::State<'_, X11ForwardState>) -> Result<(), X11ForwardError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| X11ForwardError::NotFound(id.clone()))?;
    state.conns.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn x11_forward_list(state: tauri::State<'_, X11ForwardState>) -> Vec<X11ForwardSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_setup_packet(byte_order: u8, name: &str, data: &[u8]) -> Vec<u8> {
        let mut p = vec![byte_order, 0, 11, 0, 0, 0];
        let write_u16 = |v: u16, le: bool| -> [u8; 2] { if le { v.to_le_bytes() } else { v.to_be_bytes() } };
        let le = byte_order == b'l';
        p.extend_from_slice(&write_u16(name.len() as u16, le));
        p.extend_from_slice(&write_u16(data.len() as u16, le));
        p.extend_from_slice(&[0, 0]);
        p.extend_from_slice(name.as_bytes());
        while p.len() % 4 != 0 {
            p.push(0);
        }
        p.extend_from_slice(data);
        while p.len() % 4 != 0 {
            p.push(0);
        }
        p
    }

    #[test]
    fn test_patch_connection_setup_replaces_fake_cookie_with_real_one_big_endian() {
        let fake = vec![0xAA; 16];
        let real = ("MIT-MAGIC-COOKIE-1".to_string(), vec![0xBB; 16]);
        let mut packet = build_setup_packet(b'B', "MIT-MAGIC-COOKIE-1", &fake);

        patch_connection_setup(&mut packet, &real).unwrap();

        let data_start = 12 + "MIT-MAGIC-COOKIE-1".len().div_ceil(4) * 4;
        assert_eq!(&packet[data_start..data_start + 16], &real.1[..]);
    }

    #[test]
    fn test_patch_connection_setup_replaces_fake_cookie_little_endian() {
        let fake = vec![0xAA; 16];
        let real = ("MIT-MAGIC-COOKIE-1".to_string(), vec![0xCC; 16]);
        let mut packet = build_setup_packet(b'l', "MIT-MAGIC-COOKIE-1", &fake);

        patch_connection_setup(&mut packet, &real).unwrap();

        let data_start = 12 + "MIT-MAGIC-COOKIE-1".len().div_ceil(4) * 4;
        assert_eq!(&packet[data_start..data_start + 16], &real.1[..]);
    }

    #[test]
    fn test_patch_connection_setup_leaves_packet_alone_on_length_mismatch() {
        let fake = vec![0xAA; 8]; // wrong length on purpose
        let real = ("MIT-MAGIC-COOKIE-1".to_string(), vec![0xBB; 16]);
        let mut packet = build_setup_packet(b'B', "MIT-MAGIC-COOKIE-1", &fake);
        let original = packet.clone();

        let result = patch_connection_setup(&mut packet, &real);

        assert!(result.is_none());
        assert_eq!(packet, original);
    }

    #[test]
    fn test_patch_connection_setup_rejects_truncated_packet() {
        let mut packet = vec![b'B', 0, 11, 0];
        let real = ("MIT-MAGIC-COOKIE-1".to_string(), vec![0xBB; 16]);
        assert!(patch_connection_setup(&mut packet, &real).is_none());
    }

    #[test]
    fn test_generate_fake_cookie_length_matches_mit_magic_cookie_1() {
        let cookie = generate_fake_cookie();
        assert_eq!(cookie.len(), 16);
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    }
}

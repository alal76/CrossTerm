use async_trait::async_trait;
use russh::client;
use russh::keys::key::PublicKey;
use russh::{ChannelId, ChannelMsg, CryptoVec, Disconnect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex, RwLock};
use uuid::Uuid;
use zeroize::Zeroizing;

#[cfg(unix)]
use tokio::net::UnixStream as TokioUnixStream;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SshError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Channel error: {0}")]
    Channel(String),
    #[error("Key error: {0}")]
    Key(String),
    #[error("Port forward error: {0}")]
    PortForward(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Agent forwarding error: {0}")]
    AgentForward(String),
    #[error("Host key changed for {0} — possible MITM attack")]
    HostKeyChanged(String),
}

impl Serialize for SshError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<russh::Error> for SshError {
    fn from(e: russh::Error) -> Self {
        SshError::ConnectionFailed(e.to_string())
    }
}

impl From<std::io::Error> for SshError {
    fn from(e: std::io::Error) -> Self {
        SshError::Io(e.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SshAuth {
    Password { password: String },
    PrivateKey {
        key_data: String,
        passphrase: Option<String>,
    },
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpHost {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PortForward {
    Local {
        id: String,
        bind_host: String,
        bind_port: u16,
        remote_host: String,
        remote_port: u16,
    },
    Remote {
        id: String,
        bind_host: String,
        bind_port: u16,
        remote_host: String,
        remote_port: u16,
    },
    Dynamic {
        id: String,
        bind_host: String,
        bind_port: u16,
    },
}

impl PortForward {
    pub fn id(&self) -> &str {
        match self {
            PortForward::Local { id, .. } => id,
            PortForward::Remote { id, .. } => id,
            PortForward::Dynamic { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConnectionInfo {
    pub connection_id: String,
    pub session_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub connected_at: String,
    pub port_forwards: Vec<PortForward>,
    pub cipher_algorithm: Option<String>,
    pub kex_algorithm: Option<String>,
    pub latency_ms: Option<u64>,
}

// ── Tauri Event Payloads ────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct SshOutputEvent {
    connection_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct SshDisconnectedEvent {
    connection_id: String,
    reason: String,
}

#[derive(Clone, Serialize)]
struct SshConnectedEvent {
    connection_id: String,
    cipher_algorithm: Option<String>,
    kex_algorithm: Option<String>,
    latency_ms: Option<u64>,
}

#[derive(Clone, Serialize)]
struct SshHostKeyNewEvent {
    host: String,
    port: u16,
    algorithm: String,
    fingerprint: String,
}

#[derive(Clone, Serialize)]
struct SshAuthPromptEvent {
    connection_id: String,
    name: String,
    instructions: String,
    prompts: Vec<AuthPromptInfo>,
}

#[derive(Clone, Serialize)]
struct AuthPromptInfo {
    prompt: String,
    echo: bool,
}

#[derive(Clone, Serialize)]
struct SshConnectLogEvent {
    connection_id: String,
    level: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct SshBannerEvent {
    connection_id: String,
    banner: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SshDiscoverResult {
    pub host: String,
    pub port: u16,
    pub none_auth_accepted: bool,
    pub auth_required: bool,
    pub banner: Option<String>,
}

/// Credential info for auto-save event emitted after successful auth.
#[derive(Clone, Serialize, Deserialize)]
struct SshAuthSuccessEvent {
    connection_id: String,
    host: String,
    port: u16,
    username: String,
    auth_method: String,
}

// ── Internal command channel for the session task ───────────────────────

enum SshCommand {
    Write(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

// ── Client Handler ──────────────────────────────────────────────────────

pub(crate) struct SshClientHandler {
    app_handle: AppHandle,
    connection_id: String,
    host: String,
    port: u16,
    /// Remote forward configs: remote bind -> local target
    #[allow(clippy::type_complexity)]
    remote_forwards: Arc<TokioMutex<HashMap<(String, u32), (String, u16)>>>,
    /// Whether SSH agent forwarding is enabled for this connection.
    agent_enabled: bool,
    /// Active agent forwarding sockets keyed by channel ID (Unix only).
    #[cfg(unix)]
    agent_sockets: HashMap<ChannelId, TokioUnixStream>,
}

#[async_trait]
impl client::Handler for SshClientHandler {
    type Error = SshError;

    async fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self.app_handle.emit(
            "ssh:banner",
            SshBannerEvent {
                connection_id: self.connection_id.clone(),
                banner: banner.to_string(),
            },
        );
        Ok(())
    }

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key.fingerprint();
        let algorithm = server_public_key.name().to_string();
        let host_key = format!("{}:{}", self.host, self.port);

        let known_hosts_path = known_hosts_file_path();

        // Read existing known_hosts
        if known_hosts_path.exists() {
            let file = std::fs::File::open(&known_hosts_path)
                .map_err(|e| SshError::Io(e.to_string()))?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.map_err(|e| SshError::Io(e.to_string()))?;
                let line = line.trim().to_string();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() == 3 && parts[0] == host_key {
                    // Found existing entry
                    if parts[2] == fingerprint {
                        // Match — accept silently
                        return Ok(true);
                    } else {
                        // Key changed — reject
                        return Err(SshError::HostKeyChanged(host_key));
                    }
                }
            }
        }

        // Not found — TOFU: accept and save
        if let Some(parent) = known_hosts_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SshError::Io(e.to_string()))?;
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&known_hosts_path)
            .map_err(|e| SshError::Io(e.to_string()))?;
        writeln!(file, "{} {} {}", host_key, algorithm, fingerprint)
            .map_err(|e| SshError::Io(e.to_string()))?;

        let _ = self.app_handle.emit(
            "ssh:host_key_new",
            SshHostKeyNewEvent {
                host: self.host.clone(),
                port: self.port,
                algorithm,
                fingerprint,
            },
        );

        Ok(true)
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let forwards = self.remote_forwards.lock().await;
        let key = (connected_address.to_string(), connected_port);
        let (local_host, local_port) = forwards
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ("127.0.0.1".to_string(), connected_port as u16));
        drop(forwards);

        let local_addr = format!("{}:{}", local_host, local_port);

        tokio::spawn(async move {
            let Ok(mut tcp_stream) = TcpStream::connect(&local_addr).await else {
                return;
            };
            let (mut tcp_read, mut tcp_write) = tcp_stream.split();
            let mut ch = channel;

            loop {
                tokio::select! {
                    msg = ch.wait() => {
                        match msg {
                            Some(ChannelMsg::Data { data }) => {
                                if tcp_write.write_all(&data).await.is_err() {
                                    break;
                                }
                            }
                            Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
                            _ => {}
                        }
                    }
                    result = async {
                        let mut buf = [0u8; 8192];
                        tcp_read.read(&mut buf).await.map(|n| (n, buf))
                    } => {
                        match result {
                            Ok((0, _)) => break,
                            Ok((n, buf)) => {
                                if ch.data(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Handle agent forwarding channels
        #[cfg(unix)]
        if let Some(socket) = self.agent_sockets.get_mut(&channel) {
            // Forward data to local SSH agent
            socket.write_all(data).await
                .map_err(|e| SshError::AgentForward(e.to_string()))?;

            // Read response: SSH agent protocol uses 4-byte length prefix
            let mut len_buf = [0u8; 4];
            socket.read_exact(&mut len_buf).await
                .map_err(|e| SshError::AgentForward(e.to_string()))?;
            let msg_len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; msg_len];
            socket.read_exact(&mut body).await
                .map_err(|e| SshError::AgentForward(e.to_string()))?;

            let mut response = CryptoVec::new();
            response.extend(&len_buf);
            response.extend(&body);
            session.data(channel, response);
            return Ok(());
        }

        // Shell channel data is handled by the reader task via ch.wait(),
        // and ssh_exec channels handle their own output.  Emitting here
        // would duplicate output and leak exec-channel data (e.g. remote
        // monitoring commands) into the terminal.
        Ok(())
    }

    async fn server_channel_open_agent_forward(
        &mut self,
        channel: ChannelId,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        #[cfg(unix)]
        if self.agent_enabled {
            if let Ok(sock_path) = std::env::var("SSH_AUTH_SOCK") {
                if let Ok(stream) = TokioUnixStream::connect(&sock_path).await {
                    self.agent_sockets.insert(channel, stream);
                }
            }
        }
        Ok(())
    }
}

// ── Connection ──────────────────────────────────────────────────────────

pub(crate) struct SshConnection {
    pub(crate) info: SshConnectionInfo,
    pub(crate) handle: client::Handle<SshClientHandler>,
    /// Handle for the jump host connection, if connected via ProxyJump.
    pub(crate) jump_host_handle: Option<client::Handle<SshClientHandler>>,
    cmd_tx: mpsc::Sender<SshCommand>,
    forward_tasks: HashMap<String, tokio::task::JoinHandle<()>>,
    #[allow(clippy::type_complexity)]
    remote_forwards: Arc<TokioMutex<HashMap<(String, u32), (String, u16)>>>,
    /// Early output buffer: collects data before the frontend terminal mounts.
    /// Shared with the reader task; drained once via `ssh_drain_buffer`.
    output_buffer: Arc<std::sync::Mutex<Option<Vec<String>>>>,
    /// Cached content from the first drain, returned on subsequent drain calls
    /// (React StrictMode may call drain multiple times due to double-mount).
    drained_cache: Arc<std::sync::Mutex<Option<String>>>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SshState {
    pub(crate) connections: Arc<RwLock<HashMap<String, Arc<TokioMutex<SshConnection>>>>>,
    pending_auth_responses: Arc<RwLock<HashMap<String, oneshot::Sender<Vec<String>>>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            pending_auth_responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn known_hosts_file_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("CrossTerm")
        .join("known_hosts")
}

#[allow(clippy::field_reassign_with_default)]
fn build_config(keep_alive_secs: Option<u64>) -> Arc<client::Config> {
    let mut config = client::Config::default();
    config.keepalive_interval = Some(std::time::Duration::from_secs(keep_alive_secs.unwrap_or(30)));
    config.keepalive_max = 3;

    // Enforce secure cipher/kex policy – only modern, audited algorithms
    config.preferred = russh::Preferred {
        kex: std::borrow::Cow::Borrowed(&[
            russh::kex::CURVE25519,
            russh::kex::CURVE25519_PRE_RFC_8731,
            russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
            russh::kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
        ]),
        key: std::borrow::Cow::Borrowed(&[
            russh::keys::key::ED25519,
            russh::keys::key::RSA_SHA2_512,
            russh::keys::key::RSA_SHA2_256,
        ]),
        cipher: std::borrow::Cow::Borrowed(&[
            russh::cipher::CHACHA20_POLY1305,
            russh::cipher::AES_256_GCM,
            russh::cipher::AES_256_CTR,
        ]),
        mac: std::borrow::Cow::Borrowed(&[
            russh::mac::HMAC_SHA256_ETM,
            russh::mac::HMAC_SHA512_ETM,
            russh::mac::HMAC_SHA256,
        ]),
        compression: std::borrow::Cow::Borrowed(&[russh::compression::NONE]),
    };

    Arc::new(config)
}

/// Establish the SSH transport (TCP + handshake) without authenticating.
async fn ssh_connect_transport(
    host: &str,
    port: u16,
    handler: SshClientHandler,
    keep_alive_secs: Option<u64>,
) -> Result<client::Handle<SshClientHandler>, SshError> {
    let config = build_config(keep_alive_secs);
    let addr = format!("{}:{}", host, port);
    client::connect(config, &addr, handler)
        .await
        .map_err(|e| SshError::ConnectionFailed(e.to_string()))
}

/// Attempt a single authentication method. Returns `Ok(true)` on success.
async fn ssh_try_auth(
    handle: &mut client::Handle<SshClientHandler>,
    username: &str,
    auth: &SshAuth,
) -> Result<bool, SshError> {
    match auth {
        SshAuth::Password { password } => {
            let password = Zeroizing::new(password.clone());
            handle
                .authenticate_password(username, (*password).clone())
                .await
                .map_err(|e| SshError::ConnectionFailed(e.to_string()))
        }
        SshAuth::PrivateKey {
            key_data,
            passphrase,
        } => {
            let key_pair = if let Some(pass) = passphrase {
                let pass = Zeroizing::new(pass.clone());
                russh_keys::decode_secret_key(key_data, Some(pass.as_str()))
                    .map_err(|e| SshError::Key(e.to_string()))?
            } else {
                russh_keys::decode_secret_key(key_data, None)
                    .map_err(|e| SshError::Key(e.to_string()))?
            };
            handle
                .authenticate_publickey(username, Arc::new(key_pair))
                .await
                .map_err(|e| SshError::ConnectionFailed(e.to_string()))
        }
        SshAuth::None => {
            handle
                .authenticate_none(username)
                .await
                .map_err(|e| SshError::ConnectionFailed(e.to_string()))
        }
    }
}

/// Drive keyboard-interactive auth, emitting prompt events and waiting for
/// frontend responses via a oneshot channel stored in `pending_responses`.
async fn ssh_keyboard_interactive_auth(
    handle: &mut client::Handle<SshClientHandler>,
    username: &str,
    connection_id: &str,
    app_handle: &AppHandle,
    pending_responses: &Arc<RwLock<HashMap<String, oneshot::Sender<Vec<String>>>>>,
) -> Result<(), SshError> {
    use russh::client::KeyboardInteractiveAuthResponse;

    let mut result = handle
        .authenticate_keyboard_interactive_start(username, None::<String>)
        .await
        .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;

    loop {
        match result {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure => return Err(SshError::AuthFailed),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                if prompts.is_empty() {
                    // Empty prompt round — respond with empty vec and continue
                    result = handle
                        .authenticate_keyboard_interactive_respond(vec![])
                        .await
                        .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;
                    continue;
                }

                let (tx, rx) = oneshot::channel();
                pending_responses
                    .write()
                    .await
                    .insert(connection_id.to_string(), tx);

                let _ = app_handle.emit(
                    "ssh:auth_prompt",
                    SshAuthPromptEvent {
                        connection_id: connection_id.to_string(),
                        name,
                        instructions,
                        prompts: prompts
                            .iter()
                            .map(|p| AuthPromptInfo {
                                prompt: p.prompt.clone(),
                                echo: p.echo,
                            })
                            .collect(),
                    },
                );

                let responses = rx.await.map_err(|_| SshError::AuthFailed)?;

                result = handle
                    .authenticate_keyboard_interactive_respond(responses)
                    .await
                    .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;
            }
        }
    }
}

async fn connect_and_auth(
    host: &str,
    port: u16,
    username: &str,
    auth: &SshAuth,
    handler: SshClientHandler,
    keep_alive_secs: Option<u64>,
) -> Result<client::Handle<SshClientHandler>, SshError> {
    let mut handle = ssh_connect_transport(host, port, handler, keep_alive_secs).await?;
    if !ssh_try_auth(&mut handle, username, auth).await? {
        return Err(SshError::AuthFailed);
    }
    Ok(handle)
}

/// Connect to a target host by tunneling through a jump host (ProxyJump).
///
/// 1. Connects and authenticates to the jump host
/// 2. Opens a direct-tcpip channel from the jump host to the target
/// 3. Runs the SSH handshake over that channel to the target
///
/// Returns `(target_handle, jump_host_handle)`.
#[allow(clippy::too_many_arguments)]
async fn connect_via_jump(
    jump: &JumpHost,
    target_host: &str,
    target_port: u16,
    target_username: &str,
    target_auth: &SshAuth,
    target_handler: SshClientHandler,
    app_handle: &AppHandle,
    connection_id: &str,
    keep_alive_secs: Option<u64>,
) -> Result<(client::Handle<SshClientHandler>, client::Handle<SshClientHandler>), SshError> {
    let jump_handler = SshClientHandler {
        app_handle: app_handle.clone(),
        connection_id: format!("{}-jump", connection_id),
        host: jump.host.clone(),
        port: jump.port,
        remote_forwards: Arc::new(TokioMutex::new(HashMap::new())),
        agent_enabled: false,
        #[cfg(unix)]
        agent_sockets: HashMap::new(),
    };

    let jump_handle = connect_and_auth(
        &jump.host, jump.port, &jump.username, &jump.auth,
        jump_handler, keep_alive_secs,
    ).await.map_err(|e| SshError::ConnectionFailed(
        format!("Jump host connection failed: {}", e)
    ))?;

    // Open direct-tcpip channel through jump host to the actual target
    let channel = jump_handle
        .channel_open_direct_tcpip(target_host, target_port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| SshError::ConnectionFailed(
            format!("Jump host tunnel to {}:{} failed: {}", target_host, target_port, e)
        ))?;

    let stream = channel.into_stream();
    let config = build_config(keep_alive_secs);

    // Run the SSH handshake to the target over the tunneled stream
    let mut target_handle = client::connect_stream(config, stream, target_handler)
        .await
        .map_err(|e| SshError::ConnectionFailed(
            format!("SSH through jump host failed: {}", e)
        ))?;

    // Authenticate to the target host
    let authenticated = ssh_try_auth(&mut target_handle, target_username, target_auth).await?;

    if !authenticated {
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "Target auth failed", "en")
            .await;
        return Err(SshError::AuthFailed);
    }

    Ok((target_handle, jump_handle))
}

// ── Helpers: connection log ─────────────────────────────────────────────

fn emit_connect_log(app_handle: &AppHandle, connection_id: &str, level: &str, message: &str) {
    let _ = app_handle.emit(
        "ssh:connect_log",
        SshConnectLogEvent {
            connection_id: connection_id.to_string(),
            level: level.to_string(),
            message: message.to_string(),
        },
    );
}

// ── Tauri Commands ──────────────────────────────────────────────────────

/// Discover authentication requirements by connecting and performing "none" auth.
#[tauri::command]
pub async fn ssh_discover(
    app_handle: AppHandle,
    host: String,
    port: u16,
    username: String,
) -> Result<SshDiscoverResult, SshError> {
    let connection_id = format!("discover-{}", Uuid::new_v4());

    let handler = SshClientHandler {
        app_handle: app_handle.clone(),
        connection_id: connection_id.clone(),
        host: host.clone(),
        port,
        remote_forwards: Arc::new(TokioMutex::new(HashMap::new())),
        agent_enabled: false,
        #[cfg(unix)]
        agent_sockets: HashMap::new(),
    };

    emit_connect_log(&app_handle, &connection_id, "info", &format!("Connecting to {}:{}…", host, port));

    let mut handle = ssh_connect_transport(&host, port, handler, None).await?;

    emit_connect_log(&app_handle, &connection_id, "info", "Transport established, probing auth methods…");

    let none_accepted = handle
        .authenticate_none(&username)
        .await
        .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;

    let _ = handle.disconnect(Disconnect::ByApplication, "Discovery complete", "en").await;

    let result = SshDiscoverResult {
        host,
        port,
        none_auth_accepted: none_accepted,
        auth_required: !none_accepted,
        banner: None,
    };

    Ok(result)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn ssh_connect(
    app_handle: AppHandle,
    state: tauri::State<'_, SshState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    session_id: String,
    host: String,
    port: u16,
    username: String,
    auth: SshAuth,
    jump_host: Option<JumpHost>,
    agent_forwarding: Option<bool>,
    cols: Option<u32>,
    rows: Option<u32>,
) -> Result<String, SshError> {
    let connection_id = Uuid::new_v4().to_string();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);
    let agent_fwd = agent_forwarding.unwrap_or(false);

    // Start timing for latency measurement
    let connect_start = std::time::Instant::now();

    // Look up session config for keep-alive interval and startup script
    let (keep_alive_secs, startup_script) = {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        if !pid.is_empty() {
            match crate::config::do_session_get(&pid, &session_id) {
                Ok(session) => (
                    Some(session.keep_alive_interval_seconds as u64),
                    session.startup_script.clone(),
                ),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        }
    };

    // Audit log
    let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
    crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionConnect, &format!("SSH connect {}@{}:{}", username, host, port));

    let handler = SshClientHandler {
        app_handle: app_handle.clone(),
        connection_id: connection_id.clone(),
        host: host.clone(),
        port,
        remote_forwards: Arc::new(TokioMutex::new(HashMap::new())),
        agent_enabled: agent_fwd,
        #[cfg(unix)]
        agent_sockets: HashMap::new(),
    };

    let remote_forwards = handler.remote_forwards.clone();

    emit_connect_log(&app_handle, &connection_id, "info", &format!("Connecting to {}@{}:{}…", username, host, port));

    // Connect — either directly or via jump host
    let (handle, jump_host_handle) = if let Some(ref jump) = jump_host {
        emit_connect_log(&app_handle, &connection_id, "info", &format!("Using jump host {}:{}…", jump.host, jump.port));
        match connect_via_jump(
            jump, &host, port, &username, &auth,
            handler, &app_handle, &connection_id, keep_alive_secs,
        ).await {
            Ok((h, jh)) => (h, Some(jh)),
            Err(e) => {
                emit_connect_log(&app_handle, &connection_id, "error", &format!("Jump host connection failed: {}", e));
                crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionConnect,
                    &format!("SSH connect FAILED {}@{}:{} via jump {}:{} - {}",
                        username, host, port, jump.host, jump.port, e));
                return Err(e);
            }
        }
    } else {
        // Step 1: Establish transport (TCP + SSH handshake)
        let mut h = ssh_connect_transport(&host, port, handler, keep_alive_secs)
            .await
            .map_err(|e| {
                emit_connect_log(&app_handle, &connection_id, "error", &format!("Transport failed: {}", e));
                crate::audit::append_event(
                    &pid,
                    crate::audit::AuditEventType::SessionConnect,
                    &format!("SSH connect FAILED {}@{}:{} - {}", username, host, port, e),
                );
                e
            })?;

        emit_connect_log(&app_handle, &connection_id, "info", "Transport established, host key verified");

        // Step 2: Try primary auth method
        let auth_method_name = match &auth {
            SshAuth::Password { .. } => "password",
            SshAuth::PrivateKey { .. } => "publickey",
            SshAuth::None => "none",
        };
        emit_connect_log(&app_handle, &connection_id, "info", &format!("Trying {} authentication…", auth_method_name));

        let authenticated = ssh_try_auth(&mut h, &username, &auth).await.map_err(|e| {
            emit_connect_log(&app_handle, &connection_id, "error", &format!("Auth error: {}", e));
            crate::audit::append_event(
                &pid,
                crate::audit::AuditEventType::SessionConnect,
                &format!("SSH connect FAILED {}@{}:{} - {}", username, host, port, e),
            );
            e
        })?;

        // Step 3: If primary auth failed, fall back to keyboard-interactive
        if !authenticated {
            emit_connect_log(&app_handle, &connection_id, "warn", &format!("{} authentication rejected, trying keyboard-interactive…", auth_method_name));
            ssh_keyboard_interactive_auth(
                &mut h,
                &username,
                &connection_id,
                &app_handle,
                &state.pending_auth_responses,
            )
            .await
            .map_err(|e| {
                emit_connect_log(&app_handle, &connection_id, "error", &format!("Keyboard-interactive auth failed: {}", e));
                crate::audit::append_event(
                    &pid,
                    crate::audit::AuditEventType::SessionConnect,
                    &format!("SSH connect FAILED {}@{}:{} - {}", username, host, port, e),
                );
                e
            })?;
        }

        (h, None)
    };

    emit_connect_log(&app_handle, &connection_id, "info", "Authentication successful");

    // Emit auth success event so frontend can offer auto-save
    let auth_method_label = match &auth {
        SshAuth::Password { .. } => "password",
        SshAuth::PrivateKey { .. } => "publickey",
        SshAuth::None => "none",
    };
    let _ = app_handle.emit(
        "ssh:auth_success",
        SshAuthSuccessEvent {
            connection_id: connection_id.clone(),
            host: host.clone(),
            port,
            username: username.clone(),
            auth_method: auth_method_label.to_string(),
        },
    );

    // Measure connection latency (connect start to auth success)
    let latency_ms = connect_start.elapsed().as_millis() as u64;

    // Derive cipher/kex from the preferred config used for this connection
    let config = build_config(keep_alive_secs);
    let cipher_algorithm = config.preferred.cipher.first().map(|c| c.as_ref().to_string());
    let kex_algorithm = config.preferred.kex.first().map(|k| k.as_ref().to_string());

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

    // Request agent forwarding before PTY/shell (if enabled)
    if agent_fwd {
        let _ = channel.agent_forward(false).await;
    }

    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

    channel
        .request_shell(false)
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

    // Send startup script if configured
    if let Some(ref script) = startup_script {
        if !script.is_empty() {
            let script_data = if script.ends_with('\n') {
                script.as_bytes().to_vec()
            } else {
                format!("{}\n", script).into_bytes()
            };
            let _ = channel.data(&script_data[..]).await;
        }
    }

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SshCommand>(256);

    // Output buffer: collects data until the frontend terminal mounts and drains it
    let output_buffer: Arc<std::sync::Mutex<Option<Vec<String>>>> =
        Arc::new(std::sync::Mutex::new(Some(Vec::new())));
    let output_buffer_reader = output_buffer.clone();

    let conn_id_reader = connection_id.clone();
    let app_reader = app_handle.clone();
    let connections_ref = state.connections.clone();
    tokio::spawn(async move {
        let mut ch = channel;
        loop {
            tokio::select! {
                msg = ch.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { data }) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            // Buffer output if the frontend hasn't drained yet
                            let should_emit = {
                                let mut buf = output_buffer_reader.lock().unwrap();
                                if let Some(ref mut vec) = *buf {
                                    vec.push(text.clone());
                                    false
                                } else {
                                    true
                                }
                            };
                            if should_emit {
                                let _ = app_reader.emit(
                                    "ssh:output",
                                    SshOutputEvent {
                                        connection_id: conn_id_reader.clone(),
                                        data: text,
                                    },
                                );
                            }
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => {
                            let _ = app_reader.emit(
                                "ssh:disconnected",
                                SshDisconnectedEvent {
                                    connection_id: conn_id_reader.clone(),
                                    reason: format!("Process exited with status {}", exit_status),
                                },
                            );
                            break;
                        }
                        Some(ChannelMsg::Eof | ChannelMsg::Close) => {
                            let _ = app_reader.emit(
                                "ssh:disconnected",
                                SshDisconnectedEvent {
                                    connection_id: conn_id_reader.clone(),
                                    reason: "Connection closed".into(),
                                },
                            );
                            break;
                        }
                        None => {
                            let _ = app_reader.emit(
                                "ssh:disconnected",
                                SshDisconnectedEvent {
                                    connection_id: conn_id_reader.clone(),
                                    reason: "Connection lost".into(),
                                },
                            );
                            break;
                        }
                        _ => {}
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(SshCommand::Write(data)) => {
                            if ch.data(&data[..]).await.is_err() {
                                break;
                            }
                        }
                        Some(SshCommand::Resize { cols, rows }) => {
                            let _ = ch.window_change(cols, rows, 0, 0).await;
                        }
                        Some(SshCommand::Close) | None => {
                            let _ = ch.close().await;
                            break;
                        }
                    }
                }
            }
        }
        let mut conns = connections_ref.write().await;
        conns.remove(&conn_id_reader);
    });

    let conn = SshConnection {
        info: SshConnectionInfo {
            connection_id: connection_id.clone(),
            session_id: session_id.clone(),
            host,
            port,
            username,
            connected_at: chrono::Utc::now().to_rfc3339(),
            port_forwards: Vec::new(),
            cipher_algorithm: cipher_algorithm.clone(),
            kex_algorithm: kex_algorithm.clone(),
            latency_ms: Some(latency_ms),
        },
        handle,
        jump_host_handle,
        cmd_tx,
        forward_tasks: HashMap::new(),
        remote_forwards,
        output_buffer,
        drained_cache: Arc::new(std::sync::Mutex::new(None)),
    };

    {
        let mut connections = state.connections.write().await;
        connections.insert(connection_id.clone(), Arc::new(TokioMutex::new(conn)));
    }

    // Update last_connected_at on the session definition
    if !pid.is_empty() {
        if let Ok(mut session) = crate::config::do_session_get(&pid, &session_id) {
            session.last_connected_at = Some(chrono::Utc::now());
            session.updated_at = chrono::Utc::now();
            if let Ok(json) = serde_json::to_string_pretty(&session) {
                let _ = std::fs::write(
                    crate::config::session_file_path(&pid, &session_id),
                    json,
                );
            }
        }
    }

    let _ = app_handle.emit(
        "ssh:connected",
        SshConnectedEvent {
            connection_id: connection_id.clone(),
            cipher_algorithm,
            kex_algorithm,
            latency_ms: Some(latency_ms),
        },
    );

    Ok(connection_id)
}

/// Drain the early-output buffer for a connection.  Returns all buffered
/// output and switches the reader task to direct event emission.
#[tauri::command]
pub async fn ssh_drain_buffer(
    state: tauri::State<'_, SshState>,
    connection_id: String,
) -> Result<String, SshError> {
    let conn_arc = {
        let connections = state.connections.read().await;
        connections
            .get(&connection_id)
            .cloned()
            .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
    };
    let conn = conn_arc.lock().await;
    // Return cached content if already drained (handles React StrictMode double-mount).
    {
        let cache = conn.drained_cache.lock().unwrap();
        if let Some(ref cached) = *cache {
            return Ok(cached.clone());
        }
    }
    let result = {
        let mut buf = conn.output_buffer.lock().unwrap();
        // Take the buffer out (sets to None), which tells the reader task
        // to switch to direct event emission.
        buf.take().unwrap_or_default().join("")
    };
    {
        let mut cache = conn.drained_cache.lock().unwrap();
        *cache = Some(result.clone());
    }
    Ok(result)
}

#[tauri::command]
pub async fn ssh_disconnect(
    app_handle: AppHandle,
    state: tauri::State<'_, SshState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    connection_id: String,
) -> Result<(), SshError> {
    // Audit log
    {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionDisconnect, &format!("SSH disconnect {}", connection_id));
    }

    let conn = {
        let mut connections = state.connections.write().await;
        connections
            .remove(&connection_id)
            .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
    };

    let mut conn = conn.lock().await;

    let _ = conn.cmd_tx.send(SshCommand::Close).await;

    for (_, task) in conn.forward_tasks.drain() {
        task.abort();
    }

    let _ = conn
        .handle
        .disconnect(Disconnect::ByApplication, "User disconnected", "en")
        .await;

    // Clean up jump host connection if present
    if let Some(ref jh) = conn.jump_host_handle {
        let _ = jh
            .disconnect(Disconnect::ByApplication, "User disconnected", "en")
            .await;
    }

    let _ = app_handle.emit(
        "ssh:disconnected",
        SshDisconnectedEvent {
            connection_id,
            reason: "User disconnected".into(),
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn ssh_auth_respond(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    responses: Vec<String>,
) -> Result<(), SshError> {
    let tx = state
        .pending_auth_responses
        .write()
        .await
        .remove(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id))?;
    tx.send(responses).map_err(|_| SshError::AuthFailed)?;
    Ok(())
}

#[tauri::command]
pub async fn ssh_write(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    data: String,
) -> Result<(), SshError> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
        .clone();

    let conn = conn.lock().await;
    conn.cmd_tx
        .send(SshCommand::Write(data.into_bytes()))
        .await
        .map_err(|_| SshError::Channel("Channel closed".into()))?;

    Ok(())
}

#[tauri::command]
pub async fn ssh_resize(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    rows: u32,
    cols: u32,
) -> Result<(), SshError> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
        .clone();

    let conn = conn.lock().await;
    conn.cmd_tx
        .send(SshCommand::Resize { cols, rows })
        .await
        .map_err(|_| SshError::Channel("Channel closed".into()))?;

    Ok(())
}

#[tauri::command]
pub async fn ssh_list_connections(
    state: tauri::State<'_, SshState>,
) -> Result<Vec<SshConnectionInfo>, SshError> {
    let connections = state.connections.read().await;
    let mut infos = Vec::new();
    for conn in connections.values() {
        let conn = conn.lock().await;
        infos.push(conn.info.clone());
    }
    Ok(infos)
}

#[tauri::command]
pub async fn ssh_port_forward_add(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    forward: PortForward,
) -> Result<(), SshError> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
        .clone();

    let forward_id = forward.id().to_string();

    match &forward {
        PortForward::Local {
            bind_host,
            bind_port,
            remote_host,
            remote_port,
            ..
        } => {
            let bind_addr = format!("{}:{}", bind_host, bind_port);
            let listener = TcpListener::bind(&bind_addr)
                .await
                .map_err(|e| SshError::PortForward(format!("Bind failed: {}", e)))?;

            let remote_host = remote_host.clone();
            let remote_port = *remote_port;
            let conn_clone = conn.clone();

            let task = tokio::spawn(async move {
                loop {
                    let Ok((mut tcp_stream, _)) = listener.accept().await else {
                        break;
                    };
                    let conn_inner = conn_clone.clone();
                    let rh = remote_host.clone();
                    let rp = remote_port;

                    tokio::spawn(async move {
                        let connection = conn_inner.lock().await;
                        let channel = match connection
                            .handle
                            .channel_open_direct_tcpip(&rh, rp as u32, "127.0.0.1", 0)
                            .await
                        {
                            Ok(ch) => ch,
                            Err(_) => return,
                        };
                        drop(connection);

                        let (mut tcp_read, mut tcp_write) = tcp_stream.split();
                        let mut ch = channel;

                        loop {
                            tokio::select! {
                                msg = ch.wait() => {
                                    match msg {
                                        Some(ChannelMsg::Data { data }) => {
                                            if tcp_write.write_all(&data).await.is_err() {
                                                break;
                                            }
                                        }
                                        Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
                                        _ => {}
                                    }
                                }
                                result = async {
                                    let mut buf = [0u8; 8192];
                                    tcp_read.read(&mut buf).await.map(|n| (n, buf))
                                } => {
                                    match result {
                                        Ok((0, _)) => break,
                                        Ok((n, buf)) => {
                                            if ch.data(&buf[..n]).await.is_err() {
                                                break;
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            }
                        }
                    });
                }
            });

            let mut conn_locked = conn.lock().await;
            conn_locked.info.port_forwards.push(forward);
            conn_locked.forward_tasks.insert(forward_id, task);
        }

        PortForward::Remote {
            bind_host,
            bind_port,
            remote_host,
            remote_port,
            ..
        } => {
            let mut conn_locked = conn.lock().await;
            conn_locked
                .handle
                .tcpip_forward(bind_host, *bind_port as u32)
                .await
                .map_err(|e| SshError::PortForward(format!("Remote forward failed: {}", e)))?;
            // Register the mapping so the handler can route incoming connections
            conn_locked.remote_forwards.lock().await.insert(
                (bind_host.clone(), *bind_port as u32),
                (remote_host.clone(), *remote_port),
            );
            conn_locked.info.port_forwards.push(forward);
        }

        PortForward::Dynamic {
            bind_host,
            bind_port,
            ..
        } => {
            let bind_addr = format!("{}:{}", bind_host, bind_port);
            let listener = TcpListener::bind(&bind_addr)
                .await
                .map_err(|e| SshError::PortForward(format!("SOCKS5 bind failed: {}", e)))?;

            let conn_clone = conn.clone();
            let task = tokio::spawn(async move {
                loop {
                    let Ok((mut tcp_stream, _)) = listener.accept().await else {
                        break;
                    };
                    let conn_inner = conn_clone.clone();

                    tokio::spawn(async move {
                        let (mut reader, mut writer) = tcp_stream.split();
                        let mut buf = [0u8; 258];

                        // SOCKS5 greeting
                        if reader.read(&mut buf[..2]).await.is_err() {
                            return;
                        }
                        if buf[0] != 0x05 {
                            return;
                        }
                        let nmethods = buf[1] as usize;
                        if nmethods > 0 && reader.read_exact(&mut buf[..nmethods]).await.is_err() {
                            return;
                        }
                        if writer.write_all(&[0x05, 0x00]).await.is_err() {
                            return;
                        }
                        if reader.read_exact(&mut buf[..4]).await.is_err() {
                            return;
                        }
                        if buf[1] != 0x01 {
                            return;
                        }

                        let (dest_host, dest_port) = match buf[3] {
                            0x01 => {
                                if reader.read_exact(&mut buf[..6]).await.is_err() {
                                    return;
                                }
                                let ip =
                                    format!("{}.{}.{}.{}", buf[0], buf[1], buf[2], buf[3]);
                                let port = u16::from_be_bytes([buf[4], buf[5]]);
                                (ip, port)
                            }
                            0x03 => {
                                if reader.read_exact(&mut buf[..1]).await.is_err() {
                                    return;
                                }
                                let len = buf[0] as usize;
                                if reader.read_exact(&mut buf[..len + 2]).await.is_err() {
                                    return;
                                }
                                let domain =
                                    String::from_utf8_lossy(&buf[..len]).to_string();
                                let port = u16::from_be_bytes([buf[len], buf[len + 1]]);
                                (domain, port)
                            }
                            // ── IPv6 (SOCKS5 ATYP 0x04) ──
                            0x04 => {
                                // 16 bytes IPv6 address + 2 bytes port
                                if reader.read_exact(&mut buf[..18]).await.is_err() {
                                    return;
                                }
                                let addr = std::net::Ipv6Addr::new(
                                    u16::from_be_bytes([buf[0], buf[1]]),
                                    u16::from_be_bytes([buf[2], buf[3]]),
                                    u16::from_be_bytes([buf[4], buf[5]]),
                                    u16::from_be_bytes([buf[6], buf[7]]),
                                    u16::from_be_bytes([buf[8], buf[9]]),
                                    u16::from_be_bytes([buf[10], buf[11]]),
                                    u16::from_be_bytes([buf[12], buf[13]]),
                                    u16::from_be_bytes([buf[14], buf[15]]),
                                );
                                let port = u16::from_be_bytes([buf[16], buf[17]]);
                                (format!("{}", addr), port)
                            }
                            _ => return,
                        };

                        let connection = conn_inner.lock().await;
                        let channel = match connection
                            .handle
                            .channel_open_direct_tcpip(
                                &dest_host,
                                dest_port as u32,
                                "127.0.0.1",
                                0,
                            )
                            .await
                        {
                            Ok(ch) => ch,
                            Err(_) => {
                                let _ = writer
                                    .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                                    .await;
                                return;
                            }
                        };
                        drop(connection);

                        if writer
                            .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                            .await
                            .is_err()
                        {
                            return;
                        }

                        let mut ch = channel;
                        loop {
                            tokio::select! {
                                msg = ch.wait() => {
                                    match msg {
                                        Some(ChannelMsg::Data { data }) => {
                                            if writer.write_all(&data).await.is_err() {
                                                break;
                                            }
                                        }
                                        Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
                                        _ => {}
                                    }
                                }
                                result = async {
                                    let mut b = [0u8; 8192];
                                    reader.read(&mut b).await.map(|n| (n, b))
                                } => {
                                    match result {
                                        Ok((0, _)) => break,
                                        Ok((n, b)) => {
                                            if ch.data(&b[..n]).await.is_err() {
                                                break;
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            }
                        }
                    });
                }
            });

            let mut conn_locked = conn.lock().await;
            conn_locked.info.port_forwards.push(forward);
            conn_locked.forward_tasks.insert(forward_id, task);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn ssh_port_forward_remove(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    forward_id: String,
) -> Result<(), SshError> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
        .clone();

    let mut conn = conn.lock().await;

    if let Some(task) = conn.forward_tasks.remove(&forward_id) {
        task.abort();
    }

    // For remote forwards, cancel the server-side forwarding and clean up mapping
    let removed_forward = conn.info.port_forwards.iter().find(|f| f.id() == forward_id).cloned();
    if let Some(PortForward::Remote { bind_host, bind_port, .. }) = removed_forward {
        let _ = conn
            .handle
            .cancel_tcpip_forward(&bind_host, bind_port as u32)
            .await;
        conn.remote_forwards
            .lock()
            .await
            .remove(&(bind_host, bind_port as u32));
    }

    conn.info.port_forwards.retain(|f| f.id() != forward_id);

    Ok(())
}

#[tauri::command]
pub async fn ssh_exec(
    state: tauri::State<'_, SshState>,
    connection_id: String,
    command: String,
) -> Result<String, SshError> {
    let connections = state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| SshError::NotFound(connection_id.clone()))?
        .clone();

    let conn = conn.lock().await;

    let channel = conn
        .handle
        .channel_open_session()
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

    let mut output = Vec::new();
    let mut ch = channel;

    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => {
                output.extend_from_slice(&data);
            }
            Some(ChannelMsg::ExtendedData { data, .. }) => {
                output.extend_from_slice(&data);
            }
            Some(ChannelMsg::ExitStatus { .. })
            | Some(ChannelMsg::Eof)
            | Some(ChannelMsg::Close) => {
                break;
            }
            None => break,
            _ => {}
        }
    }

    Ok(String::from_utf8_lossy(&output).to_string())
}

#[tauri::command]
pub async fn ssh_forget_host_key(host: String, port: u16) -> Result<(), SshError> {
    let known_hosts_path = known_hosts_file_path();
    if !known_hosts_path.exists() {
        return Ok(());
    }

    let host_key = format!("{}:{}", host, port);
    let content = std::fs::read_to_string(&known_hosts_path)
        .map_err(|e| SshError::Io(e.to_string()))?;

    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            !trimmed.starts_with(&format!("{} ", host_key))
        })
        .collect();

    std::fs::write(&known_hosts_path, filtered.join("\n") + "\n")
        .map_err(|e| SshError::Io(e.to_string()))?;

    Ok(())
}

// ── BE-MOD-02: SSH key generation and listing ───────────────────────────

#[tauri::command]
pub async fn ssh_generate_key(
    key_type: String,
    comment: Option<String>,
    passphrase: Option<String>,
    output_path: String,
) -> Result<String, SshError> {
    use ssh_key::{Algorithm, LineEnding, private::PrivateKey};

    let algorithm = match key_type.to_lowercase().as_str() {
        "ed25519" => Algorithm::Ed25519,
        "rsa" => Algorithm::Rsa { hash: None },
        "ecdsa" | "ecdsa-p256" => Algorithm::Ecdsa {
            curve: ssh_key::EcdsaCurve::NistP256,
        },
        other => return Err(SshError::Key(format!("Unsupported key type: {}", other))),
    };

    let private_key = PrivateKey::random(&mut rand::rngs::OsRng, algorithm)
        .map_err(|e| SshError::Key(e.to_string()))?;

    let output = std::path::Path::new(&output_path);
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| SshError::Io(e.to_string()))?;
    }

    // Write private key
    let private_pem = if let Some(ref pass) = passphrase {
        private_key
            .encrypt(&mut rand::rngs::OsRng, pass)
            .map_err(|e| SshError::Key(e.to_string()))?
            .to_openssh(LineEnding::LF)
            .map_err(|e| SshError::Key(e.to_string()))?
    } else {
        private_key
            .to_openssh(LineEnding::LF)
            .map_err(|e| SshError::Key(e.to_string()))?
    };

    tokio::fs::write(&output_path, private_pem.as_bytes()).await
        .map_err(|e| SshError::Io(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(0o600)).await
            .map_err(|e| SshError::Io(e.to_string()))?;
    }

    // Write public key
    let public_key = private_key.public_key();
    let mut pub_openssh = public_key
        .to_openssh()
        .map_err(|e| SshError::Key(e.to_string()))?;
    if let Some(ref c) = comment {
        if !c.is_empty() {
            pub_openssh = format!("{} {}", pub_openssh.trim(), c);
        }
    }
    let pub_path = format!("{}.pub", output_path);
    tokio::fs::write(&pub_path, format!("{}\n", pub_openssh)).await
        .map_err(|e| SshError::Io(e.to_string()))?;

    let fingerprint = public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();
    Ok(fingerprint)
}

#[tauri::command]
pub async fn ssh_list_keys(
    directory: String,
) -> Result<Vec<String>, SshError> {
    let dir = std::path::Path::new(&directory);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut keys = Vec::new();
    let mut entries = tokio::fs::read_dir(&directory).await
        .map_err(|e| SshError::Io(e.to_string()))?;
    while let Some(entry) = entries.next_entry().await
        .map_err(|e| SshError::Io(e.to_string()))? {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "pub" {
                keys.push(path.to_string_lossy().to_string());
            }
        }
    }
    keys.sort();
    Ok(keys)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_state_new() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = SshState::new();
            let connections = state.connections.read().await;
            assert!(connections.is_empty(), "New SshState should have no connections");
        });
    }

    #[test]
    fn test_build_config_default_keepalive() {
        let config = build_config(None);
        assert_eq!(
            config.keepalive_interval,
            Some(std::time::Duration::from_secs(30))
        );
        assert_eq!(config.keepalive_max, 3);
    }

    #[test]
    fn test_build_config_custom_keepalive() {
        let config = build_config(Some(60));
        assert_eq!(
            config.keepalive_interval,
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(config.keepalive_max, 3);
    }

    #[test]
    fn test_port_forward_id_local() {
        let pf = PortForward::Local {
            id: "fwd-local-1".to_string(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8080,
            remote_host: "db.internal".to_string(),
            remote_port: 5432,
        };
        assert_eq!(pf.id(), "fwd-local-1");
    }

    #[test]
    fn test_port_forward_id_remote() {
        let pf = PortForward::Remote {
            id: "fwd-remote-1".to_string(),
            bind_host: "0.0.0.0".to_string(),
            bind_port: 9090,
            remote_host: "localhost".to_string(),
            remote_port: 3000,
        };
        assert_eq!(pf.id(), "fwd-remote-1");
    }

    #[test]
    fn test_port_forward_id_dynamic() {
        let pf = PortForward::Dynamic {
            id: "fwd-dyn-1".to_string(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 1080,
        };
        assert_eq!(pf.id(), "fwd-dyn-1");
    }

    #[test]
    fn test_port_forward_serde_roundtrip() {
        let pf = PortForward::Local {
            id: "test-id".to_string(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8080,
            remote_host: "db.host".to_string(),
            remote_port: 5432,
        };
        let json = serde_json::to_string(&pf).unwrap();
        let deserialized: PortForward = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id(), "test-id");
        // Verify tagged union serialization
        assert!(json.contains("\"type\":\"local\""));
    }

    #[test]
    fn test_port_forward_dynamic_serde() {
        let pf = PortForward::Dynamic {
            id: "socks".to_string(),
            bind_host: "127.0.0.1".to_string(),
            bind_port: 1080,
        };
        let json = serde_json::to_string(&pf).unwrap();
        assert!(json.contains("\"type\":\"dynamic\""));
        // Dynamic should not have remote_host/remote_port
        assert!(!json.contains("remote_host"));
    }

    #[test]
    fn test_ssh_auth_password_serde() {
        let auth = SshAuth::Password {
            password: "secret123".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"password\""));
        let deserialized: SshAuth = serde_json::from_str(&json).unwrap();
        match deserialized {
            SshAuth::Password { password } => assert_eq!(password, "secret123"),
            _ => panic!("Expected Password variant"),
        }
    }

    #[test]
    fn test_ssh_auth_private_key_serde() {
        let auth = SshAuth::PrivateKey {
            key_data: "-----BEGIN OPENSSH PRIVATE KEY-----\ntest\n-----END OPENSSH PRIVATE KEY-----".to_string(),
            passphrase: Some("mypass".to_string()),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"private_key\""));
        let deserialized: SshAuth = serde_json::from_str(&json).unwrap();
        match deserialized {
            SshAuth::PrivateKey { key_data, passphrase } => {
                assert!(key_data.contains("OPENSSH"));
                assert_eq!(passphrase, Some("mypass".to_string()));
            }
            _ => panic!("Expected PrivateKey variant"),
        }
    }

    #[test]
    fn test_ssh_connection_info_serde() {
        let info = SshConnectionInfo {
            connection_id: "conn-1".to_string(),
            session_id: "sess-1".to_string(),
            host: "example.com".to_string(),
            port: 22,
            username: "admin".to_string(),
            connected_at: "2024-01-01T00:00:00Z".to_string(),
            port_forwards: vec![],
            cipher_algorithm: Some("aes256-gcm@openssh.com".to_string()),
            kex_algorithm: Some("curve25519-sha256".to_string()),
            latency_ms: Some(42),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SshConnectionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.connection_id, "conn-1");
        assert_eq!(deserialized.host, "example.com");
        assert_eq!(deserialized.port, 22);
    }

    #[test]
    fn test_ssh_error_display() {
        assert_eq!(
            SshError::NotFound("abc".into()).to_string(),
            "Connection not found: abc"
        );
        assert_eq!(SshError::AuthFailed.to_string(), "Authentication failed");
        assert_eq!(
            SshError::PortForward("bind failed".into()).to_string(),
            "Port forward error: bind failed"
        );
    }

    #[test]
    fn test_ssh_error_serialize() {
        let err = SshError::AuthFailed;
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Authentication failed\"");
    }

    #[test]
    fn test_jump_host_serde() {
        let jh = JumpHost {
            host: "bastion.example.com".to_string(),
            port: 22,
            username: "jumpuser".to_string(),
            auth: SshAuth::Password {
                password: "pass".to_string(),
            },
        };
        let json = serde_json::to_string(&jh).unwrap();
        let deserialized: JumpHost = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "bastion.example.com");
        assert_eq!(deserialized.username, "jumpuser");
    }

    #[test]
    fn test_socks5_ipv6_parsing() {
        // Simulate the IPv6 address parsing logic from the SOCKS5 handler.
        // 16 bytes of IPv6 address + 2 bytes of port (big-endian).

        // Test 1: loopback address ::1, port 8080
        let buf: [u8; 18] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // first 8 bytes (4 groups of zeros)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // last 8 bytes (::1)
            0x1F, 0x90, // port 8080
        ];
        let addr = std::net::Ipv6Addr::new(
            u16::from_be_bytes([buf[0], buf[1]]),
            u16::from_be_bytes([buf[2], buf[3]]),
            u16::from_be_bytes([buf[4], buf[5]]),
            u16::from_be_bytes([buf[6], buf[7]]),
            u16::from_be_bytes([buf[8], buf[9]]),
            u16::from_be_bytes([buf[10], buf[11]]),
            u16::from_be_bytes([buf[12], buf[13]]),
            u16::from_be_bytes([buf[14], buf[15]]),
        );
        let port = u16::from_be_bytes([buf[16], buf[17]]);
        assert_eq!(format!("{}", addr), "::1");
        assert_eq!(port, 8080);

        // Test 2: 2001:db8::1, port 443
        let buf2: [u8; 18] = [
            0x20, 0x01, 0x0d, 0xb8, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x01, 0xBB, // port 443
        ];
        let addr2 = std::net::Ipv6Addr::new(
            u16::from_be_bytes([buf2[0], buf2[1]]),
            u16::from_be_bytes([buf2[2], buf2[3]]),
            u16::from_be_bytes([buf2[4], buf2[5]]),
            u16::from_be_bytes([buf2[6], buf2[7]]),
            u16::from_be_bytes([buf2[8], buf2[9]]),
            u16::from_be_bytes([buf2[10], buf2[11]]),
            u16::from_be_bytes([buf2[12], buf2[13]]),
            u16::from_be_bytes([buf2[14], buf2[15]]),
        );
        let port2 = u16::from_be_bytes([buf2[16], buf2[17]]);
        assert_eq!(format!("{}", addr2), "2001:db8::1");
        assert_eq!(port2, 443);

        // Test 3: fully specified address fe80::1:2:3:4, port 22
        let buf3: [u8; 18] = [
            0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04,
            0x00, 0x16, // port 22
        ];
        let addr3 = std::net::Ipv6Addr::new(
            u16::from_be_bytes([buf3[0], buf3[1]]),
            u16::from_be_bytes([buf3[2], buf3[3]]),
            u16::from_be_bytes([buf3[4], buf3[5]]),
            u16::from_be_bytes([buf3[6], buf3[7]]),
            u16::from_be_bytes([buf3[8], buf3[9]]),
            u16::from_be_bytes([buf3[10], buf3[11]]),
            u16::from_be_bytes([buf3[12], buf3[13]]),
            u16::from_be_bytes([buf3[14], buf3[15]]),
        );
        let port3 = u16::from_be_bytes([buf3[16], buf3[17]]);
        assert_eq!(format!("{}", addr3), "fe80::1:2:3:4");
        assert_eq!(port3, 22);
    }

    // ── Test helpers for integration tests ──────────────────────────────

    const TEST_SSH_HOST: &str = "127.0.0.1";
    const TEST_SSH_PORT: u16 = 2222;
    const TEST_JUMP_PORT: u16 = 2223;
    const TEST_USER: &str = "testuser";
    const TEST_PASS: &str = "testpass123";

    struct TestHandler;

    #[async_trait]
    impl client::Handler for TestHandler {
        type Error = russh::Error;

        async fn check_server_key(
            &mut self,
            _server_public_key: &PublicKey,
        ) -> Result<bool, Self::Error> {
            Ok(true) // Accept all host keys in tests
        }
    }

    async fn test_connect(
        host: &str,
        port: u16,
        user: &str,
        pass: &str,
    ) -> client::Handle<TestHandler> {
        let config = Arc::new(client::Config::default());
        let handler = TestHandler;
        let addr = format!("{}:{}", host, port);
        let mut handle = client::connect(config, &addr, handler).await.unwrap();
        let authenticated = handle.authenticate_password(user, pass).await.unwrap();
        assert!(authenticated, "password auth should succeed");
        handle
    }

    async fn test_exec(handle: &client::Handle<TestHandler>, cmd: &str) -> String {
        let channel = handle.channel_open_session().await.unwrap();
        channel.exec(true, cmd.as_bytes()).await.unwrap();
        let mut output = Vec::new();
        let mut ch = channel;
        loop {
            match ch.wait().await {
                Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
                Some(ChannelMsg::ExtendedData { data, .. }) => output.extend_from_slice(&data),
                Some(ChannelMsg::ExitStatus { .. })
                | Some(ChannelMsg::Eof)
                | Some(ChannelMsg::Close) => break,
                None => break,
                _ => {}
            }
        }
        String::from_utf8_lossy(&output).trim().to_string()
    }

    // ── Integration tests requiring a real SSH server ───────────────────

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_connect_password_auth() {
        // IT: connect to localhost:2222 with password auth, verify handle works
        let handle = test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;
        let channel = handle
            .channel_open_session()
            .await
            .expect("should open session channel on authenticated connection");
        channel.close().await.unwrap();
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[test]
    fn test_agent_forward_error() {
        let err = SshError::AgentForward("socket closed".into());
        assert_eq!(
            err.to_string(),
            "Agent forwarding error: socket closed"
        );
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("socket closed"));
    }

    #[test]
    fn test_jump_host_serde_with_key_auth() {
        let jh = JumpHost {
            host: "bastion.example.com".to_string(),
            port: 2222,
            username: "jump_user".to_string(),
            auth: SshAuth::PrivateKey {
                key_data: "-----BEGIN OPENSSH PRIVATE KEY-----\ntest\n-----END OPENSSH PRIVATE KEY-----".to_string(),
                passphrase: None,
            },
        };
        let json = serde_json::to_string(&jh).unwrap();
        let deserialized: JumpHost = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "bastion.example.com");
        assert_eq!(deserialized.port, 2222);
        match deserialized.auth {
            SshAuth::PrivateKey { key_data, passphrase } => {
                assert!(key_data.contains("OPENSSH"));
                assert!(passphrase.is_none());
            }
            _ => panic!("Expected PrivateKey variant"),
        }
    }

    // ── Integration tests for Jump Host (BE-SSH-01) ─────────────────────

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_connect_via_jump_host() {
        // IT: connect to target (crossterm-test-ssh:2222) through jump host (localhost:2223)
        let jump_handle =
            test_connect(TEST_SSH_HOST, TEST_JUMP_PORT, TEST_USER, TEST_PASS).await;

        // Open direct-tcpip tunnel through jump host to the target container
        let channel = jump_handle
            .channel_open_direct_tcpip("crossterm-test-ssh", 2222, "127.0.0.1", 0)
            .await
            .expect("should open direct-tcpip channel to target via jump host");

        let stream = channel.into_stream();
        let config = Arc::new(client::Config::default());
        let mut target_handle = client::connect_stream(config, stream, TestHandler)
            .await
            .expect("SSH handshake through jump tunnel should succeed");

        let authenticated = target_handle
            .authenticate_password(TEST_USER, TEST_PASS)
            .await
            .expect("target auth should not error");
        assert!(authenticated, "target password auth should succeed");

        // Verify we can execute a command on the target
        let output = test_exec(&target_handle, "whoami").await;
        assert_eq!(output, TEST_USER);

        let _ = target_handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_jump_host_cleanup_on_disconnect() {
        // IT: connect via jump host, disconnect both, verify handles are dropped
        let jump_handle =
            test_connect(TEST_SSH_HOST, TEST_JUMP_PORT, TEST_USER, TEST_PASS).await;

        let channel = jump_handle
            .channel_open_direct_tcpip("crossterm-test-ssh", 2222, "127.0.0.1", 0)
            .await
            .expect("should open tunnel");

        let config = Arc::new(client::Config::default());
        let mut target_handle =
            client::connect_stream(config, channel.into_stream(), TestHandler)
                .await
                .unwrap();
        let auth = target_handle
            .authenticate_password(TEST_USER, TEST_PASS)
            .await
            .unwrap();
        assert!(auth);

        // Disconnect target first, then jump host
        let _ = target_handle
            .disconnect(Disconnect::ByApplication, "cleanup test", "en")
            .await;
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "cleanup test", "en")
            .await;
        // If we get here without panic/hang, cleanup succeeded
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_jump_host_target_auth_failure() {
        // IT: connect to jump host OK, then fail auth on target with wrong password
        let jump_handle =
            test_connect(TEST_SSH_HOST, TEST_JUMP_PORT, TEST_USER, TEST_PASS).await;

        let channel = jump_handle
            .channel_open_direct_tcpip("crossterm-test-ssh", 2222, "127.0.0.1", 0)
            .await
            .expect("tunnel should open");

        let config = Arc::new(client::Config::default());
        let mut target_handle =
            client::connect_stream(config, channel.into_stream(), TestHandler)
                .await
                .unwrap();

        // Attempt auth with wrong password
        let authenticated = target_handle
            .authenticate_password(TEST_USER, "wrong_password_123")
            .await
            .unwrap();
        assert!(
            !authenticated,
            "auth with wrong password should fail"
        );

        // Clean up jump host
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    // ── Integration tests for Agent Forwarding (BE-SSH-02) ──────────────

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_agent_forward_connect() {
        // IT: connect with agent forwarding requested on the channel
        let handle =
            test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;

        let channel = handle
            .channel_open_session()
            .await
            .expect("should open session");

        // Request agent forwarding — may silently fail if no agent is running,
        // but should not error the channel
        let _ = channel.agent_forward(false).await;

        // Verify the channel is still functional after agent_forward request
        channel
            .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
            .await
            .expect("PTY request should succeed after agent forward");
        channel
            .request_shell(false)
            .await
            .expect("shell should open");

        channel.close().await.unwrap();
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_agent_forward_no_agent_sock() {
        // IT: connect when SSH_AUTH_SOCK is not set — should still work
        let original_sock = std::env::var("SSH_AUTH_SOCK").ok();
        // SAFETY: test-only env manipulation
        unsafe { std::env::remove_var("SSH_AUTH_SOCK") };

        let handle =
            test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;
        let output = test_exec(&handle, "echo agent_test_ok").await;
        assert!(
            output.contains("agent_test_ok"),
            "exec should work without agent"
        );

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;

        // Restore SSH_AUTH_SOCK
        if let Some(sock) = original_sock {
            // SAFETY: test-only env manipulation
            unsafe { std::env::set_var("SSH_AUTH_SOCK", sock) };
        }
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_connect_and_exec() {
        // IT: connect, exec "echo hello", verify output
        let handle =
            test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;
        let output = test_exec(&handle, "echo hello").await;
        assert!(
            output.contains("hello"),
            "output should contain 'hello', got: {}",
            output
        );
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_local_port_forward() {
        // IT: open direct-tcpip channel (the primitive behind local port forwarding)
        let handle =
            test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;

        // Forward to port 2222 on the remote loopback (the SSH server itself)
        let channel = handle
            .channel_open_direct_tcpip("127.0.0.1", 2222, "127.0.0.1", 0)
            .await
            .expect("direct-tcpip channel should open for local port forward");

        // Read the SSH banner from the forwarded connection
        let mut ch = channel;
        let mut banner = Vec::new();
        if let Some(ChannelMsg::Data { data }) = ch.wait().await {
            banner.extend_from_slice(&data);
        }
        let banner_str = String::from_utf8_lossy(&banner);
        assert!(
            banner_str.contains("SSH"),
            "should receive SSH banner through tunnel, got: {}",
            banner_str
        );

        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    #[tokio::test]
    #[ignore = "Requires Docker: docker compose -f tests/docker-compose.yml up -d"]
    async fn test_ssh_disconnect() {
        // IT: connect, verify, then disconnect gracefully
        let handle =
            test_connect(TEST_SSH_HOST, TEST_SSH_PORT, TEST_USER, TEST_PASS).await;

        let output = test_exec(&handle, "echo connected").await;
        assert!(output.contains("connected"));

        handle
            .disconnect(Disconnect::ByApplication, "test disconnect", "en")
            .await
            .expect("disconnect should succeed");
    }

    #[test]
    fn test_known_hosts_file_path() {
        let path = known_hosts_file_path();
        assert!(path.ends_with("CrossTerm/known_hosts"));
    }

    #[test]
    fn test_host_key_changed_error() {
        let err = SshError::HostKeyChanged("example.com:22".into());
        assert!(err.to_string().contains("example.com:22"));
        assert!(err.to_string().contains("MITM"));
    }

    #[test]
    fn test_ssh_connect_log_event_serde() {
        let log = SshConnectLogEvent {
            connection_id: "conn-123".to_string(),
            level: "info".to_string(),
            message: "Transport established".to_string(),
        };
        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"connection_id\":\"conn-123\""));
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"message\":\"Transport established\""));
    }

    #[test]
    fn test_ssh_banner_event_serde() {
        let banner = SshBannerEvent {
            connection_id: "conn-abc".to_string(),
            banner: "Authorized users only\n".to_string(),
        };
        let json = serde_json::to_string(&banner).unwrap();
        assert!(json.contains("\"banner\":\"Authorized users only"));
    }

    #[test]
    fn test_ssh_discover_result_serde() {
        let result = SshDiscoverResult {
            host: "192.168.1.1".to_string(),
            port: 22,
            none_auth_accepted: false,
            auth_required: true,
            banner: Some("Welcome".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SshDiscoverResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "192.168.1.1");
        assert!(deserialized.auth_required);
        assert!(!deserialized.none_auth_accepted);
        assert_eq!(deserialized.banner, Some("Welcome".to_string()));
    }

    #[test]
    fn test_ssh_discover_result_no_banner() {
        let result = SshDiscoverResult {
            host: "10.0.0.1".to_string(),
            port: 2222,
            none_auth_accepted: true,
            auth_required: false,
            banner: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SshDiscoverResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.none_auth_accepted);
        assert!(!deserialized.auth_required);
        assert_eq!(deserialized.banner, None);
    }

    #[test]
    fn test_ssh_auth_success_event_serde() {
        let evt = SshAuthSuccessEvent {
            connection_id: "conn-xyz".to_string(),
            host: "server.example.com".to_string(),
            port: 22,
            username: "admin".to_string(),
            auth_method: "password".to_string(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"auth_method\":\"password\""));
        assert!(json.contains("\"username\":\"admin\""));
        let deserialized: SshAuthSuccessEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.connection_id, "conn-xyz");
        assert_eq!(deserialized.auth_method, "password");
    }
}

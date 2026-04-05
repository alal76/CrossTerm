use async_trait::async_trait;
use russh::client;
use russh::keys::key::PublicKey;
use russh::{ChannelId, ChannelMsg, Disconnect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex as TokioMutex, RwLock};
use uuid::Uuid;
use zeroize::Zeroizing;

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
}

#[derive(Clone, Serialize)]
struct SshHostKeyNewEvent {
    host: String,
    port: u16,
    algorithm: String,
    fingerprint: String,
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
    remote_forwards: Arc<TokioMutex<HashMap<(String, u32), (String, u16)>>>,
}

#[async_trait]
impl client::Handler for SshClientHandler {
    type Error = SshError;

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
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let text = String::from_utf8_lossy(data).to_string();
        let _ = self.app_handle.emit(
            "ssh:output",
            SshOutputEvent {
                connection_id: self.connection_id.clone(),
                data: text,
            },
        );
        Ok(())
    }
}

// ── Connection ──────────────────────────────────────────────────────────

pub(crate) struct SshConnection {
    pub(crate) info: SshConnectionInfo,
    pub(crate) handle: client::Handle<SshClientHandler>,
    cmd_tx: mpsc::Sender<SshCommand>,
    forward_tasks: HashMap<String, tokio::task::JoinHandle<()>>,
    remote_forwards: Arc<TokioMutex<HashMap<(String, u32), (String, u16)>>>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SshState {
    pub(crate) connections: Arc<RwLock<HashMap<String, Arc<TokioMutex<SshConnection>>>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
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

fn build_config(keep_alive_secs: Option<u64>) -> Arc<client::Config> {
    let mut config = client::Config::default();
    config.keepalive_interval = Some(std::time::Duration::from_secs(keep_alive_secs.unwrap_or(30)));
    config.keepalive_max = 3;
    Arc::new(config)
}

async fn connect_and_auth(
    host: &str,
    port: u16,
    username: &str,
    auth: &SshAuth,
    handler: SshClientHandler,
    keep_alive_secs: Option<u64>,
) -> Result<client::Handle<SshClientHandler>, SshError> {
    let config = build_config(keep_alive_secs);
    let addr = format!("{}:{}", host, port);

    let mut handle = client::connect(config, &addr, handler)
        .await
        .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;

    let authenticated = match auth {
        SshAuth::Password { password } => {
            let password = Zeroizing::new(password.clone());
            handle
                .authenticate_password(username, (*password).clone())
                .await
                .map_err(|e| SshError::ConnectionFailed(e.to_string()))?
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
                .map_err(|e| SshError::ConnectionFailed(e.to_string()))?
        }
    };

    if !authenticated {
        return Err(SshError::AuthFailed);
    }

    Ok(handle)
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn ssh_connect(
    app_handle: AppHandle,
    state: tauri::State<'_, SshState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    session_id: String,
    host: String,
    port: u16,
    username: String,
    auth: SshAuth,
    _jump_host: Option<JumpHost>,
    cols: Option<u32>,
    rows: Option<u32>,
) -> Result<String, SshError> {
    let connection_id = Uuid::new_v4().to_string();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

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
    };

    let remote_forwards = handler.remote_forwards.clone();

    let handle = match connect_and_auth(&host, port, &username, &auth, handler, keep_alive_secs).await {
        Ok(h) => h,
        Err(e) => {
            crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionConnect, &format!("SSH connect FAILED {}@{}:{} - {}", username, host, port, e));
            return Err(e);
        }
    };

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| SshError::Channel(e.to_string()))?;

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
                            let _ = app_reader.emit(
                                "ssh:output",
                                SshOutputEvent {
                                    connection_id: conn_id_reader.clone(),
                                    data: text,
                                },
                            );
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
        },
        handle,
        cmd_tx,
        forward_tasks: HashMap::new(),
        remote_forwards,
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
        },
    );

    Ok(connection_id)
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
                        if nmethods > 0 {
                            if reader.read_exact(&mut buf[..nmethods]).await.is_err() {
                                return;
                            }
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

    // ── Integration tests requiring a real SSH server ───────────────────

    #[tokio::test]
    #[ignore = "Requires a running SSH server — use `docker run -d -p 2222:22 lscr.io/linuxserver/openssh-server` then run with --ignored"]
    async fn test_ssh_connect_password_auth() {
        // Would test: connect to localhost:2222 with password auth,
        // verify connection_id is returned and state has the connection.
    }

    #[tokio::test]
    #[ignore = "Requires a running SSH server — use Docker openssh-server container"]
    async fn test_ssh_connect_and_exec() {
        // Would test: connect, exec "echo hello", verify output contains "hello".
    }

    #[tokio::test]
    #[ignore = "Requires a running SSH server — use Docker openssh-server container"]
    async fn test_ssh_local_port_forward() {
        // Would test: connect, add local port forward 127.0.0.1:18080 -> remote:80,
        // verify the forward appears in connection info.
    }

    #[tokio::test]
    #[ignore = "Requires a running SSH server — use Docker openssh-server container"]
    async fn test_ssh_disconnect() {
        // Would test: connect, then disconnect, verify connection is removed from state.
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
}

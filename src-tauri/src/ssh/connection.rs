use super::*;
use tauri::AppHandle;

// ── Helpers ─────────────────────────────────────────────────────────────

pub(super) fn known_hosts_file_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("CrossTerm")
        .join("known_hosts")
}

#[allow(clippy::field_reassign_with_default)]
pub(super) fn build_config(keep_alive_secs: Option<u64>) -> Arc<client::Config> {
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
pub(super) async fn ssh_connect_transport(
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
pub(super) async fn ssh_try_auth(
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
#[allow(clippy::type_complexity)]
pub(super) async fn ssh_keyboard_interactive_auth(
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

pub(super) async fn connect_and_auth(
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
pub(super) async fn connect_via_jump(
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

pub(super) fn emit_connect_log(app_handle: &AppHandle, connection_id: &str, level: &str, message: &str) {
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
        .ok_or(SshError::NotFound(connection_id))?;
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

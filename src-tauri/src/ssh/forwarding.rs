use super::*;

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
                                        Some(ChannelMsg::Data { data })
                                            if tcp_write.write_all(&data).await.is_err() => { break; }
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
                                        Some(ChannelMsg::Data { data })
                                            if writer.write_all(&data).await.is_err() => { break; }
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

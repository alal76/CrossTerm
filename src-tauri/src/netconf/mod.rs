/// NETCONF (RFC 6241) over SSH subsystem.
/// Opens an SSH connection with the "netconf" subsystem, performs the
/// <hello> capability exchange, and exposes get-config / edit-config / RPC.
use russh::client::{self, Handle};
use russh::keys::key::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum NetconfError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("NETCONF error: {0}")]
    Protocol(String),
    #[error("Host key for {0} has changed — refusing to connect. If this is expected, forget the old key first.")]
    HostKeyChanged(String),
    #[error("No credentials supplied — provide a password or a private key")]
    NoCredentials,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for NetconfError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<russh::Error> for NetconfError {
    fn from(e: russh::Error) -> Self {
        NetconfError::Ssh(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetconfConfig {
    pub host: String,
    /// Default 830
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    /// PEM-encoded private key data (mirrors SshAuth::PrivateKey — the
    /// frontend resolves any credential reference to raw key material
    /// before invoking this command, same as it does for SSH).
    pub private_key: Option<String>,
    pub private_key_passphrase: Option<String>,
    /// List of requested YANG capability URIs (empty = accept server list)
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetconfSession {
    pub id: String,
    pub host: String,
    pub server_capabilities: Vec<String>,
    pub session_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetconfRpcResult {
    pub message_id: String,
    pub xml: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
struct NetconfHostKeyNewEvent {
    host: String,
    port: u16,
    algorithm: String,
    fingerprint: String,
}

/// Shares SSH's known_hosts store — a host's key identity doesn't depend
/// on which SSH subsystem (shell vs. netconf) is being used to reach it.
fn known_hosts_file_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("CrossTerm")
        .join("known_hosts")
}

struct SshHandler {
    app_handle: AppHandle,
    host: String,
    port: u16,
}

#[async_trait::async_trait]
impl client::Handler for SshHandler {
    type Error = NetconfError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key.fingerprint();
        let algorithm = server_public_key.name().to_string();
        let host_key = format!("{}:{}", self.host, self.port);

        let known_hosts_path = known_hosts_file_path();

        if known_hosts_path.exists() {
            let file = std::fs::File::open(&known_hosts_path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line?.trim().to_string();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() == 3 && parts[0] == host_key {
                    if parts[2] == fingerprint {
                        return Ok(true);
                    }
                    return Err(NetconfError::HostKeyChanged(host_key));
                }
            }
        }

        // Not found — TOFU: accept and record it for next time.
        if let Some(parent) = known_hosts_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&known_hosts_path)?;
        writeln!(file, "{} {} {}", host_key, algorithm, fingerprint)?;

        let _ = self.app_handle.emit(
            "netconf:host_key_new",
            NetconfHostKeyNewEvent {
                host: self.host.clone(),
                port: self.port,
                algorithm,
                fingerprint,
            },
        );

        Ok(true)
    }
}

pub struct NetconfState {
    sessions: Mutex<HashMap<String, NetconfSession>>,
    // Tokio Mutex so we can hold the guard across .await in send_rpc_and_receive
    channels: tokio::sync::Mutex<HashMap<String, russh::Channel<client::Msg>>>,
}

impl NetconfState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            channels: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn build_hello(caps: &[String]) -> String {
    let cap_xml = if caps.is_empty() {
        "<capability>urn:ietf:params:netconf:base:1.0</capability>".to_string()
    } else {
        caps.iter().map(|c| format!("<capability>{c}</capability>")).collect::<Vec<_>>().join("")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<hello xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">\
<capabilities>{cap_xml}</capabilities>\
</hello>]]>]]>"
    )
}

fn extract_session_id(hello_xml: &str) -> u32 {
    hello_xml.split("<session-id>").nth(1)
        .and_then(|s| s.split("</session-id>").next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn extract_capabilities(hello_xml: &str) -> Vec<String> {
    hello_xml.split("<capability>")
        .skip(1)
        .filter_map(|s| s.split("</capability>").next().map(str::to_string))
        .collect()
}

#[tauri::command]
pub async fn netconf_connect(
    app_handle: AppHandle,
    config: NetconfConfig,
    state: tauri::State<'_, NetconfState>,
) -> Result<String, NetconfError> {
    let ssh_config = Arc::new(russh::client::Config::default());
    let handler = SshHandler {
        app_handle,
        host: config.host.clone(),
        port: config.port,
    };
    let mut session = russh::client::connect(
        ssh_config,
        (config.host.as_str(), config.port),
        handler,
    ).await?;

    // Private key takes priority when both are supplied, matching SSH's
    // own auth-method preference.
    let authenticated = if let Some(key_data) = &config.private_key {
        let key_pair = if let Some(pass) = &config.private_key_passphrase {
            russh_keys::decode_secret_key(key_data, Some(pass.as_str()))
        } else {
            russh_keys::decode_secret_key(key_data, None)
        }.map_err(|e| NetconfError::Ssh(format!("Invalid private key: {e}")))?;
        session.authenticate_publickey(&config.username, Arc::new(key_pair)).await?
    } else if let Some(pw) = &config.password {
        session.authenticate_password(&config.username, pw.clone()).await?
    } else {
        return Err(NetconfError::NoCredentials);
    };
    if !authenticated {
        return Err(NetconfError::Ssh("Authentication rejected".into()));
    }

    let mut channel = session.channel_open_session()
        .await.map_err(|e| NetconfError::Ssh(e.to_string()))?;
    channel.request_subsystem(true, "netconf")
        .await.map_err(|e| NetconfError::Ssh(e.to_string()))?;

    // Send our hello
    let hello = build_hello(&config.capabilities);
    channel.data(hello.as_bytes())
        .await.map_err(|e| NetconfError::Ssh(e.to_string()))?;

    // Receive server hello
    let mut server_hello = String::new();
    loop {
        match channel.wait().await {
            Some(russh::ChannelMsg::Data { data }) => {
                server_hello.push_str(&String::from_utf8_lossy(&data));
                if server_hello.contains("]]>]]>") { break; }
            }
            _ => break,
        }
    }

    let session_id = extract_session_id(&server_hello);
    let server_caps = extract_capabilities(&server_hello);
    let id = Uuid::new_v4().to_string();

    state.sessions.lock().unwrap().insert(id.clone(), NetconfSession {
        id: id.clone(), host: config.host, server_capabilities: server_caps, session_id,
    });
    state.channels.lock().await.insert(id.clone(), channel);
    Ok(id)
}

#[tauri::command]
pub async fn netconf_get_config(
    id: String,
    datastore: String,
    filter: Option<String>,
    state: tauri::State<'_, NetconfState>,
) -> Result<NetconfRpcResult, NetconfError> {
    let msg_id = Uuid::new_v4().to_string();
    let filter_xml = filter.as_deref()
        .map(|f| format!("<filter>{f}</filter>"))
        .unwrap_or_default();
    let rpc = format!(
        "<?xml version=\"1.0\"?>\
<rpc message-id=\"{msg_id}\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">\
<get-config><source><{datastore}/></source>{filter_xml}</get-config>\
</rpc>]]>]]>"
    );
    send_rpc_and_receive(id, msg_id, rpc, state).await
}

#[tauri::command]
pub async fn netconf_rpc(
    id: String,
    xml_body: String,
    state: tauri::State<'_, NetconfState>,
) -> Result<NetconfRpcResult, NetconfError> {
    let msg_id = Uuid::new_v4().to_string();
    let rpc = format!(
        "<?xml version=\"1.0\"?>\
<rpc message-id=\"{msg_id}\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">\
{xml_body}\
</rpc>]]>]]>"
    );
    send_rpc_and_receive(id, msg_id, rpc, state).await
}

async fn send_rpc_and_receive(
    id: String,
    msg_id: String,
    rpc: String,
    state: tauri::State<'_, NetconfState>,
) -> Result<NetconfRpcResult, NetconfError> {
    let mut channels = state.channels.lock().await;  // tokio Mutex — safe to hold across .await
    let ch = channels.get_mut(&id).ok_or_else(|| NetconfError::NotFound(id.clone()))?;

    ch.data(rpc.as_bytes()).await.map_err(|e| NetconfError::Ssh(e.to_string()))?;

    let mut reply = String::new();
    loop {
        match ch.wait().await {
            Some(russh::ChannelMsg::Data { data }) => {
                reply.push_str(&String::from_utf8_lossy(&data));
                if reply.contains("]]>]]>") { break; }
            }
            _ => break,
        }
    }

    let ok = reply.contains("<ok/>") || reply.contains("<ok />");
    let error = if !ok {
        reply.split("<error-message>").nth(1)
            .and_then(|s| s.split("</error-message>").next())
            .map(str::to_string)
    } else { None };

    Ok(NetconfRpcResult { message_id: msg_id, xml: reply, ok, error })
}

#[tauri::command]
pub fn netconf_disconnect(id: String, state: tauri::State<'_, NetconfState>) -> Result<(), NetconfError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| NetconfError::NotFound(id.clone()))?;
    // channels lock is async but disconnect is sync; spawn a task to remove it
    let channels = state.channels.try_lock();
    if let Ok(mut guard) = channels {
        guard.remove(&id);
    }
    Ok(())
}

#[tauri::command]
pub fn netconf_list(state: tauri::State<'_, NetconfState>) -> Vec<NetconfSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_hello_defaults_to_base_1_0_when_no_capabilities_requested() {
        let hello = build_hello(&[]);
        assert!(hello.contains("urn:ietf:params:netconf:base:1.0"));
        assert!(hello.ends_with("]]>]]>"));
    }

    #[test]
    fn test_build_hello_includes_each_requested_capability() {
        let hello = build_hello(&["urn:ietf:params:netconf:capability:candidate:1.0".to_string()]);
        assert!(hello.contains("<capability>urn:ietf:params:netconf:capability:candidate:1.0</capability>"));
    }

    #[test]
    fn test_extract_session_id() {
        let xml = "<hello><session-id>42</session-id></hello>]]>]]>";
        assert_eq!(extract_session_id(xml), 42);
    }

    #[test]
    fn test_extract_session_id_missing_defaults_to_zero() {
        assert_eq!(extract_session_id("<hello></hello>]]>]]>"), 0);
    }

    #[test]
    fn test_extract_capabilities() {
        let xml = "<hello><capabilities>\
<capability>urn:ietf:params:netconf:base:1.1</capability>\
<capability>urn:ietf:params:netconf:capability:candidate:1.0</capability>\
</capabilities></hello>]]>]]>";
        let caps = extract_capabilities(xml);
        assert_eq!(caps, vec![
            "urn:ietf:params:netconf:base:1.1".to_string(),
            "urn:ietf:params:netconf:capability:candidate:1.0".to_string(),
        ]);
    }

    #[test]
    fn test_known_hosts_file_path() {
        let path = known_hosts_file_path();
        assert!(path.ends_with("CrossTerm/known_hosts"));
    }
}

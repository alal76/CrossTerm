/// NETCONF (RFC 6241) over SSH subsystem.
/// Opens an SSH connection with the "netconf" subsystem, performs the
/// <hello> capability exchange, and exposes get-config / edit-config / RPC.
use russh::client::{self, Handle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for NetconfError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetconfConfig {
    pub host: String,
    /// Default 830
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key_ref: Option<String>,
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

// Minimal no-auth SSH client handler
struct SshHandler;

#[async_trait::async_trait]
impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true) // TODO: TOFU verification
    }
}

pub struct NetconfState {
    sessions: Mutex<HashMap<String, NetconfSession>>,
    channels: Mutex<HashMap<String, russh::Channel<client::Msg>>>,
}

impl NetconfState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            channels: Mutex::new(HashMap::new()),
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
    config: NetconfConfig,
    state: tauri::State<'_, NetconfState>,
) -> Result<String, NetconfError> {
    let ssh_config = Arc::new(russh::client::Config::default());
    let mut session = russh::client::connect(
        ssh_config,
        (config.host.as_str(), config.port),
        SshHandler,
    ).await.map_err(|e| NetconfError::Ssh(e.to_string()))?;

    if let Some(pw) = &config.password {
        session.authenticate_password(&config.username, pw.clone())
            .await.map_err(|e| NetconfError::Ssh(e.to_string()))?;
    } else {
        return Err(NetconfError::Ssh("Key auth not yet wired for NETCONF".into()));
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
    state.channels.lock().unwrap().insert(id.clone(), channel);
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
    let mut channels = state.channels.lock().unwrap();
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
    state.channels.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn netconf_list(state: tauri::State<'_, NetconfState>) -> Vec<NetconfSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

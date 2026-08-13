use reqwest::header::{CONTENT_TYPE, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WinRmError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Command failed: {0}")]
    CommandFailed(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("XML parse error: {0}")]
    Xml(String),
}

impl Serialize for WinRmError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WinRmAuth {
    Basic,
    Ntlm,
    Kerberos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinRmConfig {
    pub host: String,
    /// 5985 = HTTP, 5986 = HTTPS
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    pub auth: WinRmAuth,
    pub verify_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinRmSession {
    pub id: String,
    pub host: String,
    pub username: String,
    pub shell_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinRmCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct WinRmState {
    sessions: Mutex<HashMap<String, (WinRmConfig, String)>>, // id -> (config, shell_id)
}

impl WinRmState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

fn build_client(config: &WinRmConfig) -> Result<reqwest::Client, WinRmError> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30));
    if !config.verify_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().map_err(|e| WinRmError::Http(e.to_string()))
}

fn endpoint_url(config: &WinRmConfig) -> String {
    let scheme = if config.use_tls { "https" } else { "http" };
    format!("{scheme}://{}:{}/wsman", config.host, config.port)
}

fn create_shell_envelope() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsmv="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
  <s:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsmv:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</wsmv:ResourceURI>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2004/09/transfer/Create</wsa:Action>
    <wsmv:OperationTimeout>PT60S</wsmv:OperationTimeout>
  </s:Header>
  <s:Body>
    <rsp:Shell><rsp:InputStreams>stdin</rsp:InputStreams><rsp:OutputStreams>stdout stderr</rsp:OutputStreams></rsp:Shell>
  </s:Body>
</s:Envelope>"#.to_string()
}

fn command_envelope(shell_id: &str, command: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsmv="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
  <s:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsmv:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</wsmv:ResourceURI>
    <wsmv:SelectorSet><wsmv:Selector Name="ShellId">{shell_id}</wsmv:Selector></wsmv:SelectorSet>
    <wsa:Action>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Command</wsa:Action>
  </s:Header>
  <s:Body>
    <rsp:CommandLine><rsp:Command>{command}</rsp:Command></rsp:CommandLine>
  </s:Body>
</s:Envelope>"#)
}

#[tauri::command]
pub async fn winrm_connect(
    config: WinRmConfig,
    state: tauri::State<'_, WinRmState>,
) -> Result<String, WinRmError> {
    let client = build_client(&config)?;
    let url = endpoint_url(&config);
    let creds = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{}:{}", config.username, config.password),
    );

    let resp = client
        .post(&url)
        .header(CONTENT_TYPE, "application/soap+xml;charset=UTF-8")
        .header(AUTHORIZATION, format!("Basic {creds}"))
        .body(create_shell_envelope())
        .send()
        .await
        .map_err(|e| WinRmError::Http(e.to_string()))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| WinRmError::Http(e.to_string()))?;

    if status == 401 {
        return Err(WinRmError::AuthFailed);
    }
    if !status.is_success() {
        return Err(WinRmError::Http(format!("{status}: {body}")));
    }

    // Extract ShellId from response XML
    let shell_id = body
        .split("ShellId>")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or("")
        .to_string();

    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), (config, shell_id));
    Ok(id)
}

#[tauri::command]
pub async fn winrm_run_command(
    id: String,
    command: String,
    state: tauri::State<'_, WinRmState>,
) -> Result<WinRmCommandResult, WinRmError> {
    let (config, shell_id) = state
        .sessions.lock().unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| WinRmError::NotFound(id.clone()))?;

    let client = build_client(&config)?;
    let url = endpoint_url(&config);
    let creds = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{}:{}", config.username, config.password),
    );

    let resp = client
        .post(&url)
        .header(CONTENT_TYPE, "application/soap+xml;charset=UTF-8")
        .header(AUTHORIZATION, format!("Basic {creds}"))
        .body(command_envelope(&shell_id, &command))
        .send()
        .await
        .map_err(|e| WinRmError::Http(e.to_string()))?;

    let body = resp.text().await.map_err(|e| WinRmError::Http(e.to_string()))?;

    Ok(WinRmCommandResult {
        stdout: body.clone(),
        stderr: String::new(),
        exit_code: 0,
    })
}

#[tauri::command]
pub fn winrm_disconnect(id: String, state: tauri::State<'_, WinRmState>) -> Result<(), WinRmError> {
    state.sessions.lock().unwrap().remove(&id)
        .ok_or_else(|| WinRmError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn winrm_list(state: tauri::State<'_, WinRmState>) -> Vec<WinRmSession> {
    state.sessions.lock().unwrap().iter().map(|(id, (cfg, shell_id))| WinRmSession {
        id: id.clone(),
        host: cfg.host.clone(),
        username: cfg.username.clone(),
        shell_id: shell_id.clone(),
    }).collect()
}

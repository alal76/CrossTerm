use reqwest::header::{CONTENT_TYPE, AUTHORIZATION, WWW_AUTHENTICATE};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sspi::{
    AuthIdentity, BufferType, ClientRequestFlags, CredentialUse, DataRepresentation, Ntlm,
    SecurityBuffer, SecurityStatus, Sspi, SspiImpl, Username,
};
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
    #[error("Kerberos auth is not yet supported for WinRM — use NTLM or Basic")]
    KerberosUnsupported,
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

fn b64_encode(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
}

fn b64_decode(data: &str) -> Result<Vec<u8>, WinRmError> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|_| WinRmError::AuthFailed)
}

/// Extracts the base64 NTLM challenge (Type 2 message) from a 401's
/// `WWW-Authenticate` header(s) — a server offering multiple schemes sends
/// one header value per scheme, so this has to search all of them rather
/// than assume NTLM is the only or first one present.
fn extract_ntlm_challenge<'a>(header_values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    header_values.filter_map(|v| v.strip_prefix("NTLM ")).find(|v| !v.is_empty())
}

async fn winrm_post_basic(
    client: &reqwest::Client,
    url: &str,
    config: &WinRmConfig,
    body: &str,
) -> Result<(StatusCode, String), WinRmError> {
    let creds = b64_encode(format!("{}:{}", config.username, config.password).as_bytes());
    let resp = client
        .post(url)
        .header(CONTENT_TYPE, "application/soap+xml;charset=UTF-8")
        .header(AUTHORIZATION, format!("Basic {creds}"))
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| WinRmError::Http(e.to_string()))?;

    let status = resp.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(WinRmError::AuthFailed);
    }
    let text = resp.text().await.map_err(|e| WinRmError::Http(e.to_string()))?;
    Ok((status, text))
}

/// Full HTTP NTLM handshake (RFC-less but well-established de facto
/// standard, the same one `requests-ntlm`/pywinrm use): send the Type 1
/// (Negotiate) token, read the Type 2 (Challenge) token back off the 401's
/// `WWW-Authenticate` header, then resend the same request with the Type 3
/// (Authenticate) token. sspi's `Ntlm` computes the actual NTLMv2 response
/// from the username/password locally — no round-trip to a KDC needed,
/// unlike Kerberos.
async fn winrm_post_ntlm(
    client: &reqwest::Client,
    url: &str,
    config: &WinRmConfig,
    body: &str,
) -> Result<(StatusCode, String), WinRmError> {
    let mut ntlm = Ntlm::new();
    let username = Username::parse(&config.username).map_err(|_| WinRmError::AuthFailed)?;
    let identity = AuthIdentity {
        username,
        password: config.password.clone().into(),
    };

    let mut acquired = ntlm
        .acquire_credentials_handle()
        .with_credential_use(CredentialUse::Outbound)
        .with_auth_data(&identity)
        .execute(&mut ntlm)
        .map_err(|e| WinRmError::Http(format!("NTLM: {e}")))?;

    // Leg 1: Negotiate (Type 1)
    let mut output = vec![SecurityBuffer::new(Vec::new(), BufferType::Token)];
    let mut builder = ntlm
        .initialize_security_context()
        .with_credentials_handle(&mut acquired.credentials_handle)
        .with_context_requirements(ClientRequestFlags::CONFIDENTIALITY | ClientRequestFlags::ALLOCATE_MEMORY)
        .with_target_data_representation(DataRepresentation::Native)
        .with_target_name(&config.host)
        .with_output(&mut output);
    ntlm.initialize_security_context_impl(&mut builder)
        .and_then(|mut r| r.resolve_to_result())
        .map_err(|e| WinRmError::Http(format!("NTLM negotiate: {e}")))?;

    let resp = client
        .post(url)
        .header(CONTENT_TYPE, "application/soap+xml;charset=UTF-8")
        .header(AUTHORIZATION, format!("NTLM {}", b64_encode(&output[0].buffer)))
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| WinRmError::Http(e.to_string()))?;

    if resp.status() != StatusCode::UNAUTHORIZED {
        // Server didn't challenge us — treat whatever it sent back as final.
        let status = resp.status();
        let text = resp.text().await.map_err(|e| WinRmError::Http(e.to_string()))?;
        return Ok((status, text));
    }

    let header_values: Vec<&str> = resp
        .headers()
        .get_all(WWW_AUTHENTICATE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    let challenge_b64 = extract_ntlm_challenge(header_values.into_iter()).ok_or(WinRmError::AuthFailed)?;
    let challenge = b64_decode(challenge_b64)?;

    // Leg 2: Authenticate (Type 3)
    let mut input = vec![SecurityBuffer::new(challenge, BufferType::Token)];
    output[0].buffer.clear();
    let mut builder = ntlm
        .initialize_security_context()
        .with_credentials_handle(&mut acquired.credentials_handle)
        .with_context_requirements(ClientRequestFlags::CONFIDENTIALITY | ClientRequestFlags::ALLOCATE_MEMORY)
        .with_target_data_representation(DataRepresentation::Native)
        .with_target_name(&config.host)
        .with_input(&mut input)
        .with_output(&mut output);
    let result = ntlm
        .initialize_security_context_impl(&mut builder)
        .and_then(|mut r| r.resolve_to_result())
        .map_err(|e| WinRmError::Http(format!("NTLM authenticate: {e}")))?;

    if matches!(result.status, SecurityStatus::CompleteAndContinue | SecurityStatus::CompleteNeeded) {
        ntlm.complete_auth_token(&mut output)
            .map_err(|e| WinRmError::Http(format!("NTLM complete: {e}")))?;
    }

    let resp = client
        .post(url)
        .header(CONTENT_TYPE, "application/soap+xml;charset=UTF-8")
        .header(AUTHORIZATION, format!("NTLM {}", b64_encode(&output[0].buffer)))
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| WinRmError::Http(e.to_string()))?;

    let status = resp.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(WinRmError::AuthFailed);
    }
    let text = resp.text().await.map_err(|e| WinRmError::Http(e.to_string()))?;
    Ok((status, text))
}

async fn winrm_post(
    client: &reqwest::Client,
    url: &str,
    config: &WinRmConfig,
    body: &str,
) -> Result<(StatusCode, String), WinRmError> {
    match config.auth {
        WinRmAuth::Basic => winrm_post_basic(client, url, config, body).await,
        WinRmAuth::Ntlm => winrm_post_ntlm(client, url, config, body).await,
        WinRmAuth::Kerberos => Err(WinRmError::KerberosUnsupported),
    }
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn command_envelope(shell_id: &str, command: &str) -> String {
    let command = xml_escape(command);
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

fn receive_envelope(shell_id: &str, command_id: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsmv="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
  <s:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsmv:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</wsmv:ResourceURI>
    <wsmv:SelectorSet><wsmv:Selector Name="ShellId">{shell_id}</wsmv:Selector></wsmv:SelectorSet>
    <wsa:Action>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Receive</wsa:Action>
  </s:Header>
  <s:Body>
    <rsp:Receive><rsp:DesiredStream CommandId="{command_id}">stdout stderr</rsp:DesiredStream></rsp:Receive>
  </s:Body>
</s:Envelope>"#)
}

fn signal_envelope(shell_id: &str, command_id: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsmv="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
  <s:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsmv:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</wsmv:ResourceURI>
    <wsmv:SelectorSet><wsmv:Selector Name="ShellId">{shell_id}</wsmv:Selector></wsmv:SelectorSet>
    <wsa:Action>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Signal</wsa:Action>
  </s:Header>
  <s:Body>
    <rsp:Signal CommandId="{command_id}"><rsp:Code>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/signal/terminate</rsp:Code></rsp:Signal>
  </s:Body>
</s:Envelope>"#)
}

fn delete_shell_envelope(shell_id: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsmv="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
  <s:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsmv:ResourceURI>http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</wsmv:ResourceURI>
    <wsmv:SelectorSet><wsmv:Selector Name="ShellId">{shell_id}</wsmv:Selector></wsmv:SelectorSet>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2004/09/transfer/Delete</wsa:Action>
  </s:Header>
  <s:Body/>
</s:Envelope>"#)
}

fn parse_command_id(xml: &str) -> Option<String> {
    xml.split("<rsp:CommandId>")
        .nth(1)
        .and_then(|s| s.split("</rsp:CommandId>").next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn is_command_done(xml: &str) -> bool {
    xml.contains("CommandState/Done")
}

fn extract_exit_code(xml: &str) -> Option<i32> {
    xml.split("<rsp:ExitCode>")
        .nth(1)
        .and_then(|s| s.split("</rsp:ExitCode>").next())
        .and_then(|s| s.trim().parse().ok())
}

/// A Receive response can carry several `<rsp:Stream>` elements for the
/// same named stream (one per chunk the server flushed), so this walks all
/// of them rather than assuming one match — same reasoning as
/// `extract_ntlm_challenge` scanning every `WWW-Authenticate` header.
fn extract_stream_chunks(xml: &str, stream_name: &str) -> Vec<u8> {
    let marker = format!("Name=\"{stream_name}\"");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(tag_start) = rest.find("<rsp:Stream ") {
        let tail = &rest[tag_start..];
        let Some(tag_end) = tail.find('>') else { break };
        let tag = &tail[..=tag_end];
        if tag.contains(&marker) && !tag.ends_with("/>") {
            let content = &tail[tag_end + 1..];
            if let Some(close) = content.find("</rsp:Stream>") {
                let b64 = content[..close].trim();
                if !b64.is_empty() {
                    if let Ok(mut bytes) = b64_decode(b64) {
                        out.append(&mut bytes);
                    }
                }
            }
        }
        rest = &tail[tag_end + 1..];
    }
    out
}

#[tauri::command]
pub async fn winrm_connect(
    config: WinRmConfig,
    state: tauri::State<'_, WinRmState>,
) -> Result<String, WinRmError> {
    let client = build_client(&config)?;
    let url = endpoint_url(&config);

    let (status, body) = winrm_post(&client, &url, &config, &create_shell_envelope()).await?;

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

    // The Command response only hands back a CommandId — actual output has
    // to be pulled with separate Receive requests, polled until the shell
    // reports the command Done (mirrors how PowerShell's WinRM client and
    // pywinrm both drive this exchange).
    let (status, body) = winrm_post(&client, &url, &config, &command_envelope(&shell_id, &command)).await?;
    if !status.is_success() {
        return Err(WinRmError::CommandFailed(format!("{status}: {body}")));
    }
    let command_id = parse_command_id(&body)
        .ok_or_else(|| WinRmError::Xml("Command response had no CommandId".into()))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code = 0;
    const MAX_POLLS: usize = 30;
    for attempt in 0..MAX_POLLS {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        let (status, body) = winrm_post(&client, &url, &config, &receive_envelope(&shell_id, &command_id)).await?;
        if !status.is_success() {
            return Err(WinRmError::CommandFailed(format!("{status}: {body}")));
        }
        stdout.extend(extract_stream_chunks(&body, "stdout"));
        stderr.extend(extract_stream_chunks(&body, "stderr"));
        if let Some(code) = extract_exit_code(&body) {
            exit_code = code;
        }
        if is_command_done(&body) {
            break;
        }
    }

    // Best-effort cleanup — a failed Signal shouldn't hide a command that
    // already produced real output.
    let _ = winrm_post(&client, &url, &config, &signal_envelope(&shell_id, &command_id)).await;

    Ok(WinRmCommandResult {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_code,
    })
}

#[tauri::command]
pub async fn winrm_disconnect(id: String, state: tauri::State<'_, WinRmState>) -> Result<(), WinRmError> {
    let (config, shell_id) = state
        .sessions.lock().unwrap()
        .remove(&id)
        .ok_or(WinRmError::NotFound(id))?;
    let client = build_client(&config)?;
    let url = endpoint_url(&config);
    // Best-effort — the local session is already gone either way.
    let _ = winrm_post(&client, &url, &config, &delete_shell_envelope(&shell_id)).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_url_scheme_follows_use_tls() {
        let mut config = WinRmConfig {
            host: "10.0.0.5".into(), port: 5985, username: "admin".into(), password: "hunter2".into(),
            use_tls: false, auth: WinRmAuth::Basic, verify_tls: true,
        };
        assert_eq!(endpoint_url(&config), "http://10.0.0.5:5985/wsman");
        config.use_tls = true;
        config.port = 5986;
        assert_eq!(endpoint_url(&config), "https://10.0.0.5:5986/wsman");
    }

    #[test]
    fn test_b64_roundtrip() {
        let encoded = b64_encode(b"hello ntlm");
        assert_eq!(b64_decode(&encoded).unwrap(), b"hello ntlm");
    }

    #[test]
    fn test_extract_ntlm_challenge_finds_ntlm_among_multiple_schemes() {
        let headers = vec!["Negotiate", "NTLM TlRMTVNTUAACAAAA"];
        assert_eq!(extract_ntlm_challenge(headers.into_iter()), Some("TlRMTVNTUAACAAAA"));
    }

    #[test]
    fn test_extract_ntlm_challenge_none_when_absent() {
        let headers = vec!["Negotiate", "Basic realm=\"WSMAN\""];
        assert_eq!(extract_ntlm_challenge(headers.into_iter()), None);
    }

    #[test]
    fn test_extract_ntlm_challenge_ignores_bare_ntlm_negotiate_header() {
        // The first leg's own 401 (before we've sent anything) can carry a
        // bare "NTLM" with no token — must not be mistaken for a challenge.
        let headers = vec!["NTLM"];
        assert_eq!(extract_ntlm_challenge(headers.into_iter()), None);
    }

    #[test]
    fn test_command_envelope_includes_shell_id_and_command() {
        let xml = command_envelope("shell-123", "ipconfig /all");
        assert!(xml.contains("shell-123"));
        assert!(xml.contains("ipconfig /all"));
    }

    #[test]
    fn test_create_shell_envelope_declares_stdin_stdout_stderr_streams() {
        let xml = create_shell_envelope();
        assert!(xml.contains("<rsp:InputStreams>stdin</rsp:InputStreams>"));
        assert!(xml.contains("<rsp:OutputStreams>stdout stderr</rsp:OutputStreams>"));
    }

    #[test]
    fn test_command_envelope_escapes_xml_special_characters() {
        let xml = command_envelope("shell-1", r#"echo "<a & b>""#);
        assert!(xml.contains("echo &quot;&lt;a &amp; b&gt;&quot;"));
        assert!(!xml.contains(r#""<a & b>""#));
    }

    #[test]
    fn test_parse_command_id() {
        let xml = r#"<rsp:CommandResponse><rsp:CommandId>ABC-123</rsp:CommandId></rsp:CommandResponse>"#;
        assert_eq!(parse_command_id(xml), Some("ABC-123".to_string()));
    }

    #[test]
    fn test_parse_command_id_missing() {
        assert_eq!(parse_command_id("<rsp:CommandResponse/>"), None);
    }

    #[test]
    fn test_is_command_done() {
        assert!(is_command_done(r#"<rsp:CommandState State="http://schemas.microsoft.com/wbem/wsman/1/windows/shell/CommandState/Done">"#));
        assert!(!is_command_done(r#"<rsp:CommandState State="http://schemas.microsoft.com/wbem/wsman/1/windows/shell/CommandState/Running">"#));
    }

    #[test]
    fn test_extract_exit_code() {
        let xml = r#"<rsp:CommandState><rsp:ExitCode>0</rsp:ExitCode></rsp:CommandState>"#;
        assert_eq!(extract_exit_code(xml), Some(0));
        assert_eq!(extract_exit_code("<rsp:CommandState/>"), None);
    }

    #[test]
    fn test_extract_stream_chunks_concatenates_multiple_chunks_for_the_named_stream() {
        // "hello " -> "aGVsbG8g", "world" -> "d29ybGQ="
        let xml = r#"
            <rsp:Stream Name="stdout" CommandId="c1">aGVsbG8g</rsp:Stream>
            <rsp:Stream Name="stderr" CommandId="c1">ZXJyb3I=</rsp:Stream>
            <rsp:Stream Name="stdout" CommandId="c1">d29ybGQ=</rsp:Stream>
            <rsp:Stream Name="stdout" CommandId="c1" End="true"/>
        "#;
        assert_eq!(String::from_utf8(extract_stream_chunks(xml, "stdout")).unwrap(), "hello world");
        assert_eq!(String::from_utf8(extract_stream_chunks(xml, "stderr")).unwrap(), "error");
    }

    #[test]
    fn test_extract_stream_chunks_empty_when_stream_absent() {
        assert_eq!(extract_stream_chunks("<rsp:ReceiveResponse/>", "stdout"), Vec::<u8>::new());
    }
}

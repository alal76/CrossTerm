/// Proxmox VE console access: authenticates against the Proxmox REST API,
/// requests a VNC proxy ticket for a QEMU VM or LXC container, then bridges
/// the resulting `vncwebsocket` connection into a plain duplex stream so it
/// can be handed to the same VNC client machinery used for direct VNC
/// connections (see `crate::vnc::connect_stream`). This lets the existing
/// `VncViewer.tsx` frontend work unchanged — Proxmox's console is just VNC
/// tunneled inside a WebSocket.
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_tungstenite::tungstenite::Message;

use crate::vnc::{self, VncConnectResult, VncError, VncState};

#[derive(Debug, Error)]
pub enum ProxmoxError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Request error: {0}")]
    Request(String),
    #[error("Unexpected API response: {0}")]
    Response(String),
    #[error("WebSocket error: {0}")]
    Ws(String),
    #[error("VNC error: {0}")]
    Vnc(String),
}

impl From<VncError> for ProxmoxError {
    fn from(e: VncError) -> Self {
        ProxmoxError::Vnc(e.to_string())
    }
}

impl Serialize for ProxmoxError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxmoxResourceType {
    Qemu,
    Lxc,
}

impl ProxmoxResourceType {
    fn api_segment(self) -> &'static str {
        match self {
            ProxmoxResourceType::Qemu => "qemu",
            ProxmoxResourceType::Lxc => "lxc",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxmoxConfig {
    pub host: String,
    /// Default 8006, the standard Proxmox VE API/UI port.
    pub port: u16,
    pub node: String,
    pub vmid: String,
    pub resource_type: ProxmoxResourceType,
    pub username: String,
    /// PAM/PVE realm, e.g. "pam" or "pve". Not included in `username`.
    pub realm: String,
    pub password: String,
    /// Accept the self-signed certificates Proxmox hosts commonly present.
    pub verify_tls: bool,
}

/// Proxmox's API returns `port` as either a JSON string or a JSON number
/// depending on version/endpoint, so accept both rather than guessing.
fn deserialize_port<'de, D>(d: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PortField {
        Num(u16),
        Str(String),
    }
    match PortField::deserialize(d)? {
        PortField::Num(n) => Ok(n),
        PortField::Str(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Deserialize)]
struct TicketResponse {
    data: TicketData,
}

#[derive(Debug, Deserialize)]
struct TicketData {
    ticket: String,
    #[serde(rename = "CSRFPreventionToken")]
    csrf: String,
}

#[derive(Debug, Deserialize)]
struct VncProxyResponse {
    data: VncProxyData,
}

#[derive(Debug, Deserialize)]
struct VncProxyData {
    ticket: String,
    #[serde(deserialize_with = "deserialize_port")]
    port: u16,
}

fn build_client(verify_tls: bool) -> Result<reqwest::Client, ProxmoxError> {
    let mut b = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));
    if !verify_tls {
        b = b.danger_accept_invalid_certs(true);
    }
    b.build().map_err(|e| ProxmoxError::Request(e.to_string()))
}

/// Percent-encodes a string for safe inclusion in a URL query component.
/// Proxmox tickets contain `:`, `=`, `+`, `/` which all need escaping here.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn authenticate(
    client: &reqwest::Client,
    base: &str,
    config: &ProxmoxConfig,
) -> Result<TicketData, ProxmoxError> {
    let resp = client
        .post(format!("{base}/api2/json/access/ticket"))
        .form(&[
            ("username", format!("{}@{}", config.username, config.realm)),
            ("password", config.password.clone()),
        ])
        .send()
        .await
        .map_err(|e| ProxmoxError::Request(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(ProxmoxError::AuthFailed(format!("HTTP {}", resp.status())));
    }

    let body: TicketResponse = resp
        .json()
        .await
        .map_err(|e| ProxmoxError::Response(e.to_string()))?;
    Ok(body.data)
}

async fn request_vnc_proxy(
    client: &reqwest::Client,
    base: &str,
    config: &ProxmoxConfig,
    ticket: &TicketData,
) -> Result<VncProxyData, ProxmoxError> {
    let url = format!(
        "{base}/api2/json/nodes/{}/{}/{}/vncproxy",
        config.node,
        config.resource_type.api_segment(),
        config.vmid
    );

    let resp = client
        .post(url)
        .header("Cookie", format!("PVEAuthCookie={}", ticket.ticket))
        .header("CSRFPreventionToken", ticket.csrf.clone())
        .form(&[("websocket", "1")])
        .send()
        .await
        .map_err(|e| ProxmoxError::Request(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(ProxmoxError::Response(format!("vncproxy HTTP {}", resp.status())));
    }

    let body: VncProxyResponse = resp
        .json()
        .await
        .map_err(|e| ProxmoxError::Response(e.to_string()))?;
    Ok(body.data)
}

#[derive(Debug)]
struct NoCertVerifier;

impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer,
        _intermediates: &[rustls::pki_types::CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[tauri::command]
pub async fn proxmox_console_connect(
    config: ProxmoxConfig,
    vnc_state: tauri::State<'_, VncState>,
    app: tauri::AppHandle,
) -> Result<VncConnectResult, ProxmoxError> {
    let base = format!("https://{}:{}", config.host, config.port);
    let client = build_client(config.verify_tls)?;

    let ticket = authenticate(&client, &base, &config).await?;
    let vnc_proxy = request_vnc_proxy(&client, &base, &config, &ticket).await?;

    let ws_url = format!(
        "wss://{}:{}/api2/json/nodes/{}/{}/{}/vncwebsocket?port={}&vncticket={}",
        config.host,
        config.port,
        config.node,
        config.resource_type.api_segment(),
        config.vmid,
        vnc_proxy.port,
        percent_encode(&vnc_proxy.ticket),
    );

    // TODO: honour verify_tls with the system CA bundle; Proxmox hosts
    // overwhelmingly run self-signed certs so a permissive verifier is used
    // for now (same tradeoff already made in websocket_term::wsterm_connect).
    let tls_config = std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(NoCertVerifier))
            .with_no_client_auth(),
    );
    let connector = tokio_tungstenite::Connector::Rustls(tls_config);

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(&ws_url)
        .header("Cookie", format!("PVEAuthCookie={}", ticket.ticket))
        .body(())
        .map_err(|e| ProxmoxError::Ws(e.to_string()))?;

    let (ws_stream, _) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector))
            .await
            .map_err(|e| ProxmoxError::Ws(e.to_string()))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // Bridge the WebSocket to a plain duplex stream so it looks like any
    // other AsyncRead+AsyncWrite to the shared VNC connection helper.
    let (vnc_side, bridge_side) = tokio::io::duplex(64 * 1024);
    let (mut bridge_read, mut bridge_write) = tokio::io::split(bridge_side);

    // WebSocket -> duplex (bytes flowing from Proxmox towards the VNC client)
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if bridge_write.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    });

    // duplex -> WebSocket (bytes flowing from the VNC client towards Proxmox)
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 16 * 1024];
        loop {
            match bridge_read.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_write.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let result = vnc::connect_stream(
        &vnc_state,
        &app,
        vnc_side,
        config.host.clone(),
        vnc_proxy.port,
        Some(vnc_proxy.ticket),
    )
    .await?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_escapes_ticket_special_chars() {
        let ticket = "PVEVNC:user@pam:65AB12CD::abc+def/ghi==";
        let encoded = percent_encode(ticket);
        assert!(!encoded.contains(':'));
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
        assert!(!encoded.contains('@'));
        // Round-trippable structure: no spaces introduced, alnum preserved.
        assert!(encoded.starts_with("PVEVNC%3A"));
    }

    #[test]
    fn percent_encode_leaves_unreserved_chars_untouched() {
        assert_eq!(percent_encode("abcXYZ019-_.~"), "abcXYZ019-_.~");
    }

    #[test]
    fn deserialize_port_accepts_string_or_number() {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(deserialize_with = "deserialize_port")]
            port: u16,
        }
        let from_str: Wrapper = serde_json::from_str(r#"{"port":"5901"}"#).unwrap();
        assert_eq!(from_str.port, 5901);
        let from_num: Wrapper = serde_json::from_str(r#"{"port":5901}"#).unwrap();
        assert_eq!(from_num.port, 5901);
    }

    #[test]
    fn api_segment_matches_proxmox_url_scheme() {
        assert_eq!(ProxmoxResourceType::Qemu.api_segment(), "qemu");
        assert_eq!(ProxmoxResourceType::Lxc.api_segment(), "lxc");
    }

    #[test]
    fn ticket_response_deserializes_from_real_shaped_json() {
        let json = r#"{"data":{"ticket":"PVE:user@pam:...","CSRFPreventionToken":"abc123:def"}}"#;
        let resp: TicketResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.csrf, "abc123:def");
    }
}

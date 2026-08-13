use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WsTermError {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("WebSocket error: {0}")]
    Ws(String),
    #[error("URL parse error: {0}")]
    UrlParse(String),
}

impl Serialize for WsTermError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsTermConfig {
    /// Full WebSocket URL, e.g. ws://host:7681 or wss://host/terminal
    pub url: String,
    /// Optional Bearer token for ttyd/gotty auth
    pub token: Option<String>,
    pub verify_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsTermSession {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsTermData {
    pub session_id: String,
    pub data: String,
}

pub struct WsTermState {
    sessions: Mutex<HashMap<String, WsTermSession>>,
    // Write-half senders keyed by session id
    senders: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>,
}

impl WsTermState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            senders: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn wsterm_connect(
    config: WsTermConfig,
    state: tauri::State<'_, WsTermState>,
    app: AppHandle,
) -> Result<String, WsTermError> {
    let url_str = config.url.clone();
    let url = url_str.parse::<tokio_tungstenite::tungstenite::http::Uri>()
        .map_err(|e| WsTermError::UrlParse(e.to_string()))?;

    let connector = if config.verify_tls {
        tokio_tungstenite::Connector::Rustls(
            std::sync::Arc::new(
                rustls::ClientConfig::builder()
                    .with_native_roots()
                    .map_err(|e| WsTermError::Ws(e.to_string()))?
                    .with_no_client_auth()
            )
        )
    } else {
        tokio_tungstenite::Connector::Rustls(
            std::sync::Arc::new(
                rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(std::sync::Arc::new(NoCertVerifier))
                    .with_no_client_auth()
            )
        )
    };

    let request = {
        let mut req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&url_str);
        if let Some(tok) = &config.token {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
        req.body(()).map_err(|e| WsTermError::Ws(e.to_string()))?
    };

    let (ws_stream, _) = tokio_tungstenite::connect_async_tls_with_config(
        request, None, false, Some(connector),
    ).await.map_err(|e| WsTermError::Ws(e.to_string()))?;

    let (mut write, mut read) = ws_stream.split();
    let id = Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Forward outbound messages to WebSocket
    let id_clone = id.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Forward inbound WebSocket messages as Tauri events
    let app_clone = app.clone();
    let id_clone2 = id.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                Message::Close(_) => break,
                _ => continue,
            };
            let _ = app_clone.emit("wsterm:data", WsTermData {
                session_id: id_clone2.clone(),
                data: text,
            });
        }
    });

    let session = WsTermSession { id: id.clone(), url: url_str };
    state.sessions.lock().unwrap().insert(id.clone(), session);
    state.senders.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn wsterm_send(
    id: String,
    data: String,
    state: tauri::State<'_, WsTermState>,
) -> Result<(), WsTermError> {
    let senders = state.senders.lock().unwrap();
    let tx = senders.get(&id).ok_or_else(|| WsTermError::NotFound(id.clone()))?;
    tx.send(data).map_err(|e| WsTermError::Ws(e.to_string()))
}

#[tauri::command]
pub fn wsterm_disconnect(id: String, state: tauri::State<'_, WsTermState>) -> Result<(), WsTermError> {
    state.sessions.lock().unwrap().remove(&id)
        .ok_or_else(|| WsTermError::NotFound(id.clone()))?;
    state.senders.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn wsterm_list(state: tauri::State<'_, WsTermState>) -> Vec<WsTermSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

// Permissive TLS verifier for self-signed BMC/appliance certificates
struct NoCertVerifier;

impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self, _end_entity: &rustls::pki_types::CertificateDer,
        _intermediates: &[rustls::pki_types::CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

use futures::{SinkExt, StreamExt};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsTermDisconnected {
    pub session_id: String,
    pub reason: String,
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
    let _url = url_str.parse::<tokio_tungstenite::tungstenite::http::Uri>()
        .map_err(|e| WsTermError::UrlParse(e.to_string()))?;

    // TODO: honour verify_tls with system CA bundle; permissive verifier used for initial bringup
    let tls_config = std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(NoCertVerifier))
            .with_no_client_auth()
    );
    let connector = tokio_tungstenite::Connector::Rustls(tls_config);

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
    let _id_clone = id.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Forward inbound WebSocket messages as Tauri events
    let app_clone = app.clone();
    let id_clone2 = id.clone();
    tokio::spawn(async move {
        let mut close_reason = "connection closed".to_string();
        loop {
            match read.next().await {
                Some(Ok(msg)) => {
                    let text = match msg {
                        Message::Text(t) => t.to_string(),
                        Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                        Message::Close(frame) => {
                            if let Some(f) = frame {
                                close_reason = format!("{}: {}", f.code, f.reason);
                            }
                            break;
                        }
                        _ => continue,
                    };
                    let _ = app_clone.emit("wsterm:data", WsTermData {
                        session_id: id_clone2.clone(),
                        data: text,
                    });
                }
                Some(Err(e)) => {
                    close_reason = e.to_string();
                    break;
                }
                None => break,
            }
        }
        let _ = app_clone.emit("wsterm:disconnected", WsTermDisconnected {
            session_id: id_clone2.clone(),
            reason: close_reason,
        });
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
    do_wsterm_send(id, data, state.inner())
}

fn do_wsterm_send(id: String, data: String, state: &WsTermState) -> Result<(), WsTermError> {
    let senders = state.senders.lock().unwrap();
    let tx = senders.get(&id).ok_or_else(|| WsTermError::NotFound(id.clone()))?;
    tx.send(data).map_err(|e| WsTermError::Ws(e.to_string()))
}

#[tauri::command]
pub fn wsterm_disconnect(id: String, state: tauri::State<'_, WsTermState>) -> Result<(), WsTermError> {
    do_wsterm_disconnect(id, state.inner())
}

fn do_wsterm_disconnect(id: String, state: &WsTermState) -> Result<(), WsTermError> {
    state.sessions.lock().unwrap().remove(&id)
        .ok_or_else(|| WsTermError::NotFound(id.clone()))?;
    state.senders.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn wsterm_list(state: tauri::State<'_, WsTermState>) -> Vec<WsTermSession> {
    do_wsterm_list(state.inner())
}

fn do_wsterm_list(state: &WsTermState) -> Vec<WsTermSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_term_error_serializes_as_display_string() {
        let err = WsTermError::NotFound("abc".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Connection not found: abc\"");

        let err = WsTermError::Ws("boom".into());
        assert_eq!(serde_json::to_string(&err).unwrap(), "\"WebSocket error: boom\"");

        let err = WsTermError::UrlParse("bad url".into());
        assert_eq!(
            serde_json::to_string(&err).unwrap(),
            "\"URL parse error: bad url\""
        );
    }

    #[test]
    fn test_wsterm_config_serde_roundtrip() {
        let cfg = WsTermConfig {
            url: "wss://host:7681".into(),
            token: Some("tok123".into()),
            verify_tls: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: WsTermConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.url, "wss://host:7681");
        assert_eq!(parsed.token.as_deref(), Some("tok123"));
        assert!(parsed.verify_tls);
    }

    #[test]
    fn test_wsterm_state_new_is_empty() {
        let state = WsTermState::new();
        assert!(state.sessions.lock().unwrap().is_empty());
        assert!(state.senders.lock().unwrap().is_empty());
        assert!(do_wsterm_list(&state).is_empty());
    }

    #[test]
    fn test_wsterm_send_not_found() {
        let state = WsTermState::new();
        let err = do_wsterm_send("nope".into(), "data".into(), &state).unwrap_err();
        assert!(matches!(err, WsTermError::NotFound(_)));
    }

    #[test]
    fn test_wsterm_send_success_delivers_to_channel() {
        let state = WsTermState::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        state.senders.lock().unwrap().insert("s1".into(), tx);

        do_wsterm_send("s1".into(), "hello".into(), &state).expect("send ok");
        let received = rx.try_recv().expect("message queued");
        assert_eq!(received, "hello");
    }

    #[test]
    fn test_wsterm_send_after_receiver_dropped_errors() {
        let state = WsTermState::new();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        drop(rx);
        state.senders.lock().unwrap().insert("s1".into(), tx);

        let err = do_wsterm_send("s1".into(), "hello".into(), &state).unwrap_err();
        assert!(matches!(err, WsTermError::Ws(_)));
    }

    #[test]
    fn test_wsterm_disconnect_removes_session_and_sender() {
        let state = WsTermState::new();
        state.sessions.lock().unwrap().insert(
            "s1".into(),
            WsTermSession {
                id: "s1".into(),
                url: "ws://x".into(),
            },
        );
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        state.senders.lock().unwrap().insert("s1".into(), tx);

        do_wsterm_disconnect("s1".into(), &state).expect("disconnect ok");
        assert!(!state.sessions.lock().unwrap().contains_key("s1"));
        assert!(!state.senders.lock().unwrap().contains_key("s1"));
    }

    #[test]
    fn test_wsterm_disconnect_not_found() {
        let state = WsTermState::new();
        let err = do_wsterm_disconnect("nope".into(), &state).unwrap_err();
        assert!(matches!(err, WsTermError::NotFound(_)));
    }

    #[test]
    fn test_wsterm_list_returns_all_sessions() {
        let state = WsTermState::new();
        state.sessions.lock().unwrap().insert(
            "s1".into(),
            WsTermSession {
                id: "s1".into(),
                url: "ws://a".into(),
            },
        );
        state.sessions.lock().unwrap().insert(
            "s2".into(),
            WsTermSession {
                id: "s2".into(),
                url: "ws://b".into(),
            },
        );
        let list = do_wsterm_list(&state);
        assert_eq!(list.len(), 2);
        let mut ids: Vec<_> = list.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn test_no_cert_verifier_accepts_any_cert() {
        use rustls::client::danger::ServerCertVerifier;
        let verifier = NoCertVerifier;
        let cert = rustls::pki_types::CertificateDer::from(vec![0u8; 4]);
        let server_name = rustls::pki_types::ServerName::try_from("example.com").unwrap();
        let result = verifier.verify_server_cert(
            &cert,
            &[],
            &server_name,
            &[],
            rustls::pki_types::UnixTime::now(),
        );
        assert!(result.is_ok());
        assert!(!verifier.supported_verify_schemes().is_empty());
    }
}

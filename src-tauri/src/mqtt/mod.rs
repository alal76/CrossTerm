/// MQTT v3.1.1 / v5 client using rumqttc.
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Publish error: {0}")]
    Publish(String),
    #[error("Subscribe error: {0}")]
    Subscribe(String),
}

impl Serialize for MqttError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MqttQos {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl From<MqttQos> for QoS {
    fn from(q: MqttQos) -> Self {
        match q {
            MqttQos::AtMostOnce => QoS::AtMostOnce,
            MqttQos::AtLeastOnce => QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => QoS::ExactlyOnce,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub host: String,
    /// 1883 = plain, 8883 = TLS
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive_secs: u16,
    pub use_tls: bool,
    pub clean_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttSession {
    pub id: String,
    pub host: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttMessage {
    pub session_id: String,
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
}

pub struct MqttState {
    sessions: Mutex<HashMap<String, (MqttSession, AsyncClient)>>,
}

impl MqttState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn mqtt_connect(
    config: MqttConfig,
    state: tauri::State<'_, MqttState>,
    app: AppHandle,
) -> Result<String, MqttError> {
    let mut opts = MqttOptions::new(
        &config.client_id,
        &config.host,
        config.port,
    );
    opts.set_keep_alive(std::time::Duration::from_secs(config.keep_alive_secs as u64));
    opts.set_clean_session(config.clean_session);

    if let Some(u) = &config.username {
        opts.set_credentials(u, config.password.as_deref().unwrap_or(""));
    }

    if config.use_tls {
        // Use native-roots TLS configuration
        opts.set_transport(rumqttc::Transport::Tls(rumqttc::TlsConfiguration::Native));
    }

    let (client, mut event_loop) = AsyncClient::new(opts, 64);
    let id = Uuid::new_v4().to_string();

    let id_clone = id.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        loop {
            match event_loop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                    let payload = String::from_utf8_lossy(&publish.payload).to_string();
                    let _ = app_clone.emit("mqtt:message", MqttMessage {
                        session_id: id_clone.clone(),
                        topic: publish.topic.clone(),
                        payload,
                        qos: publish.qos as u8,
                        retain: publish.retain,
                    });
                }
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Disconnect)) | Err(_) => break,
                _ => {}
            }
        }
    });

    let session = MqttSession {
        id: id.clone(),
        host: config.host.clone(),
        client_id: config.client_id.clone(),
    };
    state.sessions.lock().unwrap().insert(id.clone(), (session, client));
    Ok(id)
}

#[tauri::command]
pub async fn mqtt_subscribe(
    id: String,
    topic: String,
    qos: MqttQos,
    state: tauri::State<'_, MqttState>,
) -> Result<(), MqttError> {
    let client = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&id).map(|(_, c)| c.clone()).ok_or_else(|| MqttError::NotFound(id.clone()))?
    };
    client.subscribe(&topic, qos.into()).await
        .map_err(|e| MqttError::Subscribe(e.to_string()))
}

#[tauri::command]
pub async fn mqtt_publish(
    id: String,
    topic: String,
    payload: String,
    qos: MqttQos,
    retain: bool,
    state: tauri::State<'_, MqttState>,
) -> Result<(), MqttError> {
    let client = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&id).map(|(_, c)| c.clone()).ok_or_else(|| MqttError::NotFound(id.clone()))?
    };
    client.publish(&topic, qos.into(), retain, payload.into_bytes()).await
        .map_err(|e| MqttError::Publish(e.to_string()))
}

#[tauri::command]
pub async fn mqtt_unsubscribe(
    id: String,
    topic: String,
    state: tauri::State<'_, MqttState>,
) -> Result<(), MqttError> {
    let client = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&id).map(|(_, c)| c.clone()).ok_or_else(|| MqttError::NotFound(id.clone()))?
    };
    client.unsubscribe(&topic).await
        .map_err(|e| MqttError::Subscribe(e.to_string()))
}

#[tauri::command]
pub async fn mqtt_disconnect(
    id: String,
    state: tauri::State<'_, MqttState>,
) -> Result<(), MqttError> {
    let client = {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.remove(&id).map(|(_, c)| c).ok_or_else(|| MqttError::NotFound(id.clone()))?
    };
    client.disconnect().await.map_err(|e| MqttError::Connection(e.to_string()))
}

#[tauri::command]
pub fn mqtt_list_sessions(state: tauri::State<'_, MqttState>) -> Vec<MqttSession> {
    state.sessions.lock().unwrap().values().map(|(s, _)| s.clone()).collect()
}

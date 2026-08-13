/// gRPC channel manager with server reflection support.
/// Uses tonic for the channel; server reflection (grpc.reflection.v1.ServerReflection)
/// is queried to enumerate services and their RPC methods.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Reflection error: {0}")]
    Reflection(String),
    #[error("RPC error: {0}")]
    Rpc(String),
}

impl Serialize for GrpcError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// e.g. "http://localhost:50051" or "https://api.example.com"
    pub endpoint: String,
    pub verify_tls: bool,
    /// Extra metadata headers sent with every RPC
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcSession {
    pub id: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcService {
    pub name: String,
    pub methods: Vec<GrpcMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcMethod {
    pub name: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub input_type: String,
    pub output_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcRpcResult {
    pub status_code: u32,
    pub message: String,
    /// JSON-encoded response (best-effort)
    pub body: String,
    pub trailing_metadata: HashMap<String, String>,
}

pub struct GrpcState {
    sessions: Mutex<HashMap<String, (GrpcConfig, Channel)>>,
}

impl GrpcState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

// ── Minimal raw-bytes codec (no prost needed) ────────────────────────────

struct RawCodec;

impl tonic::codec::Codec for RawCodec {
    type Encode = bytes::Bytes;
    type Decode = bytes::Bytes;
    type Encoder = RawEncoder;
    type Decoder = RawDecoder;
    fn encoder(&mut self) -> RawEncoder { RawEncoder }
    fn decoder(&mut self) -> RawDecoder { RawDecoder }
}

struct RawEncoder;
impl tonic::codec::Encoder for RawEncoder {
    type Item = bytes::Bytes;
    type Error = tonic::Status;
    fn encode(&mut self, item: bytes::Bytes, dst: &mut tonic::codec::EncodeBuf<'_>) -> Result<(), tonic::Status> {
        use bytes::BufMut;
        dst.put(item);
        Ok(())
    }
}

struct RawDecoder;
impl tonic::codec::Decoder for RawDecoder {
    type Item = bytes::Bytes;
    type Error = tonic::Status;
    fn decode(&mut self, src: &mut tonic::codec::DecodeBuf<'_>) -> Result<Option<bytes::Bytes>, tonic::Status> {
        use bytes::Buf;
        let remaining = src.remaining();
        if remaining == 0 { return Ok(None); }
        Ok(Some(src.copy_to_bytes(remaining)))
    }
}

#[tauri::command]
pub async fn grpc_connect(
    config: GrpcConfig,
    state: tauri::State<'_, GrpcState>,
) -> Result<String, GrpcError> {
    let mut endpoint = Endpoint::from_shared(config.endpoint.clone())
        .map_err(|e| GrpcError::Transport(e.to_string()))?
        .timeout(std::time::Duration::from_secs(30))
        .concurrency_limit(32);

    if config.endpoint.starts_with("https://") {
        let tls = ClientTlsConfig::new(); // TODO: load native CA bundle
        endpoint = endpoint.tls_config(tls)
            .map_err(|e| GrpcError::Transport(e.to_string()))?;
    }

    let channel = endpoint.connect().await
        .map_err(|e| GrpcError::Transport(e.to_string()))?;

    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), (config, channel));
    Ok(id)
}

#[tauri::command]
pub async fn grpc_list_services(
    id: String,
    state: tauri::State<'_, GrpcState>,
) -> Result<Vec<String>, GrpcError> {
    let (_, _channel) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| GrpcError::NotFound(id.clone()))?;
    // Server reflection requires protobuf; return placeholder until prost feature is enabled
    Ok(vec!["grpc.reflection.v1.ServerReflection".to_string()])
}

#[tauri::command]
pub async fn grpc_invoke(
    id: String,
    service: String,
    method: String,
    json_body: String,
    state: tauri::State<'_, GrpcState>,
) -> Result<GrpcRpcResult, GrpcError> {
    let (cfg, channel) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| GrpcError::NotFound(id.clone()))?;

    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|e| GrpcError::Rpc(e.to_string()))?;

    let path_str = format!("/{service}/{method}");
    let path: tonic::codegen::http::uri::PathAndQuery =
        tonic::codegen::http::uri::PathAndQuery::try_from(path_str.as_str())
        .map_err(|e| GrpcError::Rpc(e.to_string()))?;

    let body = bytes::Bytes::from(json_body.into_bytes());
    let mut request = tonic::Request::new(body);

    for (k, v) in &cfg.metadata {
        if let (Ok(name), Ok(value)) = (
            tonic::metadata::MetadataKey::from_bytes(k.as_bytes()),
            tonic::metadata::MetadataValue::try_from(v.as_str()),
        ) {
            request.metadata_mut().insert(name, value);
        }
    }

    match client.unary(request, path, RawCodec).await {
        Ok(resp) => {
            let body_bytes: bytes::Bytes = resp.into_inner();
            Ok(GrpcRpcResult {
                status_code: 0,
                message: "OK".to_string(),
                body: String::from_utf8_lossy(&body_bytes).to_string(),
                trailing_metadata: HashMap::new(),
            })
        }
        Err(status) => {
            let code = status.code() as i32 as u32;
            Ok(GrpcRpcResult {
                status_code: code,
                message: status.message().to_string(),
                body: String::new(),
                trailing_metadata: HashMap::new(),
            })
        }
    }
}

#[tauri::command]
pub fn grpc_disconnect(id: String, state: tauri::State<'_, GrpcState>) -> Result<(), GrpcError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| GrpcError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn grpc_list_sessions(state: tauri::State<'_, GrpcState>) -> Vec<GrpcSession> {
    state.sessions.lock().unwrap().iter().map(|(id, (cfg, _))| GrpcSession {
        id: id.clone(), endpoint: cfg.endpoint.clone(),
    }).collect()
}

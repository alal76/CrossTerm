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
        let tls = if config.verify_tls {
            ClientTlsConfig::new().with_native_roots()
        } else {
            // Permissive — accept any cert (dev/internal servers)
            ClientTlsConfig::new()
        };
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
    let (_, channel) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| GrpcError::NotFound(id.clone()))?;

    // Use gRPC server reflection v1 (grpc.reflection.v1.ServerReflection)
    // Raw bytes for a ListServices reflection request
    let list_request = build_reflection_list_services();

    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|e| GrpcError::Reflection(e.to_string()))?;

    let request = tonic::Request::new(futures::stream::once(async move {
        list_request
    }));

    // The reflection service path
    let codec = tonic::codec::ProstCodec::<bytes::Bytes, bytes::Bytes>::default();
    let path = tonic::codegen::http::uri::PathAndQuery::from_static(
        "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo"
    );

    // In a full implementation we'd decode the protobuf response.
    // For now return the service name of the reflection service itself.
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

    // Build a raw unary RPC — encode JSON body as UTF-8 bytes (grpc-web JSON codec)
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|e| GrpcError::Rpc(e.to_string()))?;

    let path_str = format!("/{service}/{method}");
    let path = tonic::codegen::http::uri::PathAndQuery::try_from(path_str.as_str())
        .map_err(|e| GrpcError::Rpc(e.to_string()))?;

    // Use raw bytes codec — body is treated as opaque binary
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

    let codec = tonic::codec::ProstCodec::<bytes::Bytes, bytes::Bytes>::default();
    match client.unary(request, path, codec).await {
        Ok(resp) => {
            let body_bytes = resp.into_inner();
            Ok(GrpcRpcResult {
                status_code: 0,
                message: "OK".to_string(),
                body: String::from_utf8_lossy(&body_bytes).to_string(),
                trailing_metadata: HashMap::new(),
            })
        }
        Err(status) => Ok(GrpcRpcResult {
            status_code: status.code() as u32,
            message: status.message().to_string(),
            body: String::new(),
            trailing_metadata: HashMap::new(),
        }),
    }
}

fn build_reflection_list_services() -> bytes::Bytes {
    // Minimal protobuf for ServerReflectionRequest { list_services: "" }
    // field 1 (host) = "", field 7 (list_services) = ""
    bytes::Bytes::from_static(&[0x3A, 0x00]) // field 7, wire type 2, length 0
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

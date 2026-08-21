/// gRPC channel manager with real server reflection support.
///
/// The previous version of `grpc_list_services` returned a hardcoded
/// placeholder ("no protobuf dependency"), and `grpc_invoke` sent the raw
/// JSON request body as if it were the protobuf wire payload — which no
/// real gRPC server would ever accept. This implements the actual
/// `grpc.reflection.v1.ServerReflection` protocol (falling back to the
/// older `v1alpha` path many servers still only expose) and a small
/// reflection-driven dynamic protobuf codec, so arbitrary services can be
/// listed, described, and invoked with plain JSON in/out — without
/// depending on `prost`/`protoc` (no protoc binary is available in this
/// build environment, and the wire format itself is simple enough to hand-
/// roll safely, unlike the crypto-heavy protocols elsewhere in this crate).
///
/// Message field numbers for `google.protobuf.descriptor.proto` and
/// `grpc.reflection.v1.reflection.proto` are cross-checked against the
/// canonical .proto sources (protocolbuffers/protobuf and grpc/grpc on
/// GitHub) rather than recalled from memory, since a wrong field number
/// here means silently misreading structural metadata.
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
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
    #[error("Streaming methods are not yet supported — only unary RPCs can be invoked")]
    StreamingUnsupported,
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

// ── Protobuf wire format (varint/length-delimited/fixed32/fixed64) ───────

#[derive(Debug, Clone)]
enum WireValue {
    Varint(u64),
    Fixed64([u8; 8]),
    LengthDelimited(Vec<u8>),
    Fixed32([u8; 4]),
}

fn write_varint(mut n: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    for (i, &b) in data.iter().enumerate() {
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

fn write_tag(field: u32, wire_type: u32, out: &mut Vec<u8>) {
    write_varint(((field as u64) << 3) | wire_type as u64, out);
}

fn write_length_delimited(field: u32, value: &[u8], out: &mut Vec<u8>) {
    write_tag(field, 2, out);
    write_varint(value.len() as u64, out);
    out.extend_from_slice(value);
}

fn write_string_field(field: u32, value: &str, out: &mut Vec<u8>) {
    write_length_delimited(field, value.as_bytes(), out);
}

fn write_varint_field(field: u32, value: u64, out: &mut Vec<u8>) {
    write_tag(field, 0, out);
    write_varint(value, out);
}

/// Decodes all top-level fields of a message into a map of field number ->
/// every occurrence seen (repeated fields, and proto3's "last one wins"
/// singular-field semantics, both need every occurrence preserved).
fn decode_fields(data: &[u8]) -> Option<HashMap<u32, Vec<WireValue>>> {
    let mut fields: HashMap<u32, Vec<WireValue>> = HashMap::new();
    let mut i = 0;
    while i < data.len() {
        let (key, key_len) = read_varint(&data[i..])?;
        i += key_len;
        let field_num = (key >> 3) as u32;
        let wire_type = (key & 0x7) as u32;
        let value = match wire_type {
            0 => {
                let (v, n) = read_varint(&data[i..])?;
                i += n;
                WireValue::Varint(v)
            }
            1 => {
                if data.len() < i + 8 {
                    return None;
                }
                let mut b = [0u8; 8];
                b.copy_from_slice(&data[i..i + 8]);
                i += 8;
                WireValue::Fixed64(b)
            }
            2 => {
                let (len, n) = read_varint(&data[i..])?;
                i += n;
                let len = len as usize;
                if data.len() < i + len {
                    return None;
                }
                let v = data[i..i + len].to_vec();
                i += len;
                WireValue::LengthDelimited(v)
            }
            5 => {
                if data.len() < i + 4 {
                    return None;
                }
                let mut b = [0u8; 4];
                b.copy_from_slice(&data[i..i + 4]);
                i += 4;
                WireValue::Fixed32(b)
            }
            _ => return None, // groups (wire type 3/4) are not used by proto3
        };
        fields.entry(field_num).or_default().push(value);
    }
    Some(fields)
}

fn last_string(fields: &HashMap<u32, Vec<WireValue>>, field: u32) -> Option<String> {
    fields.get(&field)?.last().and_then(|v| match v {
        WireValue::LengthDelimited(b) => Some(String::from_utf8_lossy(b).into_owned()),
        _ => None,
    })
}

fn last_i64(fields: &HashMap<u32, Vec<WireValue>>, field: u32) -> Option<i64> {
    fields.get(&field)?.last().and_then(|v| match v {
        WireValue::Varint(n) => Some(*n as i64),
        _ => None,
    })
}

fn last_bool(fields: &HashMap<u32, Vec<WireValue>>, field: u32) -> bool {
    matches!(fields.get(&field).and_then(|v| v.last()), Some(WireValue::Varint(n)) if *n != 0)
}

fn all_bytes(fields: &HashMap<u32, Vec<WireValue>>, field: u32) -> Vec<Vec<u8>> {
    fields
        .get(&field)
        .map(|vs| {
            vs.iter()
                .filter_map(|v| match v {
                    WireValue::LengthDelimited(b) => Some(b.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── grpc.reflection.v1(alpha).ServerReflection message codec ─────────────
// Field numbers verified against grpc/grpc's reflection.proto.

fn build_list_services_request() -> Vec<u8> {
    let mut out = Vec::new();
    write_string_field(7, "", &mut out); // list_services (oneof)
    out
}

fn build_file_containing_symbol_request(symbol: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_string_field(4, symbol, &mut out); // file_containing_symbol (oneof)
    out
}

enum ReflectionResponse {
    Services(Vec<String>),
    FileDescriptors(Vec<Vec<u8>>),
    Error(String),
}

fn parse_reflection_response(data: &[u8]) -> Result<ReflectionResponse, GrpcError> {
    let fields = decode_fields(data).ok_or_else(|| GrpcError::Reflection("Malformed ServerReflectionResponse".into()))?;

    if let Some(WireValue::LengthDelimited(err_bytes)) = fields.get(&7).and_then(|v| v.last()) {
        let err_fields = decode_fields(err_bytes).unwrap_or_default();
        let code = last_i64(&err_fields, 1).unwrap_or(-1);
        let msg = last_string(&err_fields, 2).unwrap_or_default();
        return Ok(ReflectionResponse::Error(format!("({code}) {msg}")));
    }

    if let Some(WireValue::LengthDelimited(list_bytes)) = fields.get(&6).and_then(|v| v.last()) {
        let list_fields = decode_fields(list_bytes).unwrap_or_default();
        let services = list_fields
            .get(&1)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|v| match v {
                        WireValue::LengthDelimited(b) => decode_fields(b).and_then(|f| last_string(&f, 1)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Ok(ReflectionResponse::Services(services));
    }

    if let Some(WireValue::LengthDelimited(fd_bytes)) = fields.get(&4).and_then(|v| v.last()) {
        let fd_fields = decode_fields(fd_bytes).unwrap_or_default();
        return Ok(ReflectionResponse::FileDescriptors(all_bytes(&fd_fields, 1)));
    }

    Err(GrpcError::Reflection("Response had no recognized oneof field set".into()))
}

// ── google.protobuf.descriptor.proto (decode-only) ────────────────────────
// Field numbers verified against protocolbuffers/protobuf's descriptor.proto.

#[derive(Debug, Clone)]
struct FieldDesc {
    name: String,
    number: i64,
    label: i64, // 1=optional, 3=repeated
    field_type: i64,
    type_name: Option<String>, // fully-qualified, for TYPE_MESSAGE/TYPE_ENUM
    json_name: Option<String>,
}

#[derive(Debug, Clone)]
struct MessageDesc {
    fields: Vec<FieldDesc>,
}

#[derive(Debug, Clone)]
struct MethodDesc {
    name: String,
    input_type: String,
    output_type: String,
    client_streaming: bool,
    server_streaming: bool,
}

#[derive(Debug, Clone)]
struct ServiceDesc {
    name: String,
    methods: Vec<MethodDesc>,
}

#[derive(Default)]
struct ProtoRegistry {
    messages: HashMap<String, MessageDesc>, // key: fully-qualified ".pkg.Type"
    services: HashMap<String, ServiceDesc>,
}

fn parse_field_descriptor(bytes: &[u8]) -> Option<FieldDesc> {
    let f = decode_fields(bytes)?;
    Some(FieldDesc {
        name: last_string(&f, 1)?,
        number: last_i64(&f, 3).unwrap_or(0),
        label: last_i64(&f, 4).unwrap_or(1),
        field_type: last_i64(&f, 5).unwrap_or(9), // default to TYPE_STRING if unset (shouldn't happen)
        type_name: last_string(&f, 6),
        json_name: last_string(&f, 10),
    })
}

fn parse_message_descriptor(bytes: &[u8], prefix: &str, registry: &mut ProtoRegistry) -> Option<String> {
    let f = decode_fields(bytes)?;
    let name = last_string(&f, 1)?;
    let fq_name = format!("{prefix}.{name}");

    let fields = all_bytes(&f, 2).iter().filter_map(|b| parse_field_descriptor(b)).collect();
    registry.messages.insert(fq_name.clone(), MessageDesc { fields });

    for nested in all_bytes(&f, 3) {
        parse_message_descriptor(&nested, &fq_name, registry);
    }
    Some(fq_name)
}

fn parse_method_descriptor(bytes: &[u8]) -> Option<MethodDesc> {
    let f = decode_fields(bytes)?;
    Some(MethodDesc {
        name: last_string(&f, 1)?,
        input_type: last_string(&f, 2)?,
        output_type: last_string(&f, 3)?,
        client_streaming: last_bool(&f, 5),
        server_streaming: last_bool(&f, 6),
    })
}

fn parse_service_descriptor(bytes: &[u8], package: &str) -> Option<ServiceDesc> {
    let f = decode_fields(bytes)?;
    let name = last_string(&f, 1)?;
    let fq_name = if package.is_empty() { name.clone() } else { format!("{package}.{name}") };
    let methods = all_bytes(&f, 2).iter().filter_map(|b| parse_method_descriptor(b)).collect();
    Some(ServiceDesc { name: fq_name, methods })
}

/// Parses one `FileDescriptorProto` and merges its messages/services into
/// the registry, keyed by fully-qualified type name (leading dot, matching
/// how `type_name`/`input_type`/`output_type` reference other types).
fn register_file_descriptor(bytes: &[u8], registry: &mut ProtoRegistry) {
    let Some(f) = decode_fields(bytes) else { return };
    let package = last_string(&f, 2).unwrap_or_default();
    let prefix = if package.is_empty() { String::new() } else { format!(".{package}") };

    for msg_bytes in all_bytes(&f, 4) {
        parse_message_descriptor(&msg_bytes, &prefix, registry);
    }
    for svc_bytes in all_bytes(&f, 6) {
        if let Some(svc) = parse_service_descriptor(&svc_bytes, &package) {
            registry.services.insert(format!(".{}", svc.name), svc);
        }
    }
}

// ── Dynamic JSON <-> protobuf codec, driven by descriptors ───────────────

const TYPE_DOUBLE: i64 = 1;
const TYPE_FLOAT: i64 = 2;
const TYPE_INT64: i64 = 3;
const TYPE_UINT64: i64 = 4;
const TYPE_INT32: i64 = 5;
const TYPE_FIXED64: i64 = 6;
const TYPE_FIXED32: i64 = 7;
const TYPE_BOOL: i64 = 8;
const TYPE_STRING: i64 = 9;
const TYPE_MESSAGE: i64 = 11;
const TYPE_BYTES: i64 = 12;
const TYPE_UINT32: i64 = 13;
const TYPE_ENUM: i64 = 14;
const TYPE_SFIXED32: i64 = 15;
const TYPE_SFIXED64: i64 = 16;
const TYPE_SINT32: i64 = 17;
const TYPE_SINT64: i64 = 18;
const LABEL_REPEATED: i64 = 3;

fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}
fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

fn encode_scalar_field(field: &FieldDesc, value: &Json, out: &mut Vec<u8>) {
    match field.field_type {
        TYPE_STRING => {
            if let Some(s) = value.as_str() {
                write_string_field(field.number as u32, s, out);
            }
        }
        TYPE_BYTES => {
            if let Some(s) = value.as_str() {
                if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s) {
                    write_length_delimited(field.number as u32, &bytes, out);
                }
            }
        }
        TYPE_BOOL => {
            let b = value.as_bool().unwrap_or(false);
            write_varint_field(field.number as u32, b as u64, out);
        }
        TYPE_INT32 | TYPE_INT64 | TYPE_UINT32 | TYPE_UINT64 | TYPE_ENUM => {
            let n = value.as_i64().or_else(|| value.as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
            write_varint_field(field.number as u32, n as u64, out);
        }
        TYPE_SINT32 | TYPE_SINT64 => {
            let n = value.as_i64().unwrap_or(0);
            write_varint_field(field.number as u32, zigzag_encode(n), out);
        }
        TYPE_FIXED32 | TYPE_SFIXED32 => {
            let n = value.as_i64().unwrap_or(0) as i32;
            write_tag(field.number as u32, 5, out);
            out.extend_from_slice(&n.to_le_bytes());
        }
        TYPE_FIXED64 | TYPE_SFIXED64 => {
            let n = value.as_i64().unwrap_or(0);
            write_tag(field.number as u32, 1, out);
            out.extend_from_slice(&n.to_le_bytes());
        }
        TYPE_FLOAT => {
            let n = value.as_f64().unwrap_or(0.0) as f32;
            write_tag(field.number as u32, 5, out);
            out.extend_from_slice(&n.to_le_bytes());
        }
        TYPE_DOUBLE => {
            let n = value.as_f64().unwrap_or(0.0);
            write_tag(field.number as u32, 1, out);
            out.extend_from_slice(&n.to_le_bytes());
        }
        _ => {}
    }
}

fn json_to_protobuf(obj: &Json, msg: &MessageDesc, registry: &ProtoRegistry) -> Vec<u8> {
    let mut out = Vec::new();
    let Some(map) = obj.as_object() else { return out };

    for field in &msg.fields {
        let json_key = map
            .contains_key(&field.name)
            .then_some(field.name.as_str())
            .or_else(|| field.json_name.as_deref().filter(|k| map.contains_key(*k)));
        let Some(key) = json_key else { continue };
        let Some(value) = map.get(key) else { continue };

        if field.field_type == TYPE_MESSAGE {
            let Some(type_name) = &field.type_name else { continue };
            let Some(nested_desc) = registry.messages.get(type_name) else { continue };
            if field.label == LABEL_REPEATED {
                if let Some(arr) = value.as_array() {
                    for item in arr {
                        let encoded = json_to_protobuf(item, nested_desc, registry);
                        write_length_delimited(field.number as u32, &encoded, &mut out);
                    }
                }
            } else {
                let encoded = json_to_protobuf(value, nested_desc, registry);
                write_length_delimited(field.number as u32, &encoded, &mut out);
            }
        } else if field.label == LABEL_REPEATED {
            if let Some(arr) = value.as_array() {
                for item in arr {
                    encode_scalar_field(field, item, &mut out);
                }
            }
        } else {
            encode_scalar_field(field, value, &mut out);
        }
    }
    out
}

fn decode_scalar_value(field: &FieldDesc, value: &WireValue) -> Json {
    match (field.field_type, value) {
        (TYPE_STRING, WireValue::LengthDelimited(b)) => Json::String(String::from_utf8_lossy(b).into_owned()),
        (TYPE_BYTES, WireValue::LengthDelimited(b)) => Json::String(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b)),
        (TYPE_BOOL, WireValue::Varint(n)) => Json::Bool(*n != 0),
        (TYPE_INT32 | TYPE_ENUM, WireValue::Varint(n)) => Json::Number((*n as i32).into()),
        (TYPE_UINT32, WireValue::Varint(n)) => Json::Number((*n as u32).into()),
        (TYPE_INT64, WireValue::Varint(n)) => Json::Number((*n as i64).into()),
        (TYPE_UINT64, WireValue::Varint(n)) => Json::Number((*n).into()),
        (TYPE_SINT32, WireValue::Varint(n)) => Json::Number((zigzag_decode(*n) as i32).into()),
        (TYPE_SINT64, WireValue::Varint(n)) => Json::Number(zigzag_decode(*n).into()),
        (TYPE_FIXED32, WireValue::Fixed32(b)) => Json::Number(u32::from_le_bytes(*b).into()),
        (TYPE_SFIXED32, WireValue::Fixed32(b)) => Json::Number(i32::from_le_bytes(*b).into()),
        (TYPE_FLOAT, WireValue::Fixed32(b)) => serde_json::Number::from_f64(f32::from_le_bytes(*b) as f64).map(Json::Number).unwrap_or(Json::Null),
        (TYPE_FIXED64, WireValue::Fixed64(b)) => Json::Number(u64::from_le_bytes(*b).into()),
        (TYPE_SFIXED64, WireValue::Fixed64(b)) => Json::Number(i64::from_le_bytes(*b).into()),
        (TYPE_DOUBLE, WireValue::Fixed64(b)) => serde_json::Number::from_f64(f64::from_le_bytes(*b)).map(Json::Number).unwrap_or(Json::Null),
        _ => Json::Null,
    }
}

fn protobuf_to_json(bytes: &[u8], msg: &MessageDesc, registry: &ProtoRegistry) -> Json {
    let Some(fields) = decode_fields(bytes) else { return Json::Object(Default::default()) };
    let mut obj = serde_json::Map::new();

    for field in &msg.fields {
        let Some(values) = fields.get(&(field.number as u32)) else { continue };
        if values.is_empty() {
            continue;
        }

        if field.field_type == TYPE_MESSAGE {
            let Some(type_name) = &field.type_name else { continue };
            let Some(nested_desc) = registry.messages.get(type_name) else { continue };
            if field.label == LABEL_REPEATED {
                let arr: Vec<Json> = values
                    .iter()
                    .filter_map(|v| match v {
                        WireValue::LengthDelimited(b) => Some(protobuf_to_json(b, nested_desc, registry)),
                        _ => None,
                    })
                    .collect();
                obj.insert(field.name.clone(), Json::Array(arr));
            } else if let Some(WireValue::LengthDelimited(b)) = values.last() {
                obj.insert(field.name.clone(), protobuf_to_json(b, nested_desc, registry));
            }
        } else if field.label == LABEL_REPEATED {
            let arr: Vec<Json> = values.iter().map(|v| decode_scalar_value(field, v)).collect();
            obj.insert(field.name.clone(), Json::Array(arr));
        } else if let Some(v) = values.last() {
            obj.insert(field.name.clone(), decode_scalar_value(field, v));
        }
    }
    Json::Object(obj)
}

// ── Session state ─────────────────────────────────────────────────────────

struct GrpcConn {
    config: GrpcConfig,
    channel: Channel,
    registry: ProtoRegistry,
}

pub struct GrpcState {
    sessions: Mutex<HashMap<String, GrpcConn>>,
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
    fn encoder(&mut self) -> RawEncoder {
        RawEncoder
    }
    fn decoder(&mut self) -> RawDecoder {
        RawDecoder
    }
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
        if remaining == 0 {
            return Ok(None);
        }
        Ok(Some(src.copy_to_bytes(remaining)))
    }
}

fn apply_metadata(request: &mut tonic::Request<impl Send>, metadata: &HashMap<String, String>) {
    for (k, v) in metadata {
        if let (Ok(name), Ok(value)) = (tonic::metadata::MetadataKey::from_bytes(k.as_bytes()), tonic::metadata::MetadataValue::try_from(v.as_str())) {
            request.metadata_mut().insert(name, value);
        }
    }
}

/// Calls `ServerReflectionInfo` with a single request message and reads
/// back a single response — the reflection RPC is bidi-streaming, but every
/// query this client makes is a fresh one-request/one-response exchange,
/// so `client_streaming` (send a 1-item stream, read the first reply) is
/// simpler than driving a persistent bidi session. Tries the stable `v1`
/// service path first, falling back to `v1alpha` — many servers (including
/// most grpcurl-tested ones in the wild) still only implement the older path.
async fn call_reflection(channel: Channel, metadata: &HashMap<String, String>, request_bytes: Vec<u8>) -> Result<bytes::Bytes, GrpcError> {
    for service_path in ["grpc.reflection.v1.ServerReflection", "grpc.reflection.v1alpha.ServerReflection"] {
        let mut client = tonic::client::Grpc::new(channel.clone());
        if client.ready().await.is_err() {
            continue;
        }
        let path_str = format!("/{service_path}/ServerReflectionInfo");
        let Ok(path) = tonic::codegen::http::uri::PathAndQuery::try_from(path_str.as_str()) else { continue };

        let body = bytes::Bytes::from(request_bytes.clone());
        let mut request = tonic::Request::new(tokio_stream::once(body));
        apply_metadata(&mut request, metadata);

        match client.client_streaming(request, path, RawCodec).await {
            Ok(resp) => return Ok(resp.into_inner()),
            Err(_) => continue,
        }
    }
    Err(GrpcError::Reflection("Server does not implement grpc.reflection.v1 or v1alpha".into()))
}

#[tauri::command]
pub async fn grpc_connect(config: GrpcConfig, state: tauri::State<'_, GrpcState>) -> Result<String, GrpcError> {
    let mut endpoint = Endpoint::from_shared(config.endpoint.clone())
        .map_err(|e| GrpcError::Transport(e.to_string()))?
        .timeout(std::time::Duration::from_secs(30))
        .concurrency_limit(32);

    if config.endpoint.starts_with("https://") {
        let tls = ClientTlsConfig::new(); // TODO: load native CA bundle
        endpoint = endpoint.tls_config(tls).map_err(|e| GrpcError::Transport(e.to_string()))?;
    }

    let channel = endpoint.connect().await.map_err(|e| GrpcError::Transport(e.to_string()))?;

    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), GrpcConn { config, channel, registry: ProtoRegistry::default() });
    Ok(id)
}

#[tauri::command]
pub async fn grpc_list_services(id: String, state: tauri::State<'_, GrpcState>) -> Result<Vec<String>, GrpcError> {
    let (channel, metadata) = {
        let sessions = state.sessions.lock().unwrap();
        let conn = sessions.get(&id).ok_or_else(|| GrpcError::NotFound(id.clone()))?;
        (conn.channel.clone(), conn.config.metadata.clone())
    };

    let response = call_reflection(channel, &metadata, build_list_services_request()).await?;
    match parse_reflection_response(&response)? {
        ReflectionResponse::Services(mut services) => {
            services.retain(|s| !s.starts_with("grpc.reflection."));
            Ok(services)
        }
        ReflectionResponse::Error(e) => Err(GrpcError::Reflection(e)),
        ReflectionResponse::FileDescriptors(_) => Err(GrpcError::Reflection("Unexpected file descriptor response to list_services".into())),
    }
}

#[tauri::command]
pub async fn grpc_describe_service(id: String, service: String, state: tauri::State<'_, GrpcState>) -> Result<GrpcService, GrpcError> {
    let (channel, metadata) = {
        let sessions = state.sessions.lock().unwrap();
        let conn = sessions.get(&id).ok_or_else(|| GrpcError::NotFound(id.clone()))?;
        (conn.channel.clone(), conn.config.metadata.clone())
    };

    let response = call_reflection(channel, &metadata, build_file_containing_symbol_request(&service)).await?;
    let file_descriptors = match parse_reflection_response(&response)? {
        ReflectionResponse::FileDescriptors(files) => files,
        ReflectionResponse::Error(e) => return Err(GrpcError::Reflection(e)),
        ReflectionResponse::Services(_) => return Err(GrpcError::Reflection("Unexpected list_services response to file_containing_symbol".into())),
    };

    let mut registry = ProtoRegistry::default();
    for fd in &file_descriptors {
        register_file_descriptor(fd, &mut registry);
    }

    let fq_service = format!(".{service}");
    let svc = registry.services.get(&fq_service).cloned().ok_or_else(|| GrpcError::Reflection(format!("Service {service} not found in reflection response")))?;

    let mut sessions = state.sessions.lock().unwrap();
    if let Some(conn) = sessions.get_mut(&id) {
        conn.registry.messages.extend(registry.messages.clone());
        conn.registry.services.extend(registry.services.clone());
    }

    Ok(GrpcService {
        name: service,
        methods: svc
            .methods
            .iter()
            .map(|m| GrpcMethod {
                name: m.name.clone(),
                client_streaming: m.client_streaming,
                server_streaming: m.server_streaming,
                input_type: m.input_type.clone(),
                output_type: m.output_type.clone(),
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn grpc_invoke(id: String, service: String, method: String, json_body: String, state: tauri::State<'_, GrpcState>) -> Result<GrpcRpcResult, GrpcError> {
    let (channel, metadata, method_desc, registry) = {
        let sessions = state.sessions.lock().unwrap();
        let conn = sessions.get(&id).ok_or_else(|| GrpcError::NotFound(id.clone()))?;
        let fq_service = format!(".{service}");
        let svc = conn.registry.services.get(&fq_service).ok_or_else(|| GrpcError::Reflection("Call grpc_describe_service before invoking a method".into()))?;
        let method_desc = svc.methods.iter().find(|m| m.name == method).cloned().ok_or_else(|| GrpcError::Reflection(format!("Method {method} not found on {service}")))?;
        // Registries are typically small (a handful of message types per
        // service); cloning avoids holding the session lock across the
        // network round-trip below.
        (conn.channel.clone(), conn.config.metadata.clone(), method_desc, (conn.registry.messages.clone()))
    };

    if method_desc.client_streaming || method_desc.server_streaming {
        return Err(GrpcError::StreamingUnsupported);
    }

    let full_registry = ProtoRegistry { messages: registry, services: HashMap::new() };
    let input_desc = full_registry.messages.get(&method_desc.input_type).ok_or_else(|| GrpcError::Reflection(format!("Unknown input type {}", method_desc.input_type)))?;
    let output_desc = full_registry.messages.get(&method_desc.output_type).ok_or_else(|| GrpcError::Reflection(format!("Unknown output type {}", method_desc.output_type)))?.clone();

    let json_value: Json = serde_json::from_str(&json_body).map_err(|e| GrpcError::Rpc(format!("Invalid JSON body: {e}")))?;
    let request_bytes = json_to_protobuf(&json_value, input_desc, &full_registry);

    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|e| GrpcError::Rpc(e.to_string()))?;

    let path_str = format!("/{service}/{method}");
    let path = tonic::codegen::http::uri::PathAndQuery::try_from(path_str.as_str()).map_err(|e| GrpcError::Rpc(e.to_string()))?;

    let body = bytes::Bytes::from(request_bytes);
    let mut request = tonic::Request::new(body);
    apply_metadata(&mut request, &metadata);

    match client.unary(request, path, RawCodec).await {
        Ok(resp) => {
            let body_bytes: bytes::Bytes = resp.into_inner();
            let json_response = protobuf_to_json(&body_bytes, &output_desc, &full_registry);
            Ok(GrpcRpcResult {
                status_code: 0,
                message: "OK".to_string(),
                body: serde_json::to_string_pretty(&json_response).unwrap_or_default(),
                trailing_metadata: HashMap::new(),
            })
        }
        Err(status) => Ok(GrpcRpcResult {
            status_code: status.code() as i32 as u32,
            message: status.message().to_string(),
            body: String::new(),
            trailing_metadata: HashMap::new(),
        }),
    }
}

#[tauri::command]
pub fn grpc_disconnect(id: String, state: tauri::State<'_, GrpcState>) -> Result<(), GrpcError> {
    state.sessions.lock().unwrap().remove(&id).ok_or(GrpcError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn grpc_list_sessions(state: tauri::State<'_, GrpcState>) -> Vec<GrpcSession> {
    state.sessions.lock().unwrap().iter().map(|(id, conn)| GrpcSession { id: id.clone(), endpoint: conn.config.endpoint.clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
        for n in [0u64, 1, 127, 128, 300, 16384, u32::MAX as u64, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(n, &mut buf);
            let (decoded, len) = read_varint(&buf).unwrap();
            assert_eq!(decoded, n);
            assert_eq!(len, buf.len());
        }
    }

    #[test]
    fn test_zigzag_roundtrip() {
        for n in [0i64, 1, -1, 2, -2, 1000, -1000, i32::MIN as i64, i32::MAX as i64] {
            assert_eq!(zigzag_decode(zigzag_encode(n)), n, "roundtrip failed for {n}");
        }
    }

    #[test]
    fn test_decode_fields_string_and_varint() {
        let mut buf = Vec::new();
        write_string_field(1, "hello", &mut buf);
        write_varint_field(2, 42, &mut buf);
        let fields = decode_fields(&buf).unwrap();
        assert_eq!(last_string(&fields, 1), Some("hello".to_string()));
        assert_eq!(last_i64(&fields, 2), Some(42));
    }

    #[test]
    fn test_build_list_services_request_and_parse_response() {
        // Build a synthetic ServerReflectionResponse with a
        // list_services_response containing two services.
        let svc1 = { let mut b = Vec::new(); write_string_field(1, "pkg.Foo", &mut b); b };
        let svc2 = { let mut b = Vec::new(); write_string_field(1, "grpc.reflection.v1.ServerReflection", &mut b); b };
        let list_resp = {
            let mut b = Vec::new();
            write_length_delimited(1, &svc1, &mut b);
            write_length_delimited(1, &svc2, &mut b);
            b
        };
        let outer = { let mut b = Vec::new(); write_length_delimited(6, &list_resp, &mut b); b };

        match parse_reflection_response(&outer).unwrap() {
            ReflectionResponse::Services(services) => {
                assert_eq!(services, vec!["pkg.Foo".to_string(), "grpc.reflection.v1.ServerReflection".to_string()]);
            }
            _ => panic!("expected Services variant"),
        }
    }

    #[test]
    fn test_parse_reflection_response_error_variant() {
        let err = { let mut b = Vec::new(); write_varint_field(1, 12, &mut b); write_string_field(2, "not found".into(), &mut b); b };
        let outer = { let mut b = Vec::new(); write_length_delimited(7, &err, &mut b); b };
        match parse_reflection_response(&outer).unwrap() {
            ReflectionResponse::Error(msg) => assert!(msg.contains("not found")),
            _ => panic!("expected Error variant"),
        }
    }

    /// Builds a synthetic FileDescriptorProto for:
    ///   package demo;
    ///   message Greeting { string text = 1; }
    ///   message HelloRequest { string name = 1; repeated string tags = 2; }
    ///   message HelloResponse { Greeting greeting = 1; }
    ///   service Greeter { rpc SayHello(HelloRequest) returns (HelloResponse); }
    fn build_demo_file_descriptor() -> Vec<u8> {
        fn field(name: &str, number: i64, label: i64, field_type: i64, type_name: Option<&str>) -> Vec<u8> {
            let mut b = Vec::new();
            write_string_field(1, name, &mut b);
            write_varint_field(3, number as u64, &mut b);
            write_varint_field(4, label as u64, &mut b);
            write_varint_field(5, field_type as u64, &mut b);
            if let Some(tn) = type_name {
                write_string_field(6, tn, &mut b);
            }
            b
        }
        fn message(name: &str, fields: &[Vec<u8>]) -> Vec<u8> {
            let mut b = Vec::new();
            write_string_field(1, name, &mut b);
            for f in fields {
                write_length_delimited(2, f, &mut b);
            }
            b
        }

        let greeting = message("Greeting", &[field("text", 1, 1, TYPE_STRING, None)]);
        let hello_request = message(
            "HelloRequest",
            &[field("name", 1, 1, TYPE_STRING, None), field("tags", 2, LABEL_REPEATED, TYPE_STRING, None)],
        );
        let hello_response = message("HelloResponse", &[field("greeting", 1, 1, TYPE_MESSAGE, Some(".demo.Greeting"))]);

        let method = {
            let mut b = Vec::new();
            write_string_field(1, "SayHello", &mut b);
            write_string_field(2, ".demo.HelloRequest", &mut b);
            write_string_field(3, ".demo.HelloResponse", &mut b);
            b
        };
        let service = {
            let mut b = Vec::new();
            write_string_field(1, "Greeter", &mut b);
            write_length_delimited(2, &method, &mut b);
            b
        };

        let mut file = Vec::new();
        write_string_field(1, "demo.proto", &mut file);
        write_string_field(2, "demo", &mut file);
        for m in [&greeting, &hello_request, &hello_response] {
            write_length_delimited(4, m, &mut file);
        }
        write_length_delimited(6, &service, &mut file);
        file
    }

    #[test]
    fn test_register_file_descriptor_populates_messages_and_services() {
        let mut registry = ProtoRegistry::default();
        register_file_descriptor(&build_demo_file_descriptor(), &mut registry);

        assert!(registry.messages.contains_key(".demo.Greeting"));
        assert!(registry.messages.contains_key(".demo.HelloRequest"));
        assert!(registry.messages.contains_key(".demo.HelloResponse"));

        let svc = registry.services.get(".demo.Greeter").expect("service should be registered");
        assert_eq!(svc.methods.len(), 1);
        assert_eq!(svc.methods[0].name, "SayHello");
        assert_eq!(svc.methods[0].input_type, ".demo.HelloRequest");
        assert_eq!(svc.methods[0].output_type, ".demo.HelloResponse");
    }

    #[test]
    fn test_json_to_protobuf_to_json_round_trip_with_nested_message_and_repeated_field() {
        let mut registry = ProtoRegistry::default();
        register_file_descriptor(&build_demo_file_descriptor(), &mut registry);

        let request_desc = registry.messages.get(".demo.HelloRequest").unwrap();
        let json = serde_json::json!({ "name": "Ada", "tags": ["admin", "vip"] });
        let encoded = json_to_protobuf(&json, request_desc, &registry);
        let decoded = protobuf_to_json(&encoded, request_desc, &registry);

        assert_eq!(decoded["name"], "Ada");
        assert_eq!(decoded["tags"], serde_json::json!(["admin", "vip"]));
    }

    #[test]
    fn test_json_to_protobuf_to_json_round_trip_nested_message_field() {
        let mut registry = ProtoRegistry::default();
        register_file_descriptor(&build_demo_file_descriptor(), &mut registry);

        let response_desc = registry.messages.get(".demo.HelloResponse").unwrap();
        let json = serde_json::json!({ "greeting": { "text": "hello, Ada!" } });
        let encoded = json_to_protobuf(&json, response_desc, &registry);
        let decoded = protobuf_to_json(&encoded, response_desc, &registry);

        assert_eq!(decoded["greeting"]["text"], "hello, Ada!");
    }

    #[test]
    fn test_scalar_types_round_trip() {
        fn field(number: i64, field_type: i64) -> FieldDesc {
            FieldDesc { name: "v".into(), number, label: 1, field_type, type_name: None, json_name: None }
        }
        let msg = MessageDesc {
            fields: vec![
                field(1, TYPE_INT32),
                field(2, TYPE_BOOL),
                field(3, TYPE_DOUBLE),
                field(4, TYPE_SINT64),
                field(5, TYPE_FIXED32),
                field(6, TYPE_BYTES),
            ],
        };
        // Encode each field individually (they share field name "v" so we
        // can't put them in one JSON object) — verify per-field instead.
        let registry = ProtoRegistry::default();

        let int32_msg = MessageDesc { fields: vec![field(1, TYPE_INT32)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": -5}), &int32_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &int32_msg, &registry)["v"], -5);

        let bool_msg = MessageDesc { fields: vec![field(2, TYPE_BOOL)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": true}), &bool_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &bool_msg, &registry)["v"], true);

        let double_msg = MessageDesc { fields: vec![field(3, TYPE_DOUBLE)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": 3.5}), &double_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &double_msg, &registry)["v"], 3.5);

        let sint64_msg = MessageDesc { fields: vec![field(4, TYPE_SINT64)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": -123456}), &sint64_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &sint64_msg, &registry)["v"], -123456);

        let bytes_msg = MessageDesc { fields: vec![field(6, TYPE_BYTES)] };
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"hi");
        let encoded = json_to_protobuf(&serde_json::json!({"v": b64}), &bytes_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &bytes_msg, &registry)["v"], b64);

        let _ = msg; // silence unused-field-combo warning; each type checked individually above
    }

    #[test]
    fn test_read_varint_reports_incomplete_and_overflow() {
        // No terminating (high-bit-clear) byte anywhere in the buffer.
        assert!(read_varint(&[0x80, 0x80]).is_none());
        // 10 continuation bytes never terminate and blow past the 64-bit shift limit.
        let overflow = [0x80u8; 10];
        assert!(read_varint(&overflow).is_none());
    }

    #[test]
    fn test_decode_fields_rejects_truncated_fixed64_length_delimited_and_fixed32() {
        // wire type 1 (fixed64) needs 8 bytes; give it 3.
        let mut buf = Vec::new();
        write_tag(1, 1, &mut buf);
        buf.extend_from_slice(&[1, 2, 3]);
        assert!(decode_fields(&buf).is_none());

        // wire type 2 (length-delimited) declares more bytes than are present.
        let mut buf = Vec::new();
        write_tag(1, 2, &mut buf);
        write_varint(10, &mut buf);
        buf.extend_from_slice(&[1, 2]);
        assert!(decode_fields(&buf).is_none());

        // wire type 5 (fixed32) needs 4 bytes; give it 1.
        let mut buf = Vec::new();
        write_tag(1, 5, &mut buf);
        buf.push(0xAA);
        assert!(decode_fields(&buf).is_none());
    }

    #[test]
    fn test_decode_fields_rejects_group_wire_types() {
        let mut buf = Vec::new();
        write_tag(1, 3, &mut buf); // wire type 3 = start-group, unsupported
        assert!(decode_fields(&buf).is_none());
    }

    #[test]
    fn test_last_bool_defaults_false_when_absent_or_wrong_type() {
        let fields: HashMap<u32, Vec<WireValue>> = HashMap::new();
        assert!(!last_bool(&fields, 1));

        let mut buf = Vec::new();
        write_string_field(1, "not-a-bool", &mut buf);
        let fields = decode_fields(&buf).unwrap();
        assert!(!last_bool(&fields, 1));

        let mut buf = Vec::new();
        write_varint_field(2, 1, &mut buf);
        let fields = decode_fields(&buf).unwrap();
        assert!(last_bool(&fields, 2));
    }

    #[test]
    fn test_all_bytes_filters_to_length_delimited_only_and_defaults_empty() {
        let fields: HashMap<u32, Vec<WireValue>> = HashMap::new();
        assert!(all_bytes(&fields, 9).is_empty());

        let mut buf = Vec::new();
        write_length_delimited(1, b"a", &mut buf);
        write_length_delimited(1, b"bb", &mut buf);
        let fields = decode_fields(&buf).unwrap();
        let vals = all_bytes(&fields, 1);
        assert_eq!(vals, vec![b"a".to_vec(), b"bb".to_vec()]);
    }

    #[test]
    fn test_build_file_containing_symbol_request_encodes_field_4() {
        let req = build_file_containing_symbol_request("demo.Greeter");
        let fields = decode_fields(&req).unwrap();
        assert_eq!(last_string(&fields, 4), Some("demo.Greeter".to_string()));
    }

    #[test]
    fn test_parse_reflection_response_file_descriptors_variant() {
        let fd1 = b"fake-descriptor-bytes-1".to_vec();
        let fd2 = b"fake-descriptor-bytes-2".to_vec();
        let fd_msg = {
            let mut b = Vec::new();
            write_length_delimited(1, &fd1, &mut b);
            write_length_delimited(1, &fd2, &mut b);
            b
        };
        let outer = { let mut b = Vec::new(); write_length_delimited(4, &fd_msg, &mut b); b };
        match parse_reflection_response(&outer).unwrap() {
            ReflectionResponse::FileDescriptors(files) => assert_eq!(files, vec![fd1, fd2]),
            _ => panic!("expected FileDescriptors variant"),
        }
    }

    #[test]
    fn test_parse_reflection_response_errors_when_no_recognized_oneof_is_set() {
        // A well-formed but empty message: no oneof field (4, 6, or 7) present.
        let outer: Vec<u8> = Vec::new();
        assert!(parse_reflection_response(&outer).is_err());
    }

    #[test]
    fn test_parse_field_descriptor_requires_a_name() {
        let mut b = Vec::new();
        write_varint_field(3, 1, &mut b); // number, but no name (field 1)
        assert!(parse_field_descriptor(&b).is_none());
    }

    #[test]
    fn test_parse_service_descriptor_with_empty_package_omits_prefix() {
        let method = {
            let mut b = Vec::new();
            write_string_field(1, "Ping", &mut b);
            write_string_field(2, ".Req", &mut b);
            write_string_field(3, ".Resp", &mut b);
            b
        };
        let service = {
            let mut b = Vec::new();
            write_string_field(1, "Pinger", &mut b);
            write_length_delimited(2, &method, &mut b);
            b
        };
        let svc = parse_service_descriptor(&service, "").unwrap();
        assert_eq!(svc.name, "Pinger");
        assert_eq!(svc.methods[0].name, "Ping");
    }

    #[test]
    fn test_decode_scalar_value_covers_remaining_numeric_types() {
        fn f(number: i64, field_type: i64) -> FieldDesc {
            FieldDesc { name: "v".into(), number, label: 1, field_type, type_name: None, json_name: None }
        }
        let registry = ProtoRegistry::default();

        let uint64_msg = MessageDesc { fields: vec![f(1, TYPE_UINT64)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": 9000000000i64}), &uint64_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &uint64_msg, &registry)["v"], 9000000000i64);

        let uint32_msg = MessageDesc { fields: vec![f(1, TYPE_UINT32)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": 42}), &uint32_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &uint32_msg, &registry)["v"], 42);

        let fixed64_msg = MessageDesc { fields: vec![f(1, TYPE_FIXED64)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": 12345}), &fixed64_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &fixed64_msg, &registry)["v"], 12345);

        let sfixed64_msg = MessageDesc { fields: vec![f(1, TYPE_SFIXED64)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": -12345}), &sfixed64_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &sfixed64_msg, &registry)["v"], -12345);

        let sfixed32_msg = MessageDesc { fields: vec![f(1, TYPE_SFIXED32)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": -7}), &sfixed32_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &sfixed32_msg, &registry)["v"], -7);

        let float_msg = MessageDesc { fields: vec![f(1, TYPE_FLOAT)] };
        let encoded = json_to_protobuf(&serde_json::json!({"v": 1.5}), &float_msg, &registry);
        assert_eq!(protobuf_to_json(&encoded, &float_msg, &registry)["v"], 1.5);
    }

    #[test]
    fn test_encode_scalar_field_ignores_message_type_defensive_default() {
        // encode_scalar_field only handles scalar wire types; TYPE_MESSAGE
        // falls into the `_ => {}` arm and must not panic or write anything.
        let field = FieldDesc { name: "v".into(), number: 1, label: 1, field_type: TYPE_MESSAGE, type_name: Some(".x.Y".into()), json_name: None };
        let mut out = Vec::new();
        encode_scalar_field(&field, &serde_json::json!({"a": 1}), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn test_apply_metadata_inserts_valid_header_entries() {
        let mut request = tonic::Request::new(());
        let mut metadata = HashMap::new();
        metadata.insert("x-trace-id".to_string(), "abc123".to_string());
        apply_metadata(&mut request, &metadata);
        assert_eq!(request.metadata().get("x-trace-id").unwrap().to_str().unwrap(), "abc123");
    }

    #[test]
    fn test_apply_metadata_skips_invalid_header_names() {
        let mut request = tonic::Request::new(());
        let mut metadata = HashMap::new();
        metadata.insert("invalid header name!".to_string(), "value".to_string());
        apply_metadata(&mut request, &metadata);
        assert!(request.metadata().is_empty());
    }

    #[test]
    fn test_parse_message_descriptor_recurses_into_nested_message_types() {
        // message Outer { message Inner { string v = 1; } }
        // Field 3 on DescriptorProto is nested_type; parse_message_descriptor
        // must recurse into it and register it under the parent's fq name,
        // a path no existing test exercised (build_demo_file_descriptor has
        // no nested types).
        let inner_field = { let mut b = Vec::new(); write_string_field(1, "v", &mut b); write_varint_field(5, TYPE_STRING as u64, &mut b); b };
        let inner = { let mut b = Vec::new(); write_string_field(1, "Inner", &mut b); write_length_delimited(2, &inner_field, &mut b); b };
        let outer = {
            let mut b = Vec::new();
            write_string_field(1, "Outer", &mut b);
            write_length_delimited(3, &inner, &mut b); // nested_type
            b
        };

        let mut registry = ProtoRegistry::default();
        parse_message_descriptor(&outer, "", &mut registry);

        assert!(registry.messages.contains_key(".Outer"));
        assert!(registry.messages.contains_key(".Outer.Inner"), "nested type must be registered under the parent's fq name");
        assert_eq!(registry.messages.get(".Outer.Inner").unwrap().fields[0].name, "v");
    }

    #[test]
    fn test_json_to_protobuf_falls_back_to_json_name_when_field_name_key_absent() {
        // Some clients send camelCase JSON keys matching the descriptor's
        // json_name rather than the proto field name — json_to_protobuf must
        // still find the value via that fallback.
        let field = FieldDesc {
            name: "user_name".into(),
            number: 1,
            label: 1,
            field_type: TYPE_STRING,
            type_name: None,
            json_name: Some("userName".into()),
        };
        let msg = MessageDesc { fields: vec![field] };
        let registry = ProtoRegistry::default();

        let json = serde_json::json!({ "userName": "ada" });
        let encoded = json_to_protobuf(&json, &msg, &registry);
        let decoded = protobuf_to_json(&encoded, &msg, &registry);
        assert_eq!(decoded["user_name"], "ada");
    }

    #[test]
    fn test_grpc_error_display_and_serialize() {
        let err = GrpcError::NotFound("id1".into());
        assert_eq!(err.to_string(), "Session not found: id1");
        assert_eq!(serde_json::to_string(&err).unwrap(), "\"Session not found: id1\"");

        let err = GrpcError::StreamingUnsupported;
        assert_eq!(err.to_string(), "Streaming methods are not yet supported — only unary RPCs can be invoked");
    }

    #[test]
    fn test_grpc_config_and_session_serde_round_trip() {
        let mut metadata = HashMap::new();
        metadata.insert("auth".to_string(), "token".to_string());
        let cfg = GrpcConfig { endpoint: "http://localhost:50051".into(), verify_tls: false, metadata };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: GrpcConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.endpoint, "http://localhost:50051");
        assert_eq!(restored.metadata.get("auth").map(String::as_str), Some("token"));

        let session = GrpcSession { id: "s1".into(), endpoint: "http://localhost:50051".into() };
        let restored: GrpcSession = serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(restored.id, "s1");
    }

    #[test]
    fn test_grpc_service_method_and_rpc_result_serde_round_trip() {
        let svc = GrpcService {
            name: "Greeter".into(),
            methods: vec![GrpcMethod {
                name: "SayHello".into(),
                client_streaming: false,
                server_streaming: false,
                input_type: ".demo.HelloRequest".into(),
                output_type: ".demo.HelloResponse".into(),
            }],
        };
        let restored: GrpcService = serde_json::from_str(&serde_json::to_string(&svc).unwrap()).unwrap();
        assert_eq!(restored.methods[0].name, "SayHello");

        let result = GrpcRpcResult { status_code: 0, message: "OK".into(), body: "{}".into(), trailing_metadata: HashMap::new() };
        let restored: GrpcRpcResult = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(restored.status_code, 0);
    }

    #[test]
    fn test_grpc_state_new_starts_empty() {
        let state = GrpcState::new();
        assert!(state.sessions.lock().unwrap().is_empty());
    }
}

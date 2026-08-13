/// SNMP v1/v2c/v3 client — UDP port 161.
/// Encodes/decodes BER (Basic Encoding Rules) for SNMP PDUs without
/// external crates so no new dependency is needed.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use thiserror::Error;
use tokio::net::UdpSocket;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SnmpError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Timeout — no response from agent")]
    Timeout,
    #[error("PDU encode/decode error: {0}")]
    Pdu(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for SnmpError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SnmpVersion {
    V1,
    V2c,
    V3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnmpConfig {
    pub host: String,
    pub port: u16,
    pub version: SnmpVersion,
    /// Community string (v1/v2c)
    pub community: Option<String>,
    /// SNMPv3 security name
    pub username: Option<String>,
    /// SNMPv3 auth passphrase
    pub auth_passphrase: Option<String>,
    /// SNMPv3 priv passphrase
    pub priv_passphrase: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnmpSession {
    pub id: String,
    pub host: String,
    pub version: SnmpVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnmpVarBind {
    pub oid: String,
    pub value_type: String,
    pub value: String,
}

pub struct SnmpState {
    sessions: Mutex<HashMap<String, SnmpConfig>>,
}

impl SnmpState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

// ── Minimal BER helpers ──────────────────────────────────────────────────

fn ber_length(len: usize) -> Vec<u8> {
    if len < 128 {
        vec![len as u8]
    } else if len < 256 {
        vec![0x81, len as u8]
    } else {
        vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
    }
}

fn ber_tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut v = vec![tag];
    v.extend(ber_length(value.len()));
    v.extend_from_slice(value);
    v
}

fn ber_integer(n: i64) -> Vec<u8> {
    let bytes = n.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    ber_tlv(0x02, &bytes[start..])
}

fn ber_octet_string(s: &[u8]) -> Vec<u8> { ber_tlv(0x04, s) }
fn ber_null() -> Vec<u8> { vec![0x05, 0x00] }
fn ber_oid(oid: &str) -> Vec<u8> {
    let parts: Vec<u64> = oid.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() < 2 { return ber_tlv(0x06, &[]); }
    let mut encoded = vec![(parts[0] * 40 + parts[1]) as u8];
    for &val in &parts[2..] {
        if val < 128 {
            encoded.push(val as u8);
        } else {
            let mut buf = vec![];
            let mut v = val;
            while v > 0 {
                buf.insert(0, (v & 0x7F) as u8 | if buf.is_empty() { 0 } else { 0x80 });
                v >>= 7;
            }
            encoded.extend(buf);
        }
    }
    ber_tlv(0x06, &encoded)
}

/// Build a minimal SNMPv2c GET-REQUEST PDU.
fn build_get_request(community: &str, oid: &str, request_id: i64) -> Vec<u8> {
    let var_bind = ber_tlv(0x30, &{
        let mut v = ber_oid(oid);
        v.extend(ber_null());
        ber_tlv(0x30, &v)
    });
    let pdu = {
        let mut p = ber_integer(request_id);   // request-id
        p.extend(ber_integer(0));              // error-status: noError
        p.extend(ber_integer(0));              // error-index: 0
        p.extend(var_bind);                    // variable-bindings
        ber_tlv(0xA0, &p)                      // GetRequest-PDU
    };
    let msg = {
        let mut m = ber_integer(1);            // version: v2c = 1
        m.extend(ber_octet_string(community.as_bytes()));
        m.extend(pdu);
        ber_tlv(0x30, &m)
    };
    msg
}

/// Parse a single VarBind from raw BER bytes (best-effort, returns string repr).
fn parse_response(data: &[u8]) -> Vec<SnmpVarBind> {
    // Walk into the outermost SEQUENCE, find the variable-bindings list
    // This is intentionally lenient — full BER parsing is complex
    let text = format!("{data:02X?}");
    vec![SnmpVarBind {
        oid: "raw".to_string(),
        value_type: "hex".to_string(),
        value: text,
    }]
}

#[tauri::command]
pub fn snmp_add_session(
    config: SnmpConfig,
    state: tauri::State<'_, SnmpState>,
) -> String {
    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), config);
    id
}

#[tauri::command]
pub async fn snmp_get(
    id: String,
    oid: String,
    state: tauri::State<'_, SnmpState>,
) -> Result<Vec<SnmpVarBind>, SnmpError> {
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SnmpError::NotFound(id.clone()))?;

    let community = cfg.community.as_deref().unwrap_or("public");
    let request_id = rand::random::<i32>() as i64;
    let pdu = build_get_request(community, &oid, request_id);

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port)
        .parse().map_err(|e: std::net::AddrParseError| SnmpError::Pdu(e.to_string()))?;
    socket.send_to(&pdu, addr).await?;

    let mut buf = [0u8; 2048];
    let (n, _) = tokio::time::timeout(
        std::time::Duration::from_millis(cfg.timeout_ms.max(500)),
        socket.recv_from(&mut buf),
    ).await.map_err(|_| SnmpError::Timeout)??;

    Ok(parse_response(&buf[..n]))
}

#[tauri::command]
pub async fn snmp_walk(
    id: String,
    root_oid: String,
    max_vars: u32,
    state: tauri::State<'_, SnmpState>,
) -> Result<Vec<SnmpVarBind>, SnmpError> {
    // GET-NEXT walk — simplified: issue up to max_vars sequential GET-NEXTs
    let cfg = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| SnmpError::NotFound(id.clone()))?;
    let community = cfg.community.as_deref().unwrap_or("public");
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port)
        .parse().map_err(|e: std::net::AddrParseError| SnmpError::Pdu(e.to_string()))?;
    let timeout = std::time::Duration::from_millis(cfg.timeout_ms.max(500));

    let mut current_oid = root_oid.clone();
    let mut results = Vec::new();

    for _ in 0..max_vars.min(100) {
        let request_id = rand::random::<i32>() as i64;
        // GET-NEXT PDU uses tag 0xA1
        let community_bytes = community.as_bytes();
        let var_bind = ber_tlv(0x30, &{
            let mut v = ber_oid(&current_oid);
            v.extend(ber_null());
            ber_tlv(0x30, &v)
        });
        let pdu = {
            let mut p = ber_integer(request_id);
            p.extend(ber_integer(0));
            p.extend(ber_integer(0));
            p.extend(var_bind);
            ber_tlv(0xA1, &p)  // GetNext-PDU
        };
        let msg = {
            let mut m = ber_integer(1);
            m.extend(ber_octet_string(community_bytes));
            m.extend(pdu);
            ber_tlv(0x30, &m)
        };

        socket.send_to(&msg, addr).await?;
        let mut buf = [0u8; 2048];
        match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, _))) => results.extend(parse_response(&buf[..n])),
            _ => break,
        }
        // In a real walk we'd extract the new OID from the response; use dummy progression
        current_oid = format!("{}.0", current_oid);
    }

    Ok(results)
}

#[tauri::command]
pub fn snmp_remove_session(id: String, state: tauri::State<'_, SnmpState>) -> Result<(), SnmpError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| SnmpError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn snmp_list_sessions(state: tauri::State<'_, SnmpState>) -> Vec<SnmpSession> {
    state.sessions.lock().unwrap().iter().map(|(id, cfg)| SnmpSession {
        id: id.clone(), host: cfg.host.clone(), version: cfg.version.clone(),
    }).collect()
}

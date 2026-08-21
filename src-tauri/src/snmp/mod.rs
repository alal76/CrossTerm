/// SNMP v1/v2c/v3 client — UDP port 161.
///
/// Encodes/decodes BER (Basic Encoding Rules) for SNMP PDUs by hand, and for
/// v3 implements the User-based Security Model (USM, RFC 3414) from
/// scratch: engine discovery, the password-to-key + key-localization
/// algorithm (Appendix A.2/2.6), HMAC-MD5-96/HMAC-SHA1-96 message
/// authentication, and AES-128-CFB privacy (draft-blumenthal-aes-usm, the
/// de facto modern default — DES privacy is deprecated and not
/// implemented). No mature Rust crate covers SNMPv3 USM, hence hand-rolled.
///
/// The password-to-key expansion (1MB of cyclically-repeated password
/// material hashed in 64-byte chunks) and the AES IV layout (engineBoots
/// || engineTime || 8-byte salt) are cross-checked against Net-SNMP's
/// `keytools.c`/`snmpusm.c` — the reference implementation most agents are
/// validated against — rather than implemented from memory of RFC prose,
/// since a wrong byte order here fails auth silently rather than erroring
/// loudly. Unverified against a real agent — none reachable from this
/// environment.
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
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
    #[error("SNMPv3 authentication failed")]
    AuthFailed,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SnmpV3AuthProtocol {
    Md5,
    Sha1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SnmpV3PrivProtocol {
    None,
    Aes128,
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
    /// SNMPv3 auth passphrase — security level is inferred: None = noAuthNoPriv.
    pub auth_passphrase: Option<String>,
    pub auth_protocol: Option<SnmpV3AuthProtocol>,
    /// SNMPv3 priv passphrase — requires auth_passphrase too (authPriv).
    pub priv_passphrase: Option<String>,
    pub priv_protocol: Option<SnmpV3PrivProtocol>,
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

// ── BER encode ───────────────────────────────────────────────────────────

fn ber_length(len: usize) -> Vec<u8> {
    if len < 128 {
        vec![len as u8]
    } else if len < 256 {
        vec![0x81, len as u8]
    } else {
        vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
    }
}

/// Returns the encoded TLV along with the byte length of its tag+length
/// header — callers that need to compute an absolute offset to a nested
/// field (e.g. where to patch in an HMAC after the fact) use the header
/// length rather than assuming a fixed prefix size, since BER length
/// prefixes vary from 1 to 3 bytes depending on content size.
fn ber_tlv_h(tag: u8, value: &[u8]) -> (Vec<u8>, usize) {
    let len_bytes = ber_length(value.len());
    let header_len = 1 + len_bytes.len();
    let mut v = Vec::with_capacity(header_len + value.len());
    v.push(tag);
    v.extend(len_bytes);
    v.extend_from_slice(value);
    (v, header_len)
}

fn ber_tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    ber_tlv_h(tag, value).0
}

fn ber_sequence(children: &[u8]) -> Vec<u8> {
    ber_tlv(0x30, children)
}

fn ber_integer(n: i64) -> Vec<u8> {
    if n == 0 {
        return ber_tlv(0x02, &[0]);
    }
    let bytes = n.to_be_bytes();
    let is_negative = n < 0;
    let mut start = bytes.iter().position(|&b| if is_negative { b != 0xFF } else { b != 0 }).unwrap_or(7);
    // Ensure the sign bit of the leading byte matches the value's sign
    // (prepend a 0x00/0xFF guard byte otherwise) — required so the
    // receiver's two's-complement interpretation matches.
    if is_negative == (bytes[start] & 0x80 == 0) {
        start -= 1;
    }
    ber_tlv(0x02, &bytes[start..])
}

fn ber_octet_string(s: &[u8]) -> Vec<u8> {
    ber_tlv(0x04, s)
}
fn ber_null() -> Vec<u8> {
    vec![0x05, 0x00]
}
fn ber_oid(oid: &str) -> Vec<u8> {
    let parts: Vec<u64> = oid.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() < 2 {
        return ber_tlv(0x06, &[]);
    }
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

// ── BER decode ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct BerNode<'a> {
    tag: u8,
    value: &'a [u8],
}

/// Parses one TLV off the front of `data`. Only short- and 2/3-byte
/// long-form lengths are supported (0x81/0x82) — SNMP messages over UDP
/// never approach the size where a longer form would appear.
fn ber_parse_one(data: &[u8]) -> Option<(BerNode<'_>, &[u8])> {
    if data.len() < 2 {
        return None;
    }
    let tag = data[0];
    let (len, header_len) = if data[1] & 0x80 == 0 {
        (data[1] as usize, 2)
    } else {
        let n_len_bytes = (data[1] & 0x7F) as usize;
        if n_len_bytes == 0 || n_len_bytes > 4 || data.len() < 2 + n_len_bytes {
            return None;
        }
        let mut len = 0usize;
        for &b in &data[2..2 + n_len_bytes] {
            len = (len << 8) | b as usize;
        }
        (len, 2 + n_len_bytes)
    };
    if data.len() < header_len + len {
        return None;
    }
    let value = &data[header_len..header_len + len];
    let rest = &data[header_len + len..];
    Some((BerNode { tag, value }, rest))
}

/// Parses all sibling TLVs contained in `data` (e.g. the children of a
/// SEQUENCE's value bytes).
fn ber_parse_all(data: &[u8]) -> Vec<BerNode<'_>> {
    let mut out = Vec::new();
    let mut rest = data;
    while let Some((node, remaining)) = ber_parse_one(rest) {
        out.push(node);
        rest = remaining;
    }
    out
}

fn ber_read_int(value: &[u8]) -> i64 {
    if value.is_empty() {
        return 0;
    }
    let negative = value[0] & 0x80 != 0;
    let mut n: i64 = if negative { -1 } else { 0 };
    for &b in value {
        n = (n << 8) | b as i64;
    }
    n
}

fn ber_read_u64(value: &[u8]) -> u64 {
    let mut n: u64 = 0;
    for &b in value {
        n = (n << 8) | b as u64;
    }
    n
}

fn ber_oid_bytes_to_string(value: &[u8]) -> String {
    if value.is_empty() {
        return String::new();
    }
    let mut parts = vec![(value[0] / 40) as u64, (value[0] % 40) as u64];
    let mut acc: u64 = 0;
    for &b in &value[1..] {
        acc = (acc << 7) | (b & 0x7F) as u64;
        if b & 0x80 == 0 {
            parts.push(acc);
            acc = 0;
        }
    }
    parts.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

fn describe_value(tag: u8, value: &[u8]) -> (&'static str, String) {
    match tag {
        0x02 => ("Integer", ber_read_int(value).to_string()),
        0x04 => ("OctetString", display_octet_string(value)),
        0x05 => ("Null", String::new()),
        0x06 => ("ObjectIdentifier", ber_oid_bytes_to_string(value)),
        0x40 => ("IpAddress", value.iter().map(u8::to_string).collect::<Vec<_>>().join(".")),
        0x41 => ("Counter32", ber_read_u64(value).to_string()),
        0x42 => ("Gauge32", ber_read_u64(value).to_string()),
        0x43 => ("TimeTicks", ber_read_u64(value).to_string()),
        0x44 => ("Opaque", hex_string(value)),
        0x46 => ("Counter64", ber_read_u64(value).to_string()),
        0x80 => ("NoSuchObject", String::new()),
        0x81 => ("NoSuchInstance", String::new()),
        0x82 => ("EndOfMibView", String::new()),
        _ => ("Unknown", hex_string(value)),
    }
}

fn display_octet_string(value: &[u8]) -> String {
    if value.iter().all(|&b| (0x20..0x7F).contains(&b)) {
        String::from_utf8_lossy(value).into_owned()
    } else {
        hex_string(value)
    }
}

fn hex_string(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

struct ParsedVarBind {
    oid: String,
    tag: u8,
    value: Vec<u8>,
}

struct ParsedPdu {
    error_status: i64,
    varbinds: Vec<ParsedVarBind>,
}

fn parse_pdu(pdu: &BerNode) -> Option<ParsedPdu> {
    let fields = ber_parse_all(pdu.value);
    // request-id, error-status, error-index, variable-bindings(SEQUENCE)
    if fields.len() < 4 {
        return None;
    }
    let error_status = ber_read_int(fields[1].value);
    let var_bind_list = ber_parse_all(fields[3].value);
    let mut varbinds = Vec::new();
    for vb in var_bind_list {
        let pair = ber_parse_all(vb.value);
        if pair.len() < 2 {
            continue;
        }
        varbinds.push(ParsedVarBind {
            oid: ber_oid_bytes_to_string(pair[0].value),
            tag: pair[1].tag,
            value: pair[1].value.to_vec(),
        });
    }
    Some(ParsedPdu { error_status, varbinds })
}

fn varbinds_to_response(pdu: &ParsedPdu) -> Vec<SnmpVarBind> {
    pdu.varbinds
        .iter()
        .map(|vb| {
            let (type_name, value) = describe_value(vb.tag, &vb.value);
            SnmpVarBind { oid: vb.oid.clone(), value_type: type_name.to_string(), value }
        })
        .collect()
}

/// Parses a v1/v2c response message (SEQUENCE{version, community, PDU}).
fn parse_v1v2c_response(data: &[u8]) -> Result<ParsedPdu, SnmpError> {
    let (top, _) = ber_parse_one(data).ok_or_else(|| SnmpError::Pdu("Malformed response".into()))?;
    let fields = ber_parse_all(top.value);
    if fields.len() < 3 {
        return Err(SnmpError::Pdu("Malformed response envelope".into()));
    }
    parse_pdu(&fields[2]).ok_or_else(|| SnmpError::Pdu("Malformed PDU".into()))
}

// ── PDU builders (shared by v1/v2c and v3's scopedPDU) ──────────────────

fn build_var_bind_list(oid: &str) -> Vec<u8> {
    let inner = {
        let mut v = ber_oid(oid);
        v.extend(ber_null());
        v
    };
    ber_sequence(&ber_sequence(&inner))
}

fn build_pdu(pdu_tag: u8, request_id: i64, oid: &str, non_repeaters: Option<i64>, max_reps: Option<i64>) -> Vec<u8> {
    let mut p = ber_integer(request_id);
    p.extend(ber_integer(non_repeaters.unwrap_or(0)));
    p.extend(ber_integer(max_reps.unwrap_or(0)));
    p.extend(build_var_bind_list(oid));
    ber_tlv(pdu_tag, &p)
}

const PDU_GET: u8 = 0xA0;
const PDU_GET_NEXT: u8 = 0xA1;

fn build_v1v2c_message(version: &SnmpVersion, community: &str, pdu_tag: u8, request_id: i64, oid: &str) -> Vec<u8> {
    let version_num = match version {
        SnmpVersion::V1 => 0,
        _ => 1,
    };
    let mut m = ber_integer(version_num);
    m.extend(ber_octet_string(community.as_bytes()));
    m.extend(build_pdu(pdu_tag, request_id, oid, None, None));
    ber_sequence(&m)
}

// ── SNMPv3 USM: password-to-key + key localization (RFC 3414 Appendix A) ──

const EXPANDED_PASSPHRASE_LEN: usize = 1024 * 1024;
const KU_HASH_BLOCK: usize = 64;

fn password_to_key_md5(password: &[u8]) -> [u8; 16] {
    let mut hasher = <Md5 as md5::Digest>::new();
    let mut remaining = EXPANDED_PASSPHRASE_LEN;
    let mut pindex = 0usize;
    let mut buf = [0u8; KU_HASH_BLOCK];
    while remaining > 0 {
        for slot in buf.iter_mut() {
            *slot = password[pindex % password.len()];
            pindex += 1;
        }
        md5::Digest::update(&mut hasher, buf);
        remaining -= KU_HASH_BLOCK;
    }
    md5::Digest::finalize(hasher).into()
}

fn password_to_key_sha1(password: &[u8]) -> [u8; 20] {
    use sha1::Digest;
    let mut hasher = Sha1::new();
    let mut remaining = EXPANDED_PASSPHRASE_LEN;
    let mut pindex = 0usize;
    let mut buf = [0u8; KU_HASH_BLOCK];
    while remaining > 0 {
        for slot in buf.iter_mut() {
            *slot = password[pindex % password.len()];
            pindex += 1;
        }
        Digest::update(&mut hasher, buf);
        remaining -= KU_HASH_BLOCK;
    }
    hasher.finalize().into()
}

/// Localizes Ku to this specific agent's engine ID: Kul = Hash(Ku || engineID || Ku).
fn localize_key(proto: &SnmpV3AuthProtocol, ku: &[u8], engine_id: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(ku.len() * 2 + engine_id.len());
    buf.extend_from_slice(ku);
    buf.extend_from_slice(engine_id);
    buf.extend_from_slice(ku);
    match proto {
        SnmpV3AuthProtocol::Md5 => {
            let digest = <Md5 as md5::Digest>::digest(&buf);
            digest.to_vec()
        }
        SnmpV3AuthProtocol::Sha1 => {
            use sha1::Digest;
            Sha1::digest(&buf).to_vec()
        }
    }
}

fn localized_key(proto: &SnmpV3AuthProtocol, password: &str, engine_id: &[u8]) -> Vec<u8> {
    let ku: Vec<u8> = match proto {
        SnmpV3AuthProtocol::Md5 => password_to_key_md5(password.as_bytes()).to_vec(),
        SnmpV3AuthProtocol::Sha1 => password_to_key_sha1(password.as_bytes()).to_vec(),
    };
    localize_key(proto, &ku, engine_id)
}

fn hmac_truncated_12(proto: &SnmpV3AuthProtocol, key: &[u8], data: &[u8]) -> [u8; 12] {
    let mut out = [0u8; 12];
    match proto {
        SnmpV3AuthProtocol::Md5 => {
            let mut mac = <Hmac<Md5> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            out.copy_from_slice(&mac.finalize().into_bytes()[..12]);
        }
        SnmpV3AuthProtocol::Sha1 => {
            let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            out.copy_from_slice(&mac.finalize().into_bytes()[..12]);
        }
    }
    out
}

// ── SNMPv3 AES-128-CFB privacy (draft-blumenthal-aes-usm §3.1.2.2) ───────

/// Full-block AES-128-CFB (CFB128, not CFB1/CFB8) using the raw block
/// cipher — SNMP's privacy transform doesn't pad; ciphertext is exactly the
/// plaintext length, unlike CBC.
fn aes_cfb128(key16: &[u8], iv16: &[u8; 16], data: &[u8], encrypt: bool) -> Vec<u8> {
    use aes::cipher::{BlockEncrypt, KeyInit};
    let cipher = aes::Aes128::new(key16.into());
    let mut out = Vec::with_capacity(data.len());
    let mut feedback = *iv16;
    for chunk in data.chunks(16) {
        let mut keystream = feedback;
        let block = aes::cipher::generic_array::GenericArray::from_mut_slice(&mut keystream);
        cipher.encrypt_block(block);
        let mut ct_chunk = vec![0u8; chunk.len()];
        for i in 0..chunk.len() {
            ct_chunk[i] = chunk[i] ^ keystream[i];
        }
        feedback = [0u8; 16];
        // Next feedback block is the ciphertext (encrypting) or the
        // original ciphertext just consumed (decrypting) — CFB feeds back
        // whichever side carries the actual transmitted bytes.
        let feedback_source = if encrypt { &ct_chunk } else { chunk };
        feedback[..feedback_source.len()].copy_from_slice(feedback_source);
        out.extend_from_slice(&ct_chunk);
    }
    out
}

// ── SNMPv3 message assembly ───────────────────────────────────────────────

struct V3Engine {
    id: Vec<u8>,
    boots: i64,
    time: i64,
}

const SECURITY_MODEL_USM: i64 = 3;

fn build_msg_global_data(msg_id: i64, flags: u8) -> Vec<u8> {
    let mut g = ber_integer(msg_id);
    g.extend(ber_integer(65507)); // msgMaxSize
    g.extend(ber_octet_string(&[flags]));
    g.extend(ber_integer(SECURITY_MODEL_USM));
    ber_sequence(&g)
}

/// Builds a full SNMPv3 message. `auth` is `Some(protocol)` to sign it;
/// `priv_key` is `Some((protocol, key))` to encrypt the scopedPDU. Returns
/// the message along with the absolute byte offset of the (still-zeroed)
/// msgAuthenticationParameters content, so the caller can compute the real
/// HMAC over the assembled bytes and patch it in afterward — the only way
/// to sign a message that contains a placeholder for its own signature.
#[allow(clippy::too_many_arguments)]
fn build_v3_message(
    engine: &V3Engine,
    username: &str,
    auth_key: Option<&(SnmpV3AuthProtocol, Vec<u8>)>,
    priv_key: Option<&[u8]>,
    msg_id: i64,
    pdu_bytes: &[u8],
) -> (Vec<u8>, Option<usize>) {
    let auth_flag = if auth_key.is_some() { 0x01 } else { 0x00 };
    let priv_flag = if priv_key.is_some() { 0x02 } else { 0x00 };
    let flags = auth_flag | priv_flag | 0x04; // reportable

    let scoped_pdu = {
        let mut s = ber_octet_string(&engine.id);
        s.extend(ber_octet_string(b""));
        s.extend_from_slice(pdu_bytes);
        ber_sequence(&s)
    };

    let mut salt = [0u8; 8];
    let (msg_data, priv_params): (Vec<u8>, Vec<u8>) = if let Some(key) = priv_key {
        rand::thread_rng().fill_bytes(&mut salt);
        let mut iv = [0u8; 16];
        iv[..4].copy_from_slice(&(engine.boots as u32).to_be_bytes());
        iv[4..8].copy_from_slice(&(engine.time as u32).to_be_bytes());
        iv[8..].copy_from_slice(&salt);
        let ciphertext = aes_cfb128(key, &iv, &scoped_pdu, true);
        (ber_octet_string(&ciphertext), salt.to_vec())
    } else {
        (scoped_pdu, Vec::new())
    };

    let engine_id_tlv = ber_octet_string(&engine.id);
    let boots_tlv = ber_integer(engine.boots);
    let time_tlv = ber_integer(engine.time);
    let username_tlv = ber_octet_string(username.as_bytes());
    let (auth_params_tlv, auth_params_header_len) = ber_tlv_h(0x04, if auth_key.is_some() { &[0u8; 12] } else { &[] });
    let priv_params_tlv = ber_octet_string(&priv_params);

    let offset_in_sec_children = engine_id_tlv.len() + boots_tlv.len() + time_tlv.len() + username_tlv.len() + auth_params_header_len;

    let mut sec_children = Vec::new();
    sec_children.extend_from_slice(&engine_id_tlv);
    sec_children.extend_from_slice(&boots_tlv);
    sec_children.extend_from_slice(&time_tlv);
    sec_children.extend_from_slice(&username_tlv);
    sec_children.extend_from_slice(&auth_params_tlv);
    sec_children.extend_from_slice(&priv_params_tlv);

    let (sec_seq, sec_seq_header_len) = ber_tlv_h(0x30, &sec_children);
    let offset_in_sec_seq = sec_seq_header_len + offset_in_sec_children;

    let (sec_octet_string, sec_os_header_len) = ber_tlv_h(0x04, &sec_seq);
    let offset_in_sec_os = sec_os_header_len + offset_in_sec_seq;

    let version_tlv = ber_integer(3);
    let global_data_tlv = build_msg_global_data(msg_id, flags);

    let offset_in_top_children = version_tlv.len() + global_data_tlv.len() + offset_in_sec_os;

    let mut top_children = Vec::new();
    top_children.extend_from_slice(&version_tlv);
    top_children.extend_from_slice(&global_data_tlv);
    top_children.extend_from_slice(&sec_octet_string);
    top_children.extend_from_slice(&msg_data);

    let (full_message, top_header_len) = ber_tlv_h(0x30, &top_children);
    let auth_offset = auth_key.map(|_| top_header_len + offset_in_top_children);

    (full_message, auth_offset)
}

fn sign_v3_message(mut message: Vec<u8>, auth_offset: Option<usize>, auth: Option<&(SnmpV3AuthProtocol, Vec<u8>)>) -> Vec<u8> {
    if let (Some(offset), Some((proto, key))) = (auth_offset, auth) {
        let mac = hmac_truncated_12(proto, key, &message);
        message[offset..offset + 12].copy_from_slice(&mac);
    }
    message
}

struct V3Response {
    engine: V3Engine,
    pdu: Option<ParsedPdu>,
}

fn parse_v3_response(data: &[u8], priv_key: Option<&[u8]>) -> Result<V3Response, SnmpError> {
    let (top, _) = ber_parse_one(data).ok_or_else(|| SnmpError::Pdu("Malformed v3 response".into()))?;
    let fields = ber_parse_all(top.value);
    if fields.len() < 4 {
        return Err(SnmpError::Pdu("Malformed v3 envelope".into()));
    }
    // fields[2] = msgSecurityParameters OCTET STRING wrapping a SEQUENCE
    let (sec_seq, _) = ber_parse_one(fields[2].value).ok_or_else(|| SnmpError::Pdu("Malformed security parameters".into()))?;
    let sec_fields = ber_parse_all(sec_seq.value);
    if sec_fields.len() < 6 {
        return Err(SnmpError::Pdu("Malformed USM security parameters".into()));
    }
    let engine = V3Engine {
        id: sec_fields[0].value.to_vec(),
        boots: ber_read_int(sec_fields[1].value),
        time: ber_read_int(sec_fields[2].value),
    };

    // fields[3] = msgData: either a plaintext scopedPDU SEQUENCE (tag 0x30)
    // or an encrypted OCTET STRING (tag 0x04) that decrypts to one.
    let scoped_pdu_bytes: Vec<u8> = if fields[3].tag == 0x04 {
        let key = priv_key.ok_or_else(|| SnmpError::Pdu("Response is encrypted but no privacy key is configured".into()))?;
        let priv_params = sec_fields[5].value;
        if priv_params.len() != 8 {
            return Err(SnmpError::Pdu("Malformed privacy parameters".into()));
        }
        let mut iv = [0u8; 16];
        iv[..4].copy_from_slice(&(engine.boots as u32).to_be_bytes());
        iv[4..8].copy_from_slice(&(engine.time as u32).to_be_bytes());
        iv[8..].copy_from_slice(priv_params);
        aes_cfb128(key, &iv, fields[3].value, false)
    } else {
        fields[3].value.to_vec()
    };

    let (scoped_pdu, _) = ber_parse_one(&scoped_pdu_bytes).ok_or_else(|| SnmpError::Pdu("Malformed scopedPDU".into()))?;
    let scoped_fields = ber_parse_all(scoped_pdu.value);
    if scoped_fields.len() < 3 {
        // A pure discovery Report may carry no usable PDU — return the
        // engine info we already have without erroring.
        return Ok(V3Response { engine, pdu: None });
    }
    let pdu = parse_pdu(&scoped_fields[2]);
    Ok(V3Response { engine, pdu })
}

async fn v3_discover(socket: &UdpSocket, addr: SocketAddr, timeout: std::time::Duration) -> Result<V3Engine, SnmpError> {
    let engine = V3Engine { id: Vec::new(), boots: 0, time: 0 };
    let pdu = build_pdu(PDU_GET, rand::random::<i32>() as i64, "1.3.6.1.2.1.1.1.0", None, None);
    let (message, _) = build_v3_message(&engine, "", None, None, rand::random::<i32>() as i64, &pdu);
    socket.send_to(&message, addr).await?;
    let mut buf = [0u8; 2048];
    let (n, _) = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await.map_err(|_| SnmpError::Timeout)??;
    let resp = parse_v3_response(&buf[..n], None)?;
    if resp.engine.id.is_empty() {
        return Err(SnmpError::Pdu("Agent did not return an engine ID during discovery".into()));
    }
    Ok(resp.engine)
}

struct V3Keys {
    auth: Option<(SnmpV3AuthProtocol, Vec<u8>)>,
    privacy: Option<Vec<u8>>,
}

fn derive_v3_keys(cfg: &SnmpConfig, engine_id: &[u8]) -> V3Keys {
    let auth = cfg.auth_passphrase.as_ref().map(|pw| {
        let proto = cfg.auth_protocol.clone().unwrap_or(SnmpV3AuthProtocol::Sha1);
        let key = localized_key(&proto, pw, engine_id);
        (proto, key)
    });
    let privacy = match (&auth, &cfg.priv_passphrase, cfg.priv_protocol.clone().unwrap_or(SnmpV3PrivProtocol::Aes128)) {
        (Some((auth_proto, _)), Some(priv_pw), SnmpV3PrivProtocol::Aes128) => {
            // Privacy keys are localized with the *auth* protocol's hash,
            // per RFC 3414 §1.6's note that privacy transforms reuse the
            // authentication hash — there's no separate "privacy hash".
            Some(localized_key(auth_proto, priv_pw, engine_id)[..16].to_vec())
        }
        _ => None,
    };
    V3Keys { auth, privacy }
}

async fn v3_request(
    socket: &UdpSocket,
    addr: SocketAddr,
    cfg: &SnmpConfig,
    timeout: std::time::Duration,
    pdu_tag: u8,
    oid: &str,
) -> Result<Vec<SnmpVarBind>, SnmpError> {
    let username = cfg.username.as_deref().unwrap_or("");
    let engine = v3_discover(socket, addr, timeout).await?;
    let keys = derive_v3_keys(cfg, &engine.id);

    let pdu = build_pdu(pdu_tag, rand::random::<i32>() as i64, oid, None, None);
    let (message, auth_offset) = build_v3_message(&engine, username, keys.auth.as_ref(), keys.privacy.as_deref(), rand::random::<i32>() as i64, &pdu);
    let message = sign_v3_message(message, auth_offset, keys.auth.as_ref());

    socket.send_to(&message, addr).await?;
    let mut buf = [0u8; 4096];
    let (n, _) = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await.map_err(|_| SnmpError::Timeout)??;
    let resp = parse_v3_response(&buf[..n], keys.privacy.as_deref())?;
    let pdu = resp.pdu.ok_or(SnmpError::AuthFailed)?;
    if pdu.error_status != 0 {
        return Err(SnmpError::Pdu(format!("Agent returned error-status {}", pdu.error_status)));
    }
    Ok(varbinds_to_response(&pdu))
}

// ── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn snmp_add_session(config: SnmpConfig, state: tauri::State<'_, SnmpState>) -> String {
    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), config);
    id
}

#[tauri::command]
pub async fn snmp_get(id: String, oid: String, state: tauri::State<'_, SnmpState>) -> Result<Vec<SnmpVarBind>, SnmpError> {
    let cfg = state.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| SnmpError::NotFound(id.clone()))?;
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse().map_err(|e: std::net::AddrParseError| SnmpError::Pdu(e.to_string()))?;
    let timeout = std::time::Duration::from_millis(cfg.timeout_ms.max(500));

    if cfg.version == SnmpVersion::V3 {
        return v3_request(&socket, addr, &cfg, timeout, PDU_GET, &oid).await;
    }

    let community = cfg.community.as_deref().unwrap_or("public");
    let message = build_v1v2c_message(&cfg.version, community, PDU_GET, rand::random::<i32>() as i64, &oid);
    socket.send_to(&message, addr).await?;
    let mut buf = [0u8; 2048];
    let (n, _) = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await.map_err(|_| SnmpError::Timeout)??;
    let pdu = parse_v1v2c_response(&buf[..n])?;
    Ok(varbinds_to_response(&pdu))
}

#[tauri::command]
pub async fn snmp_walk(id: String, root_oid: String, max_vars: u32, state: tauri::State<'_, SnmpState>) -> Result<Vec<SnmpVarBind>, SnmpError> {
    let cfg = state.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| SnmpError::NotFound(id.clone()))?;
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse().map_err(|e: std::net::AddrParseError| SnmpError::Pdu(e.to_string()))?;
    let timeout = std::time::Duration::from_millis(cfg.timeout_ms.max(500));

    let mut current_oid = root_oid.clone();
    let mut results = Vec::new();

    for _ in 0..max_vars.min(100) {
        let varbinds = if cfg.version == SnmpVersion::V3 {
            match v3_request(&socket, addr, &cfg, timeout, PDU_GET_NEXT, &current_oid).await {
                Ok(v) => v,
                Err(_) => break,
            }
        } else {
            let community = cfg.community.as_deref().unwrap_or("public");
            let message = build_v1v2c_message(&cfg.version, community, PDU_GET_NEXT, rand::random::<i32>() as i64, &current_oid);
            socket.send_to(&message, addr).await?;
            let mut buf = [0u8; 2048];
            let recv = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await;
            match recv {
                Ok(Ok((n, _))) => match parse_v1v2c_response(&buf[..n]) {
                    Ok(pdu) if pdu.error_status == 0 => varbinds_to_response(&pdu),
                    _ => break,
                },
                _ => break,
            }
        };

        let Some(vb) = varbinds.first() else { break };
        // Lexicographic-subtree convention: stop once the walk leaves the
        // requested root, or the agent signals end-of-MIB-view.
        if vb.value_type == "EndOfMibView" || !vb.oid.starts_with(&format!("{root_oid}.")) {
            break;
        }
        current_oid = vb.oid.clone();
        results.push(vb.clone());
    }

    Ok(results)
}

#[tauri::command]
pub fn snmp_remove_session(id: String, state: tauri::State<'_, SnmpState>) -> Result<(), SnmpError> {
    state.sessions.lock().unwrap().remove(&id).ok_or(SnmpError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn snmp_list_sessions(state: tauri::State<'_, SnmpState>) -> Vec<SnmpSession> {
    state.sessions.lock().unwrap().iter().map(|(id, cfg)| SnmpSession { id: id.clone(), host: cfg.host.clone(), version: cfg.version.clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ber_integer_roundtrip_positive_and_negative() {
        for n in [0i64, 1, 127, 128, 255, 256, 65535, -1, -128, -129] {
            let encoded = ber_integer(n);
            let (node, _) = ber_parse_one(&encoded).unwrap();
            assert_eq!(node.tag, 0x02);
            assert_eq!(ber_read_int(node.value), n, "roundtrip failed for {n}");
        }
    }

    #[test]
    fn test_ber_oid_roundtrip() {
        let oid = "1.3.6.1.2.1.1.1.0";
        let encoded = ber_oid(oid);
        let (node, _) = ber_parse_one(&encoded).unwrap();
        assert_eq!(ber_oid_bytes_to_string(node.value), oid);
    }

    #[test]
    fn test_ber_oid_roundtrip_multibyte_arc() {
        // An arc >= 128 needs multi-byte base-128 encoding.
        let oid = "1.3.6.1.4.1.9999.200";
        let encoded = ber_oid(oid);
        let (node, _) = ber_parse_one(&encoded).unwrap();
        assert_eq!(ber_oid_bytes_to_string(node.value), oid);
    }

    #[test]
    fn test_ber_parse_all_walks_sequence_children() {
        let seq = ber_sequence(&[ber_integer(1), ber_octet_string(b"public")].concat());
        let (node, _) = ber_parse_one(&seq).unwrap();
        let children = ber_parse_all(node.value);
        assert_eq!(children.len(), 2);
        assert_eq!(ber_read_int(children[0].value), 1);
        assert_eq!(children[1].value, b"public");
    }

    #[test]
    fn test_build_and_parse_v1v2c_get_request_response_shape() {
        // Build a GetRequest, then build a plausible GetResponse with one
        // varbind and confirm parse_v1v2c_response extracts it correctly.
        let request_id = 42i64;
        let var_bind = {
            let mut v = ber_oid("1.3.6.1.2.1.1.1.0");
            v.extend(ber_octet_string(b"Linux server"));
            ber_sequence(&v)
        };
        let pdu = {
            let mut p = ber_integer(request_id);
            p.extend(ber_integer(0));
            p.extend(ber_integer(0));
            p.extend(ber_sequence(&var_bind));
            ber_tlv(0xA2, &p) // GetResponse-PDU
        };
        let msg = {
            let mut m = ber_integer(1);
            m.extend(ber_octet_string(b"public"));
            m.extend(pdu);
            ber_sequence(&m)
        };

        let parsed = parse_v1v2c_response(&msg).unwrap();
        assert_eq!(parsed.error_status, 0);
        assert_eq!(parsed.varbinds.len(), 1);
        assert_eq!(parsed.varbinds[0].oid, "1.3.6.1.2.1.1.1.0");
        let (type_name, value) = describe_value(parsed.varbinds[0].tag, &parsed.varbinds[0].value);
        assert_eq!(type_name, "OctetString");
        assert_eq!(value, "Linux server");
    }

    #[test]
    fn test_describe_value_typed_variants() {
        assert_eq!(describe_value(0x02, &ber_integer(5)[2..]).0, "Integer");
        assert_eq!(describe_value(0x43, &[0, 0, 0, 100]).0, "TimeTicks");
        assert_eq!(describe_value(0x43, &[0, 0, 0, 100]).1, "100");
        assert_eq!(describe_value(0x40, &[192, 168, 1, 1]).1, "192.168.1.1");
        assert_eq!(describe_value(0x82, &[]).0, "EndOfMibView");
    }

    #[test]
    fn test_password_to_key_md5_matches_known_rfc3414_test_vector() {
        // RFC 3414 Appendix A.3.1, verified against the RFC text directly
        // (not from memory): password "maplesyrup" -> Ku.
        let ku = password_to_key_md5(b"maplesyrup");
        assert_eq!(hex_string(&ku).replace(' ', "").to_lowercase(), "9faf3283884e92834ebc9847d8edd963");
    }

    #[test]
    fn test_password_to_key_sha1_matches_known_rfc3414_test_vector() {
        // RFC 3414 Appendix A.3.2, verified against the RFC text directly
        // (not from memory): password "maplesyrup" -> Ku.
        let ku = password_to_key_sha1(b"maplesyrup");
        assert_eq!(hex_string(&ku).replace(' ', "").to_lowercase(), "9fb5cc0381497b3793528939ff788d5d79145211");
    }

    #[test]
    fn test_localize_key_md5_matches_known_rfc3414_test_vector() {
        // RFC 3414 Appendix A.3.1: Ku (from "maplesyrup") localized with
        // engineID 000000000000000000000002 -> Kul.
        let ku = password_to_key_md5(b"maplesyrup");
        let engine_id = hex_to_bytes("000000000000000000000002");
        let kul = localize_key(&SnmpV3AuthProtocol::Md5, &ku, &engine_id);
        assert_eq!(hex_string(&kul).replace(' ', "").to_lowercase(), "526f5eed9fcce26f8964c2930787d82b");
    }

    #[test]
    fn test_localize_key_sha1_matches_known_rfc3414_test_vector() {
        // RFC 3414 Appendix A.3.2: Ku (from "maplesyrup") localized with
        // engineID 000000000000000000000002 -> Kul.
        let ku = password_to_key_sha1(b"maplesyrup");
        let engine_id = hex_to_bytes("000000000000000000000002");
        let kul = localize_key(&SnmpV3AuthProtocol::Sha1, &ku, &engine_id);
        assert_eq!(hex_string(&kul).replace(' ', "").to_lowercase(), "6695febc9288e36282235fc7151f128497b38f3f");
    }

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn test_hmac_truncated_12_produces_12_bytes() {
        let mac = hmac_truncated_12(&SnmpV3AuthProtocol::Sha1, b"some-localized-key-here", b"message bytes");
        assert_eq!(mac.len(), 12);
        let mac_md5 = hmac_truncated_12(&SnmpV3AuthProtocol::Md5, b"some-localized-key-here", b"message bytes");
        assert_eq!(mac_md5.len(), 12);
    }

    #[test]
    fn test_aes_cfb128_encrypt_decrypt_roundtrip() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 16];
        let plaintext = b"a scopedPDU that is not block-aligned!!";
        let ciphertext = aes_cfb128(&key, &iv, plaintext, true);
        assert_eq!(ciphertext.len(), plaintext.len());
        assert_ne!(ciphertext, plaintext);
        let decrypted = aes_cfb128(&key, &iv, &ciphertext, false);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aes_cfb128_non_block_aligned_length() {
        // 20 bytes: one full 16-byte block plus a 4-byte partial block.
        let key = [0x33u8; 16];
        let iv = [0x44u8; 16];
        let plaintext = vec![7u8; 20];
        let ciphertext = aes_cfb128(&key, &iv, &plaintext, true);
        let decrypted = aes_cfb128(&key, &iv, &ciphertext, false);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_build_v3_message_auth_offset_points_at_a_zeroed_12_byte_span() {
        let engine = V3Engine { id: vec![0x80, 0x00, 0x1f, 0x88, 0x80], boots: 1, time: 100 };
        let auth = (SnmpV3AuthProtocol::Sha1, vec![0xAB; 20]);
        let pdu = build_pdu(PDU_GET, 1, "1.3.6.1.2.1.1.1.0", None, None);
        let (message, offset) = build_v3_message(&engine, "admin", Some(&auth), None, 1, &pdu);
        let offset = offset.unwrap();
        assert_eq!(&message[offset..offset + 12], &[0u8; 12]);
    }

    #[test]
    fn test_sign_v3_message_patches_the_hmac_in_place_without_changing_length() {
        let engine = V3Engine { id: vec![0x80, 0x00, 0x1f, 0x88, 0x80], boots: 1, time: 100 };
        let auth = (SnmpV3AuthProtocol::Sha1, vec![0xAB; 20]);
        let pdu = build_pdu(PDU_GET, 1, "1.3.6.1.2.1.1.1.0", None, None);
        let (message, offset) = build_v3_message(&engine, "admin", Some(&auth), None, 1, &pdu);
        let original_len = message.len();
        let signed = sign_v3_message(message, offset, Some(&auth));
        assert_eq!(signed.len(), original_len);
        let offset = offset.unwrap();
        assert_ne!(&signed[offset..offset + 12], &[0u8; 12]);

        // Re-parse and confirm the message is still well-formed BER after patching.
        let (top, rest) = ber_parse_one(&signed).unwrap();
        assert_eq!(top.tag, 0x30);
        assert!(rest.is_empty());
    }

    #[test]
    fn test_build_v3_message_no_auth_no_priv_has_empty_auth_and_priv_params() {
        let engine = V3Engine { id: vec![0x80, 0x00, 0x1f, 0x88, 0x80], boots: 0, time: 0 };
        let pdu = build_pdu(PDU_GET, 1, "1.3.6.1.2.1.1.1.0", None, None);
        let (message, offset) = build_v3_message(&engine, "", None, None, 1, &pdu);
        assert!(offset.is_none());
        // scopedPDU should be present as a plaintext SEQUENCE, not an
        // encrypted OCTET STRING.
        let (top, _) = ber_parse_one(&message).unwrap();
        let fields = ber_parse_all(top.value);
        assert_eq!(fields[3].tag, 0x30);
    }

    #[test]
    fn test_v3_message_round_trip_authpriv() {
        let engine = V3Engine { id: vec![0x80, 0x00, 0x1f, 0x88, 0x80, 0x01, 0x02, 0x03], boots: 3, time: 12345 };
        let auth = (SnmpV3AuthProtocol::Sha1, vec![0xCD; 20]);
        let priv_key = vec![0xEF; 16];
        let pdu = build_pdu(PDU_GET, 7, "1.3.6.1.2.1.1.5.0", None, None);
        let (message, offset) = build_v3_message(&engine, "monitor", Some(&auth), Some(&priv_key), 9, &pdu);
        let message = sign_v3_message(message, offset, Some(&auth));

        let resp = parse_v3_response(&message, Some(&priv_key)).unwrap();
        assert_eq!(resp.engine.id, engine.id);
        assert_eq!(resp.engine.boots, 3);
        assert_eq!(resp.engine.time, 12345);
        let pdu = resp.pdu.expect("scopedPDU should decrypt and parse");
        assert_eq!(pdu.varbinds[0].oid, "1.3.6.1.2.1.1.5.0");
    }

    #[test]
    fn test_walk_stops_at_end_of_mib_view() {
        let (type_name, _) = describe_value(0x82, &[]);
        assert_eq!(type_name, "EndOfMibView");
    }

    #[test]
    fn test_ber_length_uses_short_and_both_long_forms() {
        assert_eq!(ber_length(0), vec![0]);
        assert_eq!(ber_length(127), vec![127]);
        assert_eq!(ber_length(128), vec![0x81, 128]);
        assert_eq!(ber_length(255), vec![0x81, 255]);
        assert_eq!(ber_length(256), vec![0x82, 1, 0]);
        assert_eq!(ber_length(300), vec![0x82, 1, 44]);
    }

    #[test]
    fn test_ber_tlv_h_reports_correct_header_length_for_each_length_form() {
        let (_, h) = ber_tlv_h(0x04, &vec![0u8; 10]);
        assert_eq!(h, 2); // tag + 1-byte length
        let (_, h) = ber_tlv_h(0x04, &vec![0u8; 200]);
        assert_eq!(h, 3); // tag + 0x81 + 1 length byte
        let (_, h) = ber_tlv_h(0x04, &vec![0u8; 300]);
        assert_eq!(h, 4); // tag + 0x82 + 2 length bytes
    }

    #[test]
    fn test_display_octet_string_falls_back_to_hex_for_non_printable_bytes() {
        assert_eq!(display_octet_string(b"hello"), "hello");
        assert_eq!(display_octet_string(&[0x00, 0x01, 0xFF]), "00 01 FF");
    }

    #[test]
    fn test_hex_string_formats_uppercase_pairs() {
        assert_eq!(hex_string(&[0xDE, 0xAD, 0xBE, 0xEF]), "DE AD BE EF");
        assert_eq!(hex_string(&[]), "");
    }

    #[test]
    fn test_ber_read_u64_big_endian() {
        assert_eq!(ber_read_u64(&[0x00, 0x00, 0x01, 0x00]), 256);
        assert_eq!(ber_read_u64(&[0xFF, 0xFF, 0xFF, 0xFF]), 0xFFFF_FFFF);
    }

    #[test]
    fn test_ber_oid_with_fewer_than_two_arcs_encodes_empty_value() {
        let encoded = ber_oid("1");
        let (node, _) = ber_parse_one(&encoded).unwrap();
        assert_eq!(node.tag, 0x06);
        assert!(node.value.is_empty());
    }

    #[test]
    fn test_ber_parse_one_returns_none_on_truncated_input() {
        assert!(ber_parse_one(&[]).is_none());
        assert!(ber_parse_one(&[0x02]).is_none()); // tag with no length byte
        assert!(ber_parse_one(&[0x02, 0x05, 0x01]).is_none()); // length says 5, only 1 byte present
    }

    #[test]
    fn test_ber_parse_one_rejects_malformed_long_form_length() {
        // 0x84 => 4 length bytes claimed, but none follow.
        assert!(ber_parse_one(&[0x04, 0x84]).is_none());
        // 0x80 => "indefinite length" (0 length-bytes), not supported here.
        assert!(ber_parse_one(&[0x04, 0x80]).is_none());
    }

    #[test]
    fn test_parse_pdu_returns_none_when_fields_are_missing() {
        // A PDU needs request-id, error-status, error-index, varbind-list (4 fields).
        let short_pdu = ber_tlv(0xA0, &ber_integer(1));
        let (node, _) = ber_parse_one(&short_pdu).unwrap();
        assert!(parse_pdu(&node).is_none());
    }

    #[test]
    fn test_build_var_bind_list_wraps_oid_and_null_in_nested_sequences() {
        let vbl = build_var_bind_list("1.3.6.1.2.1.1.1.0");
        let (outer, _) = ber_parse_one(&vbl).unwrap();
        assert_eq!(outer.tag, 0x30);
        let inner_seqs = ber_parse_all(outer.value);
        assert_eq!(inner_seqs.len(), 1);
        let pair = ber_parse_all(inner_seqs[0].value);
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].tag, 0x06); // OID
        assert_eq!(pair[1].tag, 0x05); // NULL
    }

    #[test]
    fn test_build_pdu_encodes_request_id_and_repeaters() {
        let pdu = build_pdu(PDU_GET_NEXT, 99, "1.3.6.1.2.1.1.1.0", Some(2), Some(10));
        let (node, _) = ber_parse_one(&pdu).unwrap();
        assert_eq!(node.tag, PDU_GET_NEXT);
        let fields = ber_parse_all(node.value);
        assert_eq!(ber_read_int(fields[0].value), 99); // request-id
        assert_eq!(ber_read_int(fields[1].value), 2); // non-repeaters
        assert_eq!(ber_read_int(fields[2].value), 10); // max-repetitions
    }

    #[test]
    fn test_build_v1v2c_message_encodes_version_and_community() {
        let msg = build_v1v2c_message(&SnmpVersion::V2c, "public", PDU_GET, 1, "1.3.6.1.2.1.1.1.0");
        let (top, _) = ber_parse_one(&msg).unwrap();
        let fields = ber_parse_all(top.value);
        assert_eq!(ber_read_int(fields[0].value), 1); // v2c -> version 1
        assert_eq!(fields[1].value, b"public");
        assert_eq!(fields[2].tag, PDU_GET);

        let msg_v1 = build_v1v2c_message(&SnmpVersion::V1, "public", PDU_GET, 1, "1.3.6.1.2.1.1.1.0");
        let (top, _) = ber_parse_one(&msg_v1).unwrap();
        let fields = ber_parse_all(top.value);
        assert_eq!(ber_read_int(fields[0].value), 0); // v1 -> version 0
    }

    #[test]
    fn test_derive_v3_keys_no_auth_no_priv_when_passphrases_absent() {
        let cfg = SnmpConfig {
            host: "h".into(), port: 161, version: SnmpVersion::V3,
            community: None, username: Some("admin".into()),
            auth_passphrase: None, auth_protocol: None,
            priv_passphrase: None, priv_protocol: None, timeout_ms: 1000,
        };
        let keys = derive_v3_keys(&cfg, b"engine-id");
        assert!(keys.auth.is_none());
        assert!(keys.privacy.is_none());
    }

    #[test]
    fn test_derive_v3_keys_auth_only_when_no_priv_passphrase() {
        let cfg = SnmpConfig {
            host: "h".into(), port: 161, version: SnmpVersion::V3,
            community: None, username: Some("admin".into()),
            auth_passphrase: Some("authpass123".into()), auth_protocol: Some(SnmpV3AuthProtocol::Sha1),
            priv_passphrase: None, priv_protocol: None, timeout_ms: 1000,
        };
        let keys = derive_v3_keys(&cfg, b"engine-id");
        assert!(keys.auth.is_some());
        assert!(keys.privacy.is_none());
    }

    #[test]
    fn test_derive_v3_keys_authpriv_produces_16_byte_privacy_key() {
        let cfg = SnmpConfig {
            host: "h".into(), port: 161, version: SnmpVersion::V3,
            community: None, username: Some("admin".into()),
            auth_passphrase: Some("authpass123".into()), auth_protocol: Some(SnmpV3AuthProtocol::Md5),
            priv_passphrase: Some("privpass123".into()), priv_protocol: Some(SnmpV3PrivProtocol::Aes128),
            timeout_ms: 1000,
        };
        let keys = derive_v3_keys(&cfg, b"engine-id");
        assert!(keys.auth.is_some());
        assert_eq!(keys.privacy.unwrap().len(), 16);
    }

    #[test]
    fn test_snmp_error_display_and_serialize() {
        let err = SnmpError::NotFound("s1".into());
        assert_eq!(err.to_string(), "Session not found: s1");
        assert_eq!(serde_json::to_string(&err).unwrap(), "\"Session not found: s1\"");

        assert_eq!(SnmpError::Timeout.to_string(), "Timeout — no response from agent");
        assert_eq!(SnmpError::AuthFailed.to_string(), "SNMPv3 authentication failed");
    }

    #[test]
    fn test_snmp_config_and_session_serde_round_trip() {
        let cfg = SnmpConfig {
            host: "10.0.0.1".into(), port: 161, version: SnmpVersion::V2c,
            community: Some("public".into()), username: None,
            auth_passphrase: None, auth_protocol: None,
            priv_passphrase: None, priv_protocol: None, timeout_ms: 2000,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: SnmpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.host, "10.0.0.1");
        assert!(restored.version == SnmpVersion::V2c);

        let session = SnmpSession { id: "sess1".into(), host: "10.0.0.1".into(), version: SnmpVersion::V3 };
        let restored: SnmpSession = serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(restored.version, SnmpVersion::V3);
    }

    #[test]
    fn test_snmp_var_bind_serde_round_trip() {
        let vb = SnmpVarBind { oid: "1.3.6.1.2.1.1.1.0".into(), value_type: "OctetString".into(), value: "Linux".into() };
        let restored: SnmpVarBind = serde_json::from_str(&serde_json::to_string(&vb).unwrap()).unwrap();
        assert_eq!(restored.oid, "1.3.6.1.2.1.1.1.0");
    }

    #[test]
    fn test_snmp_state_new_starts_empty() {
        let state = SnmpState::new();
        assert!(state.sessions.lock().unwrap().is_empty());
    }
}

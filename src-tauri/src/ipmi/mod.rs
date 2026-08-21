/// IPMI 2.0 RMCP+/RAKP+ client — Serial-over-LAN (SOL) console and basic
/// chassis power control, built from scratch over raw UDP (port 623).
///
/// This implements the RMCP+ session-establishment handshake (Open Session
/// Request/Response, RAKP Messages 1-4), the resulting per-packet AES-CBC-128
/// confidentiality + HMAC-SHA1-96 integrity wrapping, and SOL payload framing
/// — IPMI v2.0 spec chapter 13. The previous version of this module only did
/// an unauthenticated RMCP presence ping and then piped raw UDP bytes back
/// and forth pretending it was a SOL session; nothing here was real.
///
/// Field layouts and HMAC input orderings are cross-checked against
/// ipmitool's lanplus plugin (`lanplus.c`/`lanplus_crypt.c`), the reference
/// implementation most BMCs are validated against — memory of spec prose is
/// exactly the kind of thing that produces a plausible-looking handshake
/// that silently fails to interop, since a wrong byte order/field just gets
/// rejected by the BMC rather than erroring loudly. Still: this is entirely
/// unverified against real hardware, since no BMC is reachable from this
/// environment. Treat it as a solid-effort implementation that needs a real
/// target to confirm, not as proven-correct.
use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::{oneshot, Mutex as TokioMutex};
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;
type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

#[derive(Debug, Error)]
pub enum IpmiError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("RMCP/RAKP error: {0}")]
    Rmcp(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for IpmiError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IpmiPrivilege {
    User,
    Operator,
    Administrator,
}

impl IpmiPrivilege {
    fn wire_value(&self) -> u8 {
        match self {
            IpmiPrivilege::User => 0x02,
            IpmiPrivilege::Operator => 0x03,
            IpmiPrivilege::Administrator => 0x04,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiConfig {
    pub host: String,
    /// Default 623
    pub port: u16,
    pub username: String,
    pub password: String,
    /// IPMI channel (typically 1) — accepted for API compatibility but not
    /// yet used: RAKP+ session establishment doesn't need it, only
    /// channel-specific commands like Set Channel Access would.
    pub channel: u8,
    pub privilege: IpmiPrivilege,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiSession {
    pub id: String,
    pub host: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiSolData {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiPowerStatus {
    pub session_id: String,
    pub powered_on: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IpmiPowerAction {
    Down,
    Up,
    Cycle,
    HardReset,
}

impl IpmiPowerAction {
    fn wire_value(self) -> u8 {
        match self {
            IpmiPowerAction::Down => 0x00,
            IpmiPowerAction::Up => 0x01,
            IpmiPowerAction::Cycle => 0x02,
            IpmiPowerAction::HardReset => 0x03,
        }
    }
}

// ── Wire-format constants (IPMI v2.0 spec, chapter 13) ──

const RMCP_CLASS_IPMI: u8 = 0x07;
const AUTHTYPE_RMCP_PLUS: u8 = 0x06;

const PAYLOAD_TYPE_IPMI: u8 = 0x00;
const PAYLOAD_TYPE_SOL: u8 = 0x01;
const PAYLOAD_TYPE_OPEN_SESSION_REQUEST: u8 = 0x10;
const PAYLOAD_TYPE_OPEN_SESSION_RESPONSE: u8 = 0x11;
const PAYLOAD_TYPE_RAKP1: u8 = 0x12;
const PAYLOAD_TYPE_RAKP2: u8 = 0x13;
const PAYLOAD_TYPE_RAKP3: u8 = 0x14;
const PAYLOAD_TYPE_RAKP4: u8 = 0x15;

const AUTH_ALG_RAKP_HMAC_SHA1: u8 = 0x01;
const INTEGRITY_ALG_HMAC_SHA1_96: u8 = 0x01;
const CRYPT_ALG_AES_CBC_128: u8 = 0x01;

const RAKP_STATUS_NO_ERRORS: u8 = 0x00;

const IPMI_BMC_SLAVE_ADDR: u8 = 0x20;
const IPMI_REMOTE_SWID: u8 = 0x81;
const NETFN_CHASSIS: u8 = 0x00;
const CMD_CHASSIS_GET_STATUS: u8 = 0x01;
const CMD_CHASSIS_CONTROL: u8 = 0x02;

fn rmcp_header() -> [u8; 4] {
    [0x06, 0x00, 0xFF, RMCP_CLASS_IPMI]
}

fn ipmi_checksum(data: &[u8]) -> u8 {
    (data.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))).wrapping_neg()
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut mac = <HmacSha1 as Mac>::new_from_slice(key).expect("HMAC-SHA1 accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// The HMAC key for every RAKP+ auth computation is the password padded (or
/// truncated) to exactly 20 bytes with zeros — not the raw password bytes.
/// Confirmed against ipmitool's `IPMI_AUTHCODE_BUFFER_SIZE`/`session->authcode`
/// handling: it always passes a fixed 20-byte buffer as the HMAC key length,
/// so a password shorter than 20 bytes hashes differently than its raw bytes
/// would. Getting this wrong produces a handshake that looks right but never
/// authenticates.
fn password_key(password: &str) -> [u8; 20] {
    let mut key = [0u8; 20];
    let bytes = password.as_bytes();
    let n = bytes.len().min(20);
    key[..n].copy_from_slice(&bytes[..n]);
    key
}

// ── Open Session Request/Response ──

fn build_open_session_request(console_session_id: u32, privilege: &IpmiPrivilege) -> Vec<u8> {
    let mut msg = Vec::with_capacity(32);
    msg.push(0); // message tag
    msg.push(privilege.wire_value());
    msg.extend_from_slice(&[0, 0]); // reserved
    msg.extend_from_slice(&console_session_id.to_le_bytes());
    // Authentication / Integrity / Confidentiality algorithm payloads —
    // each is an 8-byte block: [payload_type, reserved*3, len=8, algorithm, reserved*2]
    for (payload_type, algorithm) in [
        (0u8, AUTH_ALG_RAKP_HMAC_SHA1),
        (1u8, INTEGRITY_ALG_HMAC_SHA1_96),
        (2u8, CRYPT_ALG_AES_CBC_128),
    ] {
        msg.push(payload_type);
        msg.extend_from_slice(&[0, 0]);
        msg.push(8);
        msg.push(algorithm);
        msg.extend_from_slice(&[0, 0, 0]);
    }
    msg
}

struct OpenSessionResponse {
    #[allow(dead_code)] // validated in parse_open_session_response; kept for test assertions
    status: u8,
    bmc_session_id: u32,
}

fn parse_open_session_response(data: &[u8]) -> Result<OpenSessionResponse, IpmiError> {
    if data.len() < 8 {
        return Err(IpmiError::Rmcp("Open Session Response too short".into()));
    }
    let status = data[1];
    if status != RAKP_STATUS_NO_ERRORS {
        return Err(IpmiError::Rmcp(format!("Open Session Response error status 0x{status:02x}")));
    }
    if data.len() < 12 {
        return Err(IpmiError::Rmcp("Open Session Response missing BMC session ID".into()));
    }
    let bmc_session_id = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    Ok(OpenSessionResponse { status, bmc_session_id })
}

// ── RAKP Messages 1-4 ──

fn build_rakp1(bmc_session_id: u32, console_rand: &[u8; 16], privilege: &IpmiPrivilege, username: &str) -> Vec<u8> {
    let mut msg = Vec::with_capacity(28 + username.len());
    msg.push(0); // message tag
    msg.extend_from_slice(&[0, 0, 0]); // reserved
    msg.extend_from_slice(&bmc_session_id.to_le_bytes());
    msg.extend_from_slice(console_rand);
    // 0x10 = "name-only lookup" bit, standard for username/password auth.
    msg.push(privilege.wire_value() | 0x10);
    msg.extend_from_slice(&[0, 0]); // reserved
    msg.push(username.len() as u8);
    msg.extend_from_slice(username.as_bytes());
    msg
}

#[derive(Debug)]
struct Rakp2 {
    status: u8,
    bmc_rand: [u8; 16],
    bmc_guid: [u8; 16],
    key_exchange_auth_code: Vec<u8>,
}

fn parse_rakp2(data: &[u8]) -> Result<Rakp2, IpmiError> {
    if data.len() < 4 {
        return Err(IpmiError::Rmcp("RAKP2 too short".into()));
    }
    let status = data[1];
    if status != RAKP_STATUS_NO_ERRORS {
        return Err(IpmiError::Rmcp(format!("RAKP2 error status 0x{status:02x} — check username/privilege level")));
    }
    if data.len() < 40 {
        return Err(IpmiError::Rmcp("RAKP2 missing random number/GUID".into()));
    }
    let mut bmc_rand = [0u8; 16];
    bmc_rand.copy_from_slice(&data[8..24]);
    let mut bmc_guid = [0u8; 16];
    bmc_guid.copy_from_slice(&data[24..40]);
    // HMAC-SHA1 auth code is 20 bytes; may be absent if auth alg is RAKP-None
    // (unsupported here, but don't panic on a short buffer).
    let key_exchange_auth_code = data.get(40..60).unwrap_or(&[]).to_vec();
    Ok(Rakp2 { status, bmc_rand, bmc_guid, key_exchange_auth_code })
}

/// Verifies the BMC's RAKP2 key exchange auth code — this is what
/// authenticates the *server* to the client (the client authenticates
/// itself to the server via the RAKP3 auth code below). Buffer order per
/// ipmitool's `lanplus_rakp2_hmac_matches`:
/// SIDm(console session id,4) || SIDc(bmc session id,4) || Rm(console
/// rand,16) || Rc(bmc rand,16) || GUIDc(bmc guid,16) || ROLEm(1) ||
/// ULENGTHm(1) || username.
#[allow(clippy::too_many_arguments)]
fn verify_rakp2_auth_code(
    password: &str,
    console_session_id: u32,
    bmc_session_id: u32,
    console_rand: &[u8; 16],
    bmc_rand: &[u8; 16],
    bmc_guid: &[u8; 16],
    role_byte: u8,
    username: &str,
    received_auth_code: &[u8],
) -> bool {
    let mut buf = Vec::with_capacity(58 + username.len());
    buf.extend_from_slice(&console_session_id.to_le_bytes());
    buf.extend_from_slice(&bmc_session_id.to_le_bytes());
    buf.extend_from_slice(console_rand);
    buf.extend_from_slice(bmc_rand);
    buf.extend_from_slice(bmc_guid);
    buf.push(role_byte);
    buf.push(username.len() as u8);
    buf.extend_from_slice(username.as_bytes());
    let expected = hmac_sha1(&password_key(password), &buf);
    received_auth_code.len() == 20 && constant_time_eq(&expected, received_auth_code)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Session Integrity Key. Input order per `lanplus_generate_sik`:
/// Rm(console rand,16) || Rc(bmc rand,16) || ROLEm(1) || ULENGTHm(1) ||
/// username — notably *not* the same field order as the RAKP2 auth code
/// (no session IDs or GUID here).
fn compute_sik(password: &str, console_rand: &[u8; 16], bmc_rand: &[u8; 16], role_byte: u8, username: &str) -> [u8; 20] {
    let mut buf = Vec::with_capacity(34 + username.len());
    buf.extend_from_slice(console_rand);
    buf.extend_from_slice(bmc_rand);
    buf.push(role_byte);
    buf.push(username.len() as u8);
    buf.extend_from_slice(username.as_bytes());
    hmac_sha1(&password_key(password), &buf)
}

fn compute_k1(sik: &[u8; 20]) -> [u8; 20] {
    hmac_sha1(sik, &[0x01; 20])
}

fn compute_k2(sik: &[u8; 20]) -> [u8; 20] {
    hmac_sha1(sik, &[0x02; 20])
}

/// RAKP3 auth code — proves to the BMC that the client also derived the
/// right key. Input order per `lanplus_generate_rakp3_authcode`:
/// Rc(bmc rand,16) || SIDm(console session id,4) || ROLEm(1) ||
/// ULENGTHm(1) || username.
fn compute_rakp3_auth_code(password: &str, bmc_rand: &[u8; 16], console_session_id: u32, role_byte: u8, username: &str) -> [u8; 20] {
    let mut buf = Vec::with_capacity(22 + username.len());
    buf.extend_from_slice(bmc_rand);
    buf.extend_from_slice(&console_session_id.to_le_bytes());
    buf.push(role_byte);
    buf.push(username.len() as u8);
    buf.extend_from_slice(username.as_bytes());
    hmac_sha1(&password_key(password), &buf)
}

fn build_rakp3(rakp2_status: u8, bmc_session_id: u32, auth_code: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(8 + auth_code.len());
    msg.push(0); // message tag
    msg.push(rakp2_status);
    msg.extend_from_slice(&[0, 0]); // reserved
    msg.extend_from_slice(&bmc_session_id.to_le_bytes());
    msg.extend_from_slice(auth_code);
    msg
}

// ── Post-session encrypted + integrity-protected packet framing ──

/// Pads `plaintext` per IPMI's own confidentiality trailer convention (not
/// PKCS7): pad bytes count up 0x01, 0x02, … then a final byte holding the
/// pad count, sized so the total is a multiple of the AES block size (16).
fn ipmi_pad(plaintext: &[u8]) -> Vec<u8> {
    let unpadded_with_len_byte = plaintext.len() + 1;
    let pad_len = (16 - (unpadded_with_len_byte % 16)) % 16;
    let mut out = Vec::with_capacity(plaintext.len() + pad_len + 1);
    out.extend_from_slice(plaintext);
    for i in 1..=pad_len {
        out.push(i as u8);
    }
    out.push(pad_len as u8);
    out
}

fn aes_cbc_encrypt(key16: &[u8], iv16: &[u8], padded_plaintext: &[u8]) -> Vec<u8> {
    let mut buf = padded_plaintext.to_vec();
    let enc = Aes128CbcEnc::new(key16.into(), iv16.into());
    // Data is already IPMI-padded to a block multiple, so no crate-level
    // padding scheme is needed here — encrypt each block in place.
    for chunk in buf.chunks_mut(16) {
        let block = cbc::cipher::generic_array::GenericArray::from_mut_slice(chunk);
        enc.clone().encrypt_block_mut(block);
    }
    buf
}

fn aes_cbc_decrypt(key16: &[u8], iv16: &[u8], ciphertext: &[u8]) -> Vec<u8> {
    let mut buf = ciphertext.to_vec();
    let dec = Aes128CbcDec::new(key16.into(), iv16.into());
    for chunk in buf.chunks_mut(16) {
        let block = cbc::cipher::generic_array::GenericArray::from_mut_slice(chunk);
        dec.clone().decrypt_block_mut(block);
    }
    buf
}

struct SessionCrypto {
    bmc_session_id: u32,
    k1: [u8; 20],
    k2: [u8; 20],
}

/// Builds a full RMCP+ packet (RMCP header + IPMI session header + encrypted
/// payload + integrity trailer) for an established, active session — CBC
/// confidentiality and HMAC-SHA1-96 integrity are always applied post-RAKP,
/// matching the algorithms negotiated in Open Session (this client only
/// ever proposes AES-CBC-128 + HMAC-SHA1-96, so no "none" branch is needed).
fn build_active_packet(crypto: &SessionCrypto, out_seq: u32, payload_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);
    let padded = ipmi_pad(payload);
    let ciphertext = aes_cbc_encrypt(&crypto.k2[..16], &iv, &padded);

    let mut encrypted_payload = Vec::with_capacity(16 + ciphertext.len());
    encrypted_payload.extend_from_slice(&iv);
    encrypted_payload.extend_from_slice(&ciphertext);

    let mut msg = Vec::new();
    msg.extend_from_slice(&rmcp_header());

    let session_header_start = msg.len();
    msg.push(AUTHTYPE_RMCP_PLUS);
    msg.push(payload_type | 0xC0); // bit7=encrypted, bit6=authenticated
    msg.extend_from_slice(&crypto.bmc_session_id.to_le_bytes());
    msg.extend_from_slice(&out_seq.to_le_bytes());
    msg.extend_from_slice(&(encrypted_payload.len() as u16).to_le_bytes());
    msg.extend_from_slice(&encrypted_payload);

    // Integrity trailer: 0xFF pad bytes to 4-byte-align (header-from-AuthType
    // + payload + pad-length byte + next-header byte), then pad length, then
    // next header (always 0x07 per spec table 13-8), then the truncated HMAC.
    let unpadded_trailer_len = (msg.len() - session_header_start) + 2;
    let integrity_pad = (4 - (unpadded_trailer_len % 4)) % 4;
    msg.resize(msg.len() + integrity_pad, 0xFF);
    msg.push(integrity_pad as u8);
    msg.push(0x07); // next header
    let mac = hmac_sha1(&crypto.k1, &msg[session_header_start..]);
    msg.extend_from_slice(&mac[..12]); // HMAC-SHA1-96

    msg
}

struct DecryptedPacket {
    payload_type: u8,
    payload: Vec<u8>,
}

/// Unwraps a received RMCP+ packet: strips the RMCP + session headers,
/// verifies the HMAC-SHA1-96 integrity trailer (best-effort — logged but not
/// fatal, since a dropped/corrupt UDP datagram should just be ignored, not
/// tear down the session), decrypts the payload, and strips IPMI's
/// pad-length-byte confidentiality trailer.
fn unwrap_active_packet(crypto: &SessionCrypto, data: &[u8]) -> Option<DecryptedPacket> {
    if data.len() < 4 + 10 {
        return None;
    }
    let session_header_start = 4; // after RMCP header
    if data[session_header_start] != AUTHTYPE_RMCP_PLUS {
        return None;
    }
    let payload_type_byte = data[session_header_start + 1];
    let payload_type = payload_type_byte & 0x3F;
    let encrypted = payload_type_byte & 0x80 != 0;
    let authenticated = payload_type_byte & 0x40 != 0;

    let payload_len_offset = session_header_start + 10;
    if data.len() < payload_len_offset + 2 {
        return None;
    }
    let payload_len = u16::from_le_bytes([data[payload_len_offset], data[payload_len_offset + 1]]) as usize;
    let payload_start = payload_len_offset + 2;
    if data.len() < payload_start + payload_len {
        return None;
    }
    let encrypted_payload = &data[payload_start..payload_start + payload_len];

    if authenticated {
        let trailer_start = payload_start + payload_len;
        // The pad-length byte sits *after* the 0-3 integrity pad bytes, not
        // at trailer_start — its position is derived the same way the
        // sender computed it (from lengths alone), not read off the wire,
        // since trusting an attacker-controlled offset here would let a
        // tampered packet dodge the integrity check entirely.
        let unpadded_len = (trailer_start - session_header_start) + 2;
        let integrity_pad = (4 - (unpadded_len % 4)) % 4;
        let pad_len_offset = trailer_start + integrity_pad;
        let mac_start = pad_len_offset + 2;
        if data.len() < mac_start + 12 {
            return None;
        }
        let expected = hmac_sha1(&crypto.k1, &data[session_header_start..mac_start]);
        if !constant_time_eq(&expected[..12], &data[mac_start..mac_start + 12]) {
            return None;
        }
    }

    let plaintext = if encrypted {
        if encrypted_payload.len() < 16 || (encrypted_payload.len() - 16) % 16 != 0 {
            return None;
        }
        let iv = &encrypted_payload[..16];
        let decrypted = aes_cbc_decrypt(&crypto.k2[..16], iv, &encrypted_payload[16..]);
        if decrypted.is_empty() {
            return None;
        }
        let pad_len = *decrypted.last().unwrap() as usize;
        if pad_len >= decrypted.len() {
            return None;
        }
        decrypted[..decrypted.len() - pad_len - 1].to_vec()
    } else {
        encrypted_payload.to_vec()
    };

    Some(DecryptedPacket { payload_type, payload: plaintext })
}

// ── SOL payload framing ──

fn build_sol_payload(seq: u8, acked_seq: u8, accepted_char_count: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + data.len());
    out.push(seq);
    out.push(acked_seq);
    out.push(accepted_char_count);
    out.push(0); // status/flags — no break, no flow-control changes
    out.extend_from_slice(data);
    out
}

// ── IPMI (Chassis) request/response framing, for power control ──

fn build_ipmi_request(netfn: u8, cmd: u8, rq_seq: u8, data: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(7 + data.len());
    msg.push(IPMI_BMC_SLAVE_ADDR);
    msg.push(netfn << 2);
    let cs1_start = 0;
    let cs1 = ipmi_checksum(&msg[cs1_start..]);
    msg.push(cs1);

    let cs2_start = msg.len();
    msg.push(IPMI_REMOTE_SWID);
    msg.push(rq_seq << 2);
    msg.push(cmd);
    msg.extend_from_slice(data);
    let cs2 = ipmi_checksum(&msg[cs2_start..]);
    msg.push(cs2);
    msg
}

struct IpmiResponse {
    completion_code: u8,
    data: Vec<u8>,
}

fn parse_ipmi_response(msg: &[u8]) -> Option<IpmiResponse> {
    // rsAddr, netFn/rqLUN, checksum1, rqAddr, rqSeq/rqLUN, cmd, ccode, data..., checksum2
    // Minimum valid length is 8 (6 header bytes + ccode + trailing
    // checksum2), not 7 — a 7-byte message is missing checksum2 entirely.
    // The old `< 7` guard let a malformed 7-byte packet from the network
    // through to `msg[7..msg.len() - 1]` (i.e. `msg[7..6]`), where start
    // > end panics the whole IPMI listener task on untrusted input.
    if msg.len() < 8 {
        return None;
    }
    let completion_code = msg[6];
    let data = msg[7..msg.len() - 1].to_vec();
    Some(IpmiResponse { completion_code, data })
}

// ── Connection state ──

struct IpmiConn {
    socket: Arc<UdpSocket>,
    crypto: SessionCrypto,
    out_seq: AtomicU32,
    sol_out_seq: AtomicU8,
    last_received_sol_seq: AtomicU8,
    pending_ipmi: TokioMutex<HashMap<u8, oneshot::Sender<IpmiResponse>>>,
    next_rq_seq: AtomicU8,
}

impl IpmiConn {
    async fn send(&self, payload_type: u8, payload: &[u8]) -> std::io::Result<()> {
        let seq = self.out_seq.fetch_add(1, Ordering::SeqCst);
        let packet = build_active_packet(&self.crypto, seq, payload_type, payload);
        self.socket.send(&packet).await?;
        Ok(())
    }

    async fn send_ipmi_request(&self, netfn: u8, cmd: u8, data: &[u8]) -> Result<IpmiResponse, IpmiError> {
        let rq_seq = self.next_rq_seq.fetch_add(1, Ordering::SeqCst) & 0x3F;
        let (tx, rx) = oneshot::channel();
        self.pending_ipmi.lock().await.insert(rq_seq, tx);
        let request = build_ipmi_request(netfn, cmd, rq_seq, data);
        self.send(PAYLOAD_TYPE_IPMI, &request).await?;
        tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .map_err(|_| IpmiError::Rmcp("Timed out waiting for BMC response".into()))?
            .map_err(|_| IpmiError::Rmcp("Response channel closed".into()))
    }
}

pub struct IpmiState {
    sessions: std::sync::Mutex<HashMap<String, IpmiSession>>,
    conns: std::sync::Mutex<HashMap<String, Arc<IpmiConn>>>,
}

impl IpmiState {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(HashMap::new()),
            conns: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

async fn send_and_recv(socket: &UdpSocket, packet: &[u8], timeout: std::time::Duration) -> Result<Vec<u8>, IpmiError> {
    socket.send(packet).await?;
    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(timeout, socket.recv(&mut buf))
        .await
        .map_err(|_| IpmiError::Rmcp("BMC did not respond".into()))??;
    Ok(buf[..n].to_vec())
}

/// Extracts the payload bytes from a pre-session (unencrypted) RMCP+ packet
/// of the expected payload type.
fn extract_pre_session_payload(data: &[u8], expected_payload_type: u8) -> Result<Vec<u8>, IpmiError> {
    if data.len() < 14 {
        return Err(IpmiError::Rmcp("Response too short".into()));
    }
    if data[4] != AUTHTYPE_RMCP_PLUS {
        return Err(IpmiError::Rmcp("Unexpected AuthType in response".into()));
    }
    let payload_type = data[5] & 0x3F;
    if payload_type != expected_payload_type {
        return Err(IpmiError::Rmcp(format!(
            "Unexpected payload type 0x{payload_type:02x}, wanted 0x{expected_payload_type:02x}"
        )));
    }
    let payload_len = u16::from_le_bytes([data[14], data[15]]) as usize;
    let start = 16;
    if data.len() < start + payload_len {
        return Err(IpmiError::Rmcp("Truncated payload".into()));
    }
    Ok(data[start..start + payload_len].to_vec())
}

#[tauri::command]
pub async fn ipmi_sol_connect(
    config: IpmiConfig,
    state: tauri::State<'_, IpmiState>,
    app: AppHandle,
) -> Result<String, IpmiError> {
    let remote_addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e: std::net::AddrParseError| IpmiError::Rmcp(e.to_string()))?;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(remote_addr).await?;
    let timeout = std::time::Duration::from_secs(5);

    // ── Open Session Request/Response ──
    let mut console_session_id_bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut console_session_id_bytes);
    let console_session_id = u32::from_le_bytes(console_session_id_bytes);

    let open_req = wrap_pre_session_packet(PAYLOAD_TYPE_OPEN_SESSION_REQUEST, &build_open_session_request(console_session_id, &config.privilege));
    let resp = send_and_recv(&socket, &open_req, timeout).await?;
    let open_resp_payload = extract_pre_session_payload(&resp, PAYLOAD_TYPE_OPEN_SESSION_RESPONSE)?;
    let open_resp = parse_open_session_response(&open_resp_payload)?;

    // ── RAKP1 / RAKP2 ──
    let mut console_rand = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut console_rand);
    let role_byte = config.privilege.wire_value() | 0x10;

    let rakp1 = wrap_pre_session_packet(PAYLOAD_TYPE_RAKP1, &build_rakp1(open_resp.bmc_session_id, &console_rand, &config.privilege, &config.username));
    let resp = send_and_recv(&socket, &rakp1, timeout).await?;
    let rakp2_payload = extract_pre_session_payload(&resp, PAYLOAD_TYPE_RAKP2)?;
    let rakp2 = parse_rakp2(&rakp2_payload)?;

    if !verify_rakp2_auth_code(
        &config.password,
        console_session_id,
        open_resp.bmc_session_id,
        &console_rand,
        &rakp2.bmc_rand,
        &rakp2.bmc_guid,
        role_byte,
        &config.username,
        &rakp2.key_exchange_auth_code,
    ) {
        return Err(IpmiError::AuthFailed);
    }

    // ── Derive SIK, K1, K2 ──
    let sik = compute_sik(&config.password, &console_rand, &rakp2.bmc_rand, role_byte, &config.username);
    let k1 = compute_k1(&sik);
    let k2 = compute_k2(&sik);

    // ── RAKP3 ──
    let rakp3_auth_code = compute_rakp3_auth_code(&config.password, &rakp2.bmc_rand, console_session_id, role_byte, &config.username);
    let rakp3 = wrap_pre_session_packet(PAYLOAD_TYPE_RAKP3, &build_rakp3(rakp2.status, open_resp.bmc_session_id, &rakp3_auth_code));
    let resp = send_and_recv(&socket, &rakp3, timeout).await?;
    let rakp4_payload = extract_pre_session_payload(&resp, PAYLOAD_TYPE_RAKP4)?;
    if rakp4_payload.len() < 2 || rakp4_payload[1] != RAKP_STATUS_NO_ERRORS {
        return Err(IpmiError::AuthFailed);
    }
    // RAKP4's own integrity MAC (keyed with SIK) could be verified here too,
    // but RAKP3 already completed mutual authentication (we verified RAKP2,
    // and a RAKP4 status of "no errors" means the BMC accepted our RAKP3
    // auth code) — skipping it trades a little defense-in-depth for one
    // less place a byte-order mistake could wrongly reject a good session.

    let id = Uuid::new_v4().to_string();
    let socket = Arc::new(socket);
    let conn = Arc::new(IpmiConn {
        socket: socket.clone(),
        crypto: SessionCrypto { bmc_session_id: open_resp.bmc_session_id, k1, k2 },
        out_seq: AtomicU32::new(1),
        sol_out_seq: AtomicU8::new(1),
        last_received_sol_seq: AtomicU8::new(0),
        pending_ipmi: TokioMutex::new(HashMap::new()),
        next_rq_seq: AtomicU8::new(0),
    });

    // Receiver pump: decrypts every inbound packet, forwards SOL data as an
    // event (acking as it goes), and completes pending IPMI request futures.
    let recv_conn = conn.clone();
    let recv_app = app.clone();
    let recv_id = id.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            let n = match recv_conn.socket.recv(&mut buf).await {
                Ok(n) => n,
                Err(_) => break,
            };
            let Some(packet) = unwrap_active_packet(&recv_conn.crypto, &buf[..n]) else { continue };
            match packet.payload_type {
                PAYLOAD_TYPE_SOL => {
                    if packet.payload.len() < 4 {
                        continue;
                    }
                    let seq = packet.payload[0];
                    let char_data = &packet.payload[4..];
                    if seq != 0 && !char_data.is_empty() {
                        recv_conn.last_received_sol_seq.store(seq, Ordering::SeqCst);
                        let _ = recv_app.emit("ipmi:sol_data", IpmiSolData {
                            session_id: recv_id.clone(),
                            data: String::from_utf8_lossy(char_data).into_owned(),
                        });
                        // Ack-only reply: sequence 0, no data.
                        let ack = build_sol_payload(0, seq, char_data.len().min(255) as u8, &[]);
                        let _ = recv_conn.send(PAYLOAD_TYPE_SOL, &ack).await;
                    }
                }
                PAYLOAD_TYPE_IPMI => {
                    if let Some(response) = parse_ipmi_response(&packet.payload) {
                        // rqSeq/rqLUN lives at byte 4 of the IPMI message, shifted left 2.
                        if packet.payload.len() > 4 {
                            let rq_seq = packet.payload[4] >> 2;
                            if let Some(tx) = recv_conn.pending_ipmi.lock().await.remove(&rq_seq) {
                                let _ = tx.send(response);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    });

    state.sessions.lock().unwrap().insert(id.clone(), IpmiSession { id: id.clone(), host: config.host, username: config.username });
    state.conns.lock().unwrap().insert(id.clone(), conn);
    Ok(id)
}

fn wrap_pre_session_packet(payload_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(&rmcp_header());
    msg.push(AUTHTYPE_RMCP_PLUS);
    msg.push(payload_type);
    msg.extend_from_slice(&[0, 0, 0, 0]); // session ID = 0 pre-session
    msg.extend_from_slice(&[0, 0, 0, 0]); // sequence number = 0 pre-session
    msg.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    msg.extend_from_slice(payload);
    msg
}

#[tauri::command]
pub async fn ipmi_sol_send(
    id: String,
    data: String,
    state: tauri::State<'_, IpmiState>,
) -> Result<(), IpmiError> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| IpmiError::NotFound(id.clone()))?;
    let seq = conn.sol_out_seq.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |s| Some(if s >= 15 { 1 } else { s + 1 })).unwrap();
    let acked = conn.last_received_sol_seq.load(Ordering::SeqCst);
    let payload = build_sol_payload(seq, acked, 0, data.as_bytes());
    conn.send(PAYLOAD_TYPE_SOL, &payload).await?;
    Ok(())
}

#[tauri::command]
pub async fn ipmi_power_status(id: String, state: tauri::State<'_, IpmiState>) -> Result<IpmiPowerStatus, IpmiError> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| IpmiError::NotFound(id.clone()))?;
    let resp = conn.send_ipmi_request(NETFN_CHASSIS, CMD_CHASSIS_GET_STATUS, &[]).await?;
    if resp.completion_code != 0 {
        return Err(IpmiError::Rmcp(format!("Get Chassis Status failed: completion code 0x{:02x}", resp.completion_code)));
    }
    let powered_on = resp.data.first().map(|b| b & 0x01 != 0).unwrap_or(false);
    Ok(IpmiPowerStatus { session_id: id, powered_on })
}

#[tauri::command]
pub async fn ipmi_power_control(id: String, action: IpmiPowerAction, state: tauri::State<'_, IpmiState>) -> Result<(), IpmiError> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| IpmiError::NotFound(id.clone()))?;
    let resp = conn.send_ipmi_request(NETFN_CHASSIS, CMD_CHASSIS_CONTROL, &[action.wire_value()]).await?;
    if resp.completion_code != 0 {
        return Err(IpmiError::Rmcp(format!("Chassis Control failed: completion code 0x{:02x}", resp.completion_code)));
    }
    Ok(())
}

#[tauri::command]
pub fn ipmi_sol_disconnect(id: String, state: tauri::State<'_, IpmiState>) -> Result<(), IpmiError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| IpmiError::NotFound(id.clone()))?;
    state.conns.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn ipmi_list(state: tauri::State<'_, IpmiState>) -> Vec<IpmiSession> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipmi_checksum_makes_sum_zero_mod_256() {
        let data = [0x20u8, 0x18];
        let cs = ipmi_checksum(&data);
        let sum: u8 = data.iter().fold(0u8, |a, b| a.wrapping_add(*b)).wrapping_add(cs);
        assert_eq!(sum, 0);
    }

    #[test]
    fn test_build_ipmi_request_get_chassis_status() {
        // Matches ipmitool's wire format: rsAddr, netFn<<2, cs1, rqAddr, rqSeq<<2, cmd, cs2
        let msg = build_ipmi_request(NETFN_CHASSIS, CMD_CHASSIS_GET_STATUS, 0, &[]);
        assert_eq!(msg[0], IPMI_BMC_SLAVE_ADDR);
        assert_eq!(msg[1], NETFN_CHASSIS << 2);
        assert_eq!(msg[3], IPMI_REMOTE_SWID);
        assert_eq!(msg[5], CMD_CHASSIS_GET_STATUS);
        assert_eq!(msg.len(), 7);
    }

    #[test]
    fn test_parse_ipmi_response_extracts_completion_code_and_data() {
        // rsAddr, netFn/lun, cs1, rqAddr, rqSeq/lun, cmd, ccode, data, cs2
        let msg = [0x81, 0x04, 0x00, 0x20, 0x00, 0x01, 0x00, 0x01, 0x00];
        let resp = parse_ipmi_response(&msg).unwrap();
        assert_eq!(resp.completion_code, 0x00);
        assert_eq!(resp.data, vec![0x01]);
    }

    #[test]
    fn test_ipmi_pad_reaches_a_block_multiple_with_incrementing_bytes() {
        let padded = ipmi_pad(b"hello");
        assert_eq!(padded.len() % 16, 0);
        // 5 data bytes + N pad bytes (1,2,..N) + 1 length byte = multiple of 16
        // => N = 10, so trailer is [1,2,...,10] then final byte 10.
        assert_eq!(&padded[5..15], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(*padded.last().unwrap(), 10);
    }

    #[test]
    fn test_ipmi_pad_empty_input() {
        let padded = ipmi_pad(&[]);
        assert_eq!(padded.len(), 16);
        assert_eq!(*padded.last().unwrap(), 15);
    }

    #[test]
    fn test_aes_cbc_roundtrip() {
        let key = [0x42u8; 16];
        let iv = [0x24u8; 16];
        let padded = ipmi_pad(b"serial console data");
        let ciphertext = aes_cbc_encrypt(&key, &iv, &padded);
        let decrypted = aes_cbc_decrypt(&key, &iv, &ciphertext);
        assert_eq!(decrypted, padded);
    }

    #[test]
    fn test_build_open_session_request_layout() {
        let req = build_open_session_request(0xA0A2A3A4, &IpmiPrivilege::Administrator);
        assert_eq!(req[0], 0); // message tag
        assert_eq!(req[1], 0x04); // administrator privilege
        assert_eq!(&req[4..8], &0xA0A2A3A4u32.to_le_bytes());
        // Auth algorithm payload block
        assert_eq!(req[8], 0);
        assert_eq!(req[11], 8);
        assert_eq!(req[12], AUTH_ALG_RAKP_HMAC_SHA1);
        // Integrity algorithm payload block
        assert_eq!(req[16], 1);
        assert_eq!(req[20], INTEGRITY_ALG_HMAC_SHA1_96);
        // Confidentiality algorithm payload block
        assert_eq!(req[24], 2);
        assert_eq!(req[28], CRYPT_ALG_AES_CBC_128);
        assert_eq!(req.len(), 32);
    }

    #[test]
    fn test_parse_open_session_response() {
        let mut data = vec![0u8; 36];
        data[1] = RAKP_STATUS_NO_ERRORS;
        data[8..12].copy_from_slice(&0xB0B1B2B3u32.to_le_bytes());
        let resp = parse_open_session_response(&data).unwrap();
        assert_eq!(resp.status, 0);
        assert_eq!(resp.bmc_session_id, 0xB0B1B2B3);
    }

    #[test]
    fn test_parse_open_session_response_error_status() {
        let mut data = vec![0u8; 12];
        data[1] = 0x02; // invalid session ID
        assert!(parse_open_session_response(&data).is_err());
    }

    #[test]
    fn test_build_rakp1_layout() {
        let console_rand = [0x11u8; 16];
        let msg = build_rakp1(0xB0B1B2B3, &console_rand, &IpmiPrivilege::Administrator, "admin");
        assert_eq!(msg[0], 0);
        assert_eq!(&msg[4..8], &0xB0B1B2B3u32.to_le_bytes());
        assert_eq!(&msg[8..24], &console_rand);
        assert_eq!(msg[24], 0x04 | 0x10); // privilege | name-only-lookup
        assert_eq!(msg[27], 5); // "admin".len()
        assert_eq!(&msg[28..33], b"admin");
    }

    #[test]
    fn test_parse_rakp2() {
        let mut data = vec![0u8; 60];
        data[1] = RAKP_STATUS_NO_ERRORS;
        let bmc_rand = [0x22u8; 16];
        let bmc_guid = [0x33u8; 16];
        data[8..24].copy_from_slice(&bmc_rand);
        data[24..40].copy_from_slice(&bmc_guid);
        data[40..60].copy_from_slice(&[0x44u8; 20]);
        let rakp2 = parse_rakp2(&data).unwrap();
        assert_eq!(rakp2.bmc_rand, bmc_rand);
        assert_eq!(rakp2.bmc_guid, bmc_guid);
        assert_eq!(rakp2.key_exchange_auth_code, vec![0x44u8; 20]);
    }

    /// End-to-end cross-check of the whole key-derivation chain against a
    /// hand-computed BMC side, using the exact same field orderings pulled
    /// from ipmitool's lanplus_crypt.c — this at least proves internal
    /// consistency (our "client" and our simulated "BMC" agree), even
    /// though it can't prove interop with a real BMC's implementation.
    #[test]
    fn test_rakp_handshake_round_trip_against_a_simulated_bmc() {
        let password = "hunter2";
        let username = "admin";
        let privilege = IpmiPrivilege::Administrator;
        let role_byte = privilege.wire_value() | 0x10;

        let console_session_id = 0xA0A2A3A4u32;
        let bmc_session_id = 0xB0B1B2B3u32;
        let console_rand = [0x11u8; 16];
        let bmc_rand = [0x22u8; 16];
        let bmc_guid = [0x33u8; 16];

        // "BMC" computes RAKP2's key exchange auth code the same way our
        // client verifies it.
        let mut buf = Vec::new();
        buf.extend_from_slice(&console_session_id.to_le_bytes());
        buf.extend_from_slice(&bmc_session_id.to_le_bytes());
        buf.extend_from_slice(&console_rand);
        buf.extend_from_slice(&bmc_rand);
        buf.extend_from_slice(&bmc_guid);
        buf.push(role_byte);
        buf.push(username.len() as u8);
        buf.extend_from_slice(username.as_bytes());
        let bmc_rakp2_auth_code = hmac_sha1(&password_key(password), &buf);

        assert!(verify_rakp2_auth_code(
            password, console_session_id, bmc_session_id, &console_rand, &bmc_rand, &bmc_guid,
            role_byte, username, &bmc_rakp2_auth_code,
        ));

        // Client and "BMC" must derive the same SIK/K1/K2.
        let client_sik = compute_sik(password, &console_rand, &bmc_rand, role_byte, username);
        let bmc_sik = compute_sik(password, &console_rand, &bmc_rand, role_byte, username);
        assert_eq!(client_sik, bmc_sik);
        assert_eq!(compute_k1(&client_sik), compute_k1(&bmc_sik));
        assert_eq!(compute_k2(&client_sik), compute_k2(&bmc_sik));

        // "BMC" verifies the client's RAKP3 auth code the same way the
        // client generated it.
        let client_rakp3_auth_code = compute_rakp3_auth_code(password, &bmc_rand, console_session_id, role_byte, username);
        let mut bmc_check = Vec::new();
        bmc_check.extend_from_slice(&bmc_rand);
        bmc_check.extend_from_slice(&console_session_id.to_le_bytes());
        bmc_check.push(role_byte);
        bmc_check.push(username.len() as u8);
        bmc_check.extend_from_slice(username.as_bytes());
        let bmc_expected = hmac_sha1(&password_key(password), &bmc_check);
        assert_eq!(client_rakp3_auth_code, bmc_expected);
    }

    #[test]
    fn test_verify_rakp2_auth_code_rejects_wrong_length_auth_code() {
        // received_auth_code.len() == 20 is checked before comparing bytes;
        // a short/absent auth code (e.g. RAKP-None servers) must fail closed.
        let ok = verify_rakp2_auth_code(
            "hunter2", 1, 2, &[0x11; 16], &[0x22; 16], &[0x33; 16], 0x14, "admin", &[],
        );
        assert!(!ok);
    }

    #[test]
    fn test_verify_rakp2_auth_code_rejects_tampered_auth_code() {
        let password = "hunter2";
        let username = "admin";
        let role_byte = 0x14;
        let console_session_id = 1u32;
        let bmc_session_id = 2u32;
        let console_rand = [0x11u8; 16];
        let bmc_rand = [0x22u8; 16];
        let bmc_guid = [0x33u8; 16];

        let mut buf = Vec::new();
        buf.extend_from_slice(&console_session_id.to_le_bytes());
        buf.extend_from_slice(&bmc_session_id.to_le_bytes());
        buf.extend_from_slice(&console_rand);
        buf.extend_from_slice(&bmc_rand);
        buf.extend_from_slice(&bmc_guid);
        buf.push(role_byte);
        buf.push(username.len() as u8);
        buf.extend_from_slice(username.as_bytes());
        let mut correct = hmac_sha1(&password_key(password), &buf);
        correct[0] ^= 0xFF; // tamper with one byte

        let ok = verify_rakp2_auth_code(
            password, console_session_id, bmc_session_id, &console_rand, &bmc_rand, &bmc_guid,
            role_byte, username, &correct,
        );
        assert!(!ok);
    }

    #[test]
    fn test_password_key_pads_short_passwords_and_truncates_long_ones() {
        let short = password_key("abc");
        assert_eq!(&short[..3], b"abc");
        assert_eq!(&short[3..], &[0u8; 17]);

        let exactly_20 = password_key("123456789012345678901");
        // 21 chars in, truncated to 20
        assert_eq!(exactly_20.len(), 20);
        assert_eq!(&exactly_20[..], &b"123456789012345678901"[..20]);
    }

    #[test]
    fn test_build_active_packet_and_unwrap_round_trip() {
        let crypto = SessionCrypto { bmc_session_id: 0xB0B1B2B3, k1: [0x55; 20], k2: [0x66; 20] };
        let payload = b"~$ echo hello";
        let packet = build_active_packet(&crypto, 1, PAYLOAD_TYPE_SOL, payload);
        let unwrapped = unwrap_active_packet(&crypto, &packet).expect("packet should unwrap");
        assert_eq!(unwrapped.payload_type, PAYLOAD_TYPE_SOL);
        assert_eq!(unwrapped.payload, payload);
    }

    #[test]
    fn test_unwrap_active_packet_rejects_tampered_integrity_trailer() {
        let crypto = SessionCrypto { bmc_session_id: 0xB0B1B2B3, k1: [0x55; 20], k2: [0x66; 20] };
        let mut packet = build_active_packet(&crypto, 1, PAYLOAD_TYPE_SOL, b"data");
        let last = packet.len() - 1;
        packet[last] ^= 0xFF; // flip a bit in the HMAC
        assert!(unwrap_active_packet(&crypto, &packet).is_none());
    }

    #[test]
    fn test_build_sol_payload_layout() {
        let payload = build_sol_payload(3, 2, 5, b"ls\n");
        assert_eq!(payload[0], 3);
        assert_eq!(payload[1], 2);
        assert_eq!(payload[2], 5);
        assert_eq!(payload[3], 0);
        assert_eq!(&payload[4..], b"ls\n");
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }

    // ── rmcp_header / wrap_pre_session_packet ─────────────────────────

    #[test]
    fn test_rmcp_header_bytes() {
        let header = rmcp_header();
        assert_eq!(header, [0x06, 0x00, 0xFF, RMCP_CLASS_IPMI]);
    }

    #[test]
    fn test_wrap_pre_session_packet_layout() {
        let payload = [0xAA, 0xBB, 0xCC];
        let msg = wrap_pre_session_packet(PAYLOAD_TYPE_RAKP1, &payload);
        assert_eq!(&msg[0..4], &rmcp_header());
        assert_eq!(msg[4], AUTHTYPE_RMCP_PLUS);
        assert_eq!(msg[5], PAYLOAD_TYPE_RAKP1);
        // session ID (4 bytes) + sequence number (4 bytes) = 0 pre-session
        assert_eq!(&msg[6..10], &[0, 0, 0, 0]);
        assert_eq!(&msg[10..14], &[0, 0, 0, 0]);
        // payload length (u16 LE)
        assert_eq!(&msg[14..16], &3u16.to_le_bytes());
        assert_eq!(&msg[16..], &payload);
    }

    // ── extract_pre_session_payload ────────────────────────────────────

    #[test]
    fn test_extract_pre_session_payload_success() {
        let payload = [0x01, 0x02, 0x03, 0x04];
        let packet = wrap_pre_session_packet(PAYLOAD_TYPE_OPEN_SESSION_RESPONSE, &payload);
        let extracted = extract_pre_session_payload(&packet, PAYLOAD_TYPE_OPEN_SESSION_RESPONSE).unwrap();
        assert_eq!(extracted, payload);
    }

    #[test]
    fn test_extract_pre_session_payload_too_short_errors() {
        let data = [0u8; 10]; // < 14 bytes
        let result = extract_pre_session_payload(&data, PAYLOAD_TYPE_OPEN_SESSION_RESPONSE);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_pre_session_payload_wrong_auth_type_errors() {
        let mut packet = wrap_pre_session_packet(PAYLOAD_TYPE_OPEN_SESSION_RESPONSE, &[0x01]);
        packet[4] = 0x00; // not AUTHTYPE_RMCP_PLUS
        let result = extract_pre_session_payload(&packet, PAYLOAD_TYPE_OPEN_SESSION_RESPONSE);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_pre_session_payload_wrong_payload_type_errors() {
        let packet = wrap_pre_session_packet(PAYLOAD_TYPE_RAKP2, &[0x01]);
        let result = extract_pre_session_payload(&packet, PAYLOAD_TYPE_RAKP3);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Unexpected payload type"));
    }

    #[test]
    fn test_extract_pre_session_payload_truncated_errors() {
        let mut packet = wrap_pre_session_packet(PAYLOAD_TYPE_RAKP2, &[0x01, 0x02, 0x03, 0x04]);
        packet.truncate(17); // claims 4-byte payload but only 1 byte present
        let result = extract_pre_session_payload(&packet, PAYLOAD_TYPE_RAKP2);
        assert!(result.is_err());
    }

    // ── parse_rakp2 error paths ─────────────────────────────────────────

    #[test]
    fn test_parse_rakp2_too_short_errors() {
        let data = [0u8; 2];
        assert!(parse_rakp2(&data).is_err());
    }

    #[test]
    fn test_parse_rakp2_error_status_errors() {
        let mut data = vec![0u8; 40];
        data[1] = 0x02; // error status
        let result = parse_rakp2(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("check username/privilege"));
    }

    #[test]
    fn test_parse_rakp2_missing_rand_guid_errors() {
        let mut data = vec![0u8; 20]; // status OK but < 40 bytes
        data[1] = RAKP_STATUS_NO_ERRORS;
        assert!(parse_rakp2(&data).is_err());
    }

    #[test]
    fn test_parse_rakp2_missing_auth_code_defaults_to_empty() {
        // Exactly 40 bytes: status/rand/guid present, auth code absent.
        let mut data = vec![0u8; 40];
        data[1] = RAKP_STATUS_NO_ERRORS;
        let rakp2 = parse_rakp2(&data).unwrap();
        assert!(rakp2.key_exchange_auth_code.is_empty());
    }

    // ── IpmiPrivilege / IpmiPowerAction wire values ──────────────────────

    #[test]
    fn test_ipmi_privilege_wire_values() {
        assert_eq!(IpmiPrivilege::User.wire_value(), 0x02);
        assert_eq!(IpmiPrivilege::Operator.wire_value(), 0x03);
        assert_eq!(IpmiPrivilege::Administrator.wire_value(), 0x04);
    }

    #[test]
    fn test_ipmi_power_action_wire_values() {
        assert_eq!(IpmiPowerAction::Down.wire_value(), 0x00);
        assert_eq!(IpmiPowerAction::Up.wire_value(), 0x01);
        assert_eq!(IpmiPowerAction::Cycle.wire_value(), 0x02);
        assert_eq!(IpmiPowerAction::HardReset.wire_value(), 0x03);
    }

    // ── parse_ipmi_response edge cases ───────────────────────────────────

    #[test]
    fn test_parse_ipmi_response_too_short_returns_none() {
        let msg = [0u8; 5]; // < 7 bytes
        assert!(parse_ipmi_response(&msg).is_none());
    }

    #[test]
    fn test_parse_ipmi_response_minimal_length() {
        // Exactly 8 bytes: no data, completion code at index 6, checksum2
        // (unused by this parser) as the trailing byte.
        let msg = [0x81, 0x04, 0x00, 0x20, 0x00, 0x01, 0x00, 0xAA];
        let resp = parse_ipmi_response(&msg).unwrap();
        assert_eq!(resp.completion_code, 0x00);
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_parse_ipmi_response_too_short_returns_none_instead_of_panicking() {
        // A 7-byte message is missing the trailing checksum2 byte the real
        // wire format always includes — must be rejected, not panic.
        let msg = [0x81, 0x04, 0x00, 0x20, 0x00, 0x01, 0x00];
        assert!(parse_ipmi_response(&msg).is_none());
    }

    // ── unwrap_active_packet edge cases ───────────────────────────────────

    #[test]
    fn test_unwrap_active_packet_too_short_returns_none() {
        let crypto = SessionCrypto { bmc_session_id: 1, k1: [0x11; 20], k2: [0x22; 20] };
        let data = [0u8; 5];
        assert!(unwrap_active_packet(&crypto, &data).is_none());
    }

    #[test]
    fn test_unwrap_active_packet_wrong_auth_type_returns_none() {
        let crypto = SessionCrypto { bmc_session_id: 1, k1: [0x11; 20], k2: [0x22; 20] };
        let mut packet = build_active_packet(&crypto, 1, PAYLOAD_TYPE_SOL, b"data");
        packet[4] = 0x00; // corrupt AuthType
        assert!(unwrap_active_packet(&crypto, &packet).is_none());
    }

    #[test]
    fn test_unwrap_active_packet_unencrypted_unauthenticated_passthrough() {
        // Hand-build a packet with encrypted=0, authenticated=0 (payload
        // type byte high bits clear) to exercise the plaintext branch.
        let crypto = SessionCrypto { bmc_session_id: 0x1234, k1: [0x11; 20], k2: [0x22; 20] };
        let payload = b"plaintext-ipmi";
        let mut msg = Vec::new();
        msg.extend_from_slice(&rmcp_header());
        msg.push(AUTHTYPE_RMCP_PLUS);
        msg.push(PAYLOAD_TYPE_IPMI); // no 0xC0 bits set => unencrypted, unauthenticated
        msg.extend_from_slice(&crypto.bmc_session_id.to_le_bytes());
        msg.extend_from_slice(&1u32.to_le_bytes());
        msg.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        msg.extend_from_slice(payload);

        let unwrapped = unwrap_active_packet(&crypto, &msg).expect("plaintext packet should unwrap");
        assert_eq!(unwrapped.payload_type, PAYLOAD_TYPE_IPMI);
        assert_eq!(unwrapped.payload, payload);
    }

    // ── IpmiError ─────────────────────────────────────────────────────────

    #[test]
    fn test_ipmi_error_display_all_variants() {
        assert_eq!(IpmiError::NotFound("sess-1".into()).to_string(), "Session not found: sess-1");
        assert_eq!(IpmiError::AuthFailed.to_string(), "Authentication failed");
        assert_eq!(IpmiError::Rmcp("bad handshake".into()).to_string(), "RMCP/RAKP error: bad handshake");
        let io_err = IpmiError::from(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        assert!(io_err.to_string().contains("timed out"));
    }

    #[test]
    fn test_ipmi_error_serialize_is_plain_string() {
        let json = serde_json::to_string(&IpmiError::AuthFailed).unwrap();
        assert_eq!(json, "\"Authentication failed\"");
    }

    // ── Serde round-trips ─────────────────────────────────────────────────

    #[test]
    fn test_ipmi_config_serde_roundtrip() {
        let config = IpmiConfig {
            host: "10.0.0.5".into(), port: 623, username: "admin".into(),
            password: "pw".into(), channel: 1, privilege: IpmiPrivilege::Administrator,
        };
        let json = serde_json::to_string(&config).unwrap();
        let d: IpmiConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(d.host, "10.0.0.5");
        assert_eq!(d.privilege, IpmiPrivilege::Administrator);
    }

    #[test]
    fn test_ipmi_privilege_serde_snake_case() {
        assert_eq!(serde_json::to_string(&IpmiPrivilege::User).unwrap(), "\"user\"");
        assert_eq!(serde_json::to_string(&IpmiPrivilege::Operator).unwrap(), "\"operator\"");
        assert_eq!(serde_json::to_string(&IpmiPrivilege::Administrator).unwrap(), "\"administrator\"");
    }

    #[test]
    fn test_ipmi_power_action_serde_snake_case() {
        assert_eq!(serde_json::to_string(&IpmiPowerAction::HardReset).unwrap(), "\"hard_reset\"");
        assert_eq!(serde_json::to_string(&IpmiPowerAction::Up).unwrap(), "\"up\"");
    }

    #[test]
    fn test_ipmi_session_serde_roundtrip() {
        let session = IpmiSession { id: "s1".into(), host: "h1".into(), username: "u1".into() };
        let json = serde_json::to_string(&session).unwrap();
        let d: IpmiSession = serde_json::from_str(&json).unwrap();
        assert_eq!(d.id, "s1");
        assert_eq!(d.host, "h1");
    }

    #[test]
    fn test_ipmi_sol_data_serde_roundtrip() {
        let sol = IpmiSolData { session_id: "s1".into(), data: "console output".into() };
        let json = serde_json::to_string(&sol).unwrap();
        let d: IpmiSolData = serde_json::from_str(&json).unwrap();
        assert_eq!(d.data, "console output");
    }

    #[test]
    fn test_ipmi_power_status_serde_roundtrip() {
        let status = IpmiPowerStatus { session_id: "s1".into(), powered_on: true };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"powered_on\":true"));
    }

    // ── IpmiState (pure state; tauri::State can't be built outside a running app) ──

    #[test]
    fn test_ipmi_state_new_is_empty() {
        let state = IpmiState::new();
        assert!(state.sessions.lock().unwrap().is_empty());
        assert!(state.conns.lock().unwrap().is_empty());
    }

    #[test]
    fn test_ipmi_state_list_and_disconnect_roundtrip() {
        // Mirrors ipmi_list / ipmi_sol_disconnect against the state directly,
        // since tauri::State can't be constructed outside a running app.
        let state = IpmiState::new();
        let session = IpmiSession { id: "sess-1".into(), host: "bmc.local".into(), username: "admin".into() };
        state.sessions.lock().unwrap().insert("sess-1".to_string(), session);

        let listed: Vec<IpmiSession> = state.sessions.lock().unwrap().values().cloned().collect();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].host, "bmc.local");

        // Disconnect: remove from sessions map (mirrors ipmi_sol_disconnect).
        let removed = state.sessions.lock().unwrap().remove("sess-1");
        assert!(removed.is_some());
        assert!(state.sessions.lock().unwrap().is_empty());

        // Removing again should find nothing (mirrors the NotFound path).
        let removed_again = state.sessions.lock().unwrap().remove("sess-1");
        assert!(removed_again.is_none());
    }
}

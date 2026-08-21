/// IBM 3270 mainframe terminal over TCP with TN3270E option negotiation.
///
/// The previous version of this module only handled Telnet option
/// negotiation and then piped raw bytes to the frontend as opaque base64
/// blobs — no 3270 order-stream parsing, no screen buffer, and (a real
/// transport bug) no IAC byte-stuffing, which TN3270's "binary" mode still
/// requires for the literal byte value 0xFF (it must be doubled to
/// `IAC IAC` on the wire, or a legitimate 0xFF in the data stream would be
/// misread as a Telnet command).
///
/// This adds a real 3270 order-stream parser (WCC + SF/SFE/SBA/IC/RA/EUA/PT
/// orders) driving an addressable screen buffer with field attributes
/// (protected/numeric/displayable/intensified/MDT), EBCDIC-to-Unicode
/// rendering, and an AID-key response builder that implements
/// "Read Modified" semantics (only MDT-set fields are sent back, which is
/// what the host actually expects for Enter/PF-key presses).
///
/// Order codes, the WCC bit layout, and the 12-bit buffer-address encoding
/// (the `bit6` translation table) are cross-checked against IBM's own
/// `tnz` library (github.com/IBM/tnz) rather than recalled from memory.
/// Unverified against a real mainframe — none reachable from this
/// environment.
use crate::ebcdic::{ascii_to_ebcdic, ebcdic_to_ascii};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Tn3270Error {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Address {0} is protected")]
    FieldProtected(usize),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for Tn3270Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tn3270Model {
    /// 24×80 monochrome
    Model2,
    /// 32×80 extended
    Model3,
    /// 43×80 extended
    Model4,
    /// 27×132 extended
    Model5,
}

impl Tn3270Model {
    fn dimensions(&self) -> (usize, usize) {
        match self {
            Tn3270Model::Model2 => (24, 80),
            Tn3270Model::Model3 => (32, 80),
            Tn3270Model::Model4 => (43, 80),
            Tn3270Model::Model5 => (27, 132),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270Config {
    pub host: String,
    pub port: u16,
    pub model: Tn3270Model,
    /// Optional LU name for specific LU addressing
    pub lu_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270SessionInfo {
    pub id: String,
    pub host: String,
    pub model: Tn3270Model,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270CellInfo {
    pub ch: char,
    pub protected: bool,
    pub numeric: bool,
    pub displayable: bool,
    pub intensified: bool,
    pub field_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn3270Screen {
    pub session_id: String,
    pub rows: usize,
    pub cols: usize,
    pub cursor_addr: usize,
    pub cells: Vec<Tn3270CellInfo>,
}

// ── AID keys (Attention Identifier) ───────────────────────────────────────
// Values per the 3270 Data Stream Programmer's Reference — a small, stable
// enumeration.

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tn3270Aid {
    Enter,
    Pf1,
    Pf2,
    Pf3,
    Pf4,
    Pf5,
    Pf6,
    Pf7,
    Pf8,
    Pf9,
    Pf10,
    Pf11,
    Pf12,
    Pa1,
    Pa2,
    Pa3,
    Clear,
}

impl Tn3270Aid {
    fn wire_value(self) -> u8 {
        match self {
            Tn3270Aid::Enter => 0x7D,
            Tn3270Aid::Pf1 => 0xF1,
            Tn3270Aid::Pf2 => 0xF2,
            Tn3270Aid::Pf3 => 0xF3,
            Tn3270Aid::Pf4 => 0xF4,
            Tn3270Aid::Pf5 => 0xF5,
            Tn3270Aid::Pf6 => 0xF6,
            Tn3270Aid::Pf7 => 0xF7,
            Tn3270Aid::Pf8 => 0xF8,
            Tn3270Aid::Pf9 => 0xF9,
            Tn3270Aid::Pf10 => 0x7A,
            Tn3270Aid::Pf11 => 0x7B,
            Tn3270Aid::Pf12 => 0x7C,
            Tn3270Aid::Pa1 => 0x6C,
            Tn3270Aid::Pa2 => 0x6E,
            Tn3270Aid::Pa3 => 0x6B,
            Tn3270Aid::Clear => 0x6D,
        }
    }
}

// ── Order codes (verified against IBM tnz's `_process_order_0xNN` methods) ─
const ORDER_PT: u8 = 0x05;
const ORDER_GE: u8 = 0x08;
const ORDER_SBA: u8 = 0x11;
const ORDER_EUA: u8 = 0x12;
const ORDER_IC: u8 = 0x13;
const ORDER_SF: u8 = 0x1D;
const ORDER_SA: u8 = 0x28;
const ORDER_SFE: u8 = 0x29;
const ORDER_MF: u8 = 0x2C;
const ORDER_RA: u8 = 0x3C;

const CMD_W: u8 = 0xF1;
const CMD_RB: u8 = 0xF2;
const CMD_WSF: u8 = 0xF3;
const CMD_EW: u8 = 0xF5;
const CMD_RM: u8 = 0xF6;
const CMD_EAU: u8 = 0x6F;
const CMD_EWA: u8 = 0x7E;

#[derive(Debug, Clone, Default)]
struct Cell {
    ebcdic: u8,
    /// True on the field-attribute position itself (which occupies a cell
    /// but displays as a blank on screen).
    field_start: bool,
    protected: bool,
    numeric: bool,
    displayable: bool,
    intensified: bool,
    mdt: bool,
}

pub struct ScreenBuffer {
    rows: usize,
    cols: usize,
    cells: Vec<Cell>,
    cursor_addr: usize,
    buf_addr: usize,
}

impl ScreenBuffer {
    fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, cells: vec![Cell::default(); rows * cols], cursor_addr: 0, buf_addr: 0 }
    }

    fn size(&self) -> usize {
        self.rows * self.cols
    }

    fn erase(&mut self) {
        self.cells = vec![Cell::default(); self.size()];
        self.buf_addr = 0;
        self.cursor_addr = 0;
    }

    /// The field attribute governing `addr` — the most recent field-start
    /// cell at or before `addr`, scanning backward with wraparound (a
    /// standalone unformatted screen with no fields is unprotected).
    fn field_attr_for(&self, addr: usize) -> Option<usize> {
        let n = self.size();
        for step in 0..n {
            let a = (addr + n - step) % n;
            if self.cells[a].field_start {
                return Some(a);
            }
        }
        None
    }

    fn is_protected(&self, addr: usize) -> bool {
        self.field_attr_for(addr).map(|fa| self.cells[fa].protected).unwrap_or(false)
    }
}

/// Decodes a 12-bit buffer address from two wire bytes — valid for all
/// standard-size 3270 screens (models 2-5 all have buffer_size <= 4095,
/// so the 14/16-bit addressing mode used only for larger screens doesn't
/// apply here).
fn decode_address(hi: u8, lo: u8) -> usize {
    ((hi & 0x3F) as usize) * 64 + (lo & 0x3F) as usize
}

/// The inverse of `decode_address` — each 6-bit half goes through the same
/// "6-bit to printable byte" translation IBM's tnz calls `bit6`, not a
/// plain `| 0x40`. Table verified against `tnz.py`'s `bit6()`.
fn bit6(v: u8) -> u8 {
    let v = v & 0x3F;
    if v == 48 {
        return v | 0xC0;
    }
    if v == 33 {
        return v | 0x40;
    }
    if (1..10).contains(&(v & 0x0F)) {
        return v | 0xC0;
    }
    v | 0x40
}

fn encode_address(addr: usize) -> [u8; 2] {
    let hi = (addr / 64) as u8;
    let lo = (addr % 64) as u8;
    [bit6(hi), bit6(lo)]
}

/// Parses one field-attribute byte into its component flags. Bit layout
/// (protected=0x20, numeric=0x10, MDT=0x01, display/intensity in bits
/// 0x0C) verified against `tnz.py`'s `is_protected`/`is_displayable_attr`
/// bit checks.
fn parse_field_attr(fattr: u8) -> (bool, bool, bool, bool, bool) {
    let protected = fattr & 0x20 != 0;
    let numeric = fattr & 0x10 != 0;
    let intensity_bits = fattr & 0x0C;
    let displayable = intensity_bits != 0x0C;
    let intensified = intensity_bits == 0x08;
    let mdt = fattr & 0x01 != 0;
    (protected, numeric, displayable, intensified, mdt)
}

/// Parses a 3270 data-stream command (as delivered in one TN3270 record,
/// i.e. between two `IAC EOR` markers) and applies it to the screen
/// buffer. Orders this doesn't fully act on (PT, GE, SA, MF) still consume
/// the correct number of bytes so the rest of the stream doesn't desync.
fn process_command(screen: &mut ScreenBuffer, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let cmd = data[0];
    let mut i = match cmd {
        CMD_EW | CMD_EWA | CMD_EAU => {
            screen.erase();
            if data.len() < 2 {
                return;
            }
            2 // command + WCC
        }
        CMD_W => {
            if data.len() < 2 {
                return;
            }
            2 // command + WCC
        }
        CMD_RB | CMD_RM => return, // read commands carry no orders to apply
        CMD_WSF => return,         // structured fields (e.g. Query) — not modeled
        _ => 1,                    // unrecognized command byte; try to parse orders anyway
    };

    while i < data.len() {
        let order = data[i];
        match order {
            ORDER_SBA if i + 2 < data.len() => {
                screen.buf_addr = decode_address(data[i + 1], data[i + 2]) % screen.size().max(1);
                i += 3;
            }
            ORDER_SF if i + 1 < data.len() => {
                let (protected, numeric, displayable, intensified, mdt) = parse_field_attr(data[i + 1]);
                let addr = screen.buf_addr % screen.size().max(1);
                screen.cells[addr] = Cell { ebcdic: 0x40, field_start: true, protected, numeric, displayable, intensified, mdt };
                screen.buf_addr = (addr + 1) % screen.size().max(1);
                i += 2;
            }
            ORDER_SFE if i + 1 < data.len() => {
                let pair_count = data[i + 1] as usize;
                let mut base_fattr = 0u8;
                let mut j = i + 2;
                for _ in 0..pair_count {
                    if j + 1 >= data.len() {
                        break;
                    }
                    if data[j] == 0xC0 {
                        base_fattr = data[j + 1]; // basic 3270 field attribute type
                    }
                    j += 2;
                }
                let (protected, numeric, displayable, intensified, mdt) = parse_field_attr(base_fattr);
                let addr = screen.buf_addr % screen.size().max(1);
                screen.cells[addr] = Cell { ebcdic: 0x40, field_start: true, protected, numeric, displayable, intensified, mdt };
                screen.buf_addr = (addr + 1) % screen.size().max(1);
                i = j;
            }
            ORDER_IC => {
                screen.cursor_addr = screen.buf_addr;
                i += 1;
            }
            ORDER_RA if i + 3 < data.len() => {
                let stop_addr = decode_address(data[i + 1], data[i + 2]);
                let fill = data[i + 3];
                let size = screen.size().max(1);
                let mut a = screen.buf_addr % size;
                loop {
                    screen.cells[a].ebcdic = fill;
                    screen.cells[a].field_start = false;
                    a = (a + 1) % size;
                    if a == stop_addr % size {
                        break;
                    }
                }
                screen.buf_addr = stop_addr % size;
                i += 4;
            }
            ORDER_EUA if i + 2 < data.len() => {
                let stop_addr = decode_address(data[i + 1], data[i + 2]);
                let size = screen.size().max(1);
                let mut a = screen.buf_addr % size;
                while a != stop_addr % size {
                    if !screen.is_protected(a) {
                        screen.cells[a].ebcdic = 0x40;
                    }
                    a = (a + 1) % size;
                }
                screen.buf_addr = stop_addr % size;
                i += 3;
            }
            ORDER_PT => i += 1,
            ORDER_GE if i + 1 < data.len() => i += 2,
            ORDER_SA if i + 2 < data.len() => i += 3,
            ORDER_MF if i + 1 < data.len() => {
                let pair_count = data[i + 1] as usize;
                i += 2 + pair_count * 2;
            }
            _ => {
                // Ordinary character data — one EBCDIC byte per buffer cell.
                let size = screen.size().max(1);
                let addr = screen.buf_addr % size;
                screen.cells[addr].ebcdic = order;
                screen.cells[addr].field_start = false;
                screen.buf_addr = (addr + 1) % size;
                i += 1;
            }
        }
    }
}

/// Builds the AID-key response: AID byte + encoded cursor address, followed
/// by `SBA(field_start) + field_data` for every field whose MDT bit is set
/// (the "Read Modified" semantics real hosts expect on Enter/PF-key —
/// unmodified fields are omitted entirely, not sent as blanks).
fn build_read_modified_response(screen: &ScreenBuffer, aid: Tn3270Aid) -> Vec<u8> {
    let mut out = vec![aid.wire_value()];
    out.extend_from_slice(&encode_address(screen.cursor_addr));

    let size = screen.size();
    let mut i = 0;
    while i < size {
        if screen.cells[i].field_start && screen.cells[i].mdt {
            let data_start = (i + 1) % size;
            out.extend_from_slice(&[ORDER_SBA]);
            out.extend_from_slice(&encode_address(data_start));
            let mut a = data_start;
            while a != i && !screen.cells[a].field_start {
                out.push(screen.cells[a].ebcdic);
                a = (a + 1) % size;
            }
        }
        i += 1;
    }
    out
}

fn screen_snapshot(session_id: &str, screen: &ScreenBuffer) -> Tn3270Screen {
    Tn3270Screen {
        session_id: session_id.to_string(),
        rows: screen.rows,
        cols: screen.cols,
        cursor_addr: screen.cursor_addr,
        cells: screen
            .cells
            .iter()
            .map(|c| Tn3270CellInfo {
                ch: ebcdic_to_ascii(c.ebcdic) as char,
                protected: c.protected,
                numeric: c.numeric,
                displayable: c.displayable,
                intensified: c.intensified,
                field_start: c.field_start,
            })
            .collect(),
    }
}

// ── Telnet transport (fixed: real IAC byte-stuffing + EOR record framing) ─

const IAC: u8 = 0xFF;
const DO: u8 = 0xFD;
const WILL: u8 = 0xFB;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;
const OPT_BINARY: u8 = 0x00;
const OPT_EOR: u8 = 0x19;
const OPT_TTYPE: u8 = 0x18;
const EOR: u8 = 0xEF;

fn model_terminal_type(model: &Tn3270Model) -> &'static str {
    match model {
        Tn3270Model::Model2 => "IBM-3278-2-E",
        Tn3270Model::Model3 => "IBM-3278-3-E",
        Tn3270Model::Model4 => "IBM-3278-4-E",
        Tn3270Model::Model5 => "IBM-3278-5-E",
    }
}

/// Doubles literal 0xFF bytes (`IAC IAC`) — required by Telnet binary mode
/// so a genuine 0xFF data byte isn't misread as the start of a Telnet
/// command. The previous version of this module sent the 3270 stream raw,
/// which silently corrupts any record containing a 0xFF byte.
fn iac_escape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        out.push(b);
        if b == IAC {
            out.push(IAC);
        }
    }
    out
}

/// Reads from `reader` into `pending`, unstuffing `IAC IAC` -> `IAC`, and
/// returns the bytes of one complete record whenever `IAC EOR` is seen.
struct RecordReader {
    pending: Vec<u8>,
}

impl RecordReader {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

    /// Feeds raw bytes off the wire; returns any complete records found.
    fn feed(&mut self, raw: &[u8]) -> Vec<Vec<u8>> {
        let mut records = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let b = raw[i];
            if b == IAC && i + 1 < raw.len() {
                match raw[i + 1] {
                    IAC => {
                        self.pending.push(IAC);
                        i += 2;
                        continue;
                    }
                    EOR => {
                        records.push(std::mem::take(&mut self.pending));
                        i += 2;
                        continue;
                    }
                    _ => {
                        // Telnet negotiation bytes interleaved in the
                        // stream — not part of the 3270 record; skip past
                        // the 3-byte DO/WILL/etc form when present.
                        if i + 2 < raw.len() {
                            i += 3;
                        } else {
                            i += 2;
                        }
                        continue;
                    }
                }
            }
            self.pending.push(b);
            i += 1;
        }
        records
    }
}

pub struct Tn3270Conn {
    info: Tn3270SessionInfo,
    screen: Mutex<ScreenBuffer>,
}

pub struct Tn3270State {
    sessions: Mutex<HashMap<String, Tn3270SessionInfo>>,
    conns: Mutex<HashMap<String, std::sync::Arc<Tn3270Conn>>>,
    writers: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl Tn3270State {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()), conns: Mutex::new(HashMap::new()), writers: Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn tn3270_connect(config: Tn3270Config, state: tauri::State<'_, Tn3270State>, app: AppHandle) -> Result<String, Tn3270Error> {
    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr).await?;
    let id = Uuid::new_v4().to_string();

    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    let terminal_type = model_terminal_type(&config.model).to_string();
    let lu_name = config.lu_name.clone();

    tokio::spawn(async move {
        let _ = writer.write_all(&[IAC, DO, OPT_BINARY, IAC, DO, OPT_EOR, IAC, WILL, OPT_TTYPE]).await;

        let ttype_bytes: Vec<u8> = {
            let mut v = vec![IAC, SB, OPT_TTYPE, 0x00]; // IS
            v.extend_from_slice(terminal_type.as_bytes());
            if let Some(lu) = &lu_name {
                v.push(b'@');
                v.extend_from_slice(lu.as_bytes());
            }
            v.extend_from_slice(&[IAC, SE]);
            v
        };
        let _ = writer.write_all(&ttype_bytes).await;

        while let Some(data) = rx.recv().await {
            let escaped = iac_escape(&data);
            if writer.write_all(&escaped).await.is_err() {
                break;
            }
            let _ = writer.write_all(&[IAC, EOR]).await;
        }
    });

    let (rows, cols) = config.model.dimensions();
    let conn = std::sync::Arc::new(Tn3270Conn {
        info: Tn3270SessionInfo { id: id.clone(), host: config.host.clone(), model: config.model.clone() },
        screen: Mutex::new(ScreenBuffer::new(rows, cols)),
    });

    let app_clone = app.clone();
    let id_clone = id.clone();
    let conn_clone = conn.clone();
    tokio::spawn(async move {
        let mut record_reader = RecordReader::new();
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    for record in record_reader.feed(&buf[..n]) {
                        if record.is_empty() {
                            continue;
                        }
                        {
                            let mut screen = conn_clone.screen.lock().unwrap();
                            process_command(&mut screen, &record);
                        }
                        let screen = conn_clone.screen.lock().unwrap();
                        let _ = app_clone.emit("tn3270:screen", screen_snapshot(&id_clone, &screen));
                    }
                }
            }
        }
        let _ = app_clone.emit("tn3270:disconnected", &id_clone);
    });

    state.sessions.lock().unwrap().insert(id.clone(), conn.info.clone());
    state.conns.lock().unwrap().insert(id.clone(), conn);
    state.writers.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

/// Types `text` into `screen` starting at `addr`, marking the containing
/// field's MDT bit — mirrors a real 3270 keyboard, which refuses to enter
/// data into a protected field. Split out from the `tn3270_type` command
/// so it can be unit tested without a `tauri::State`.
fn type_into_screen(screen: &mut ScreenBuffer, addr: usize, text: &str) -> Result<(), Tn3270Error> {
    let size = screen.size();
    if size == 0 {
        return Err(Tn3270Error::ConnectionFailed("Screen buffer not initialized".into()));
    }
    if screen.is_protected(addr % size) {
        return Err(Tn3270Error::FieldProtected(addr));
    }
    let field_start = screen.field_attr_for(addr % size);
    let mut a = addr % size;
    for ch in text.chars() {
        screen.cells[a].ebcdic = ascii_to_ebcdic(ch as u8);
        a = (a + 1) % size;
    }
    if let Some(fa) = field_start {
        screen.cells[fa].mdt = true;
    }
    screen.cursor_addr = a;
    Ok(())
}

#[tauri::command]
pub fn tn3270_type(id: String, addr: usize, text: String, state: tauri::State<'_, Tn3270State>) -> Result<Tn3270Screen, Tn3270Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn3270Error::NotFound(id.clone()))?;
    let mut screen = conn.screen.lock().unwrap();
    type_into_screen(&mut screen, addr, &text)?;
    Ok(screen_snapshot(&id, &screen))
}

#[tauri::command]
pub fn tn3270_aid(id: String, aid: Tn3270Aid, state: tauri::State<'_, Tn3270State>) -> Result<(), Tn3270Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn3270Error::NotFound(id.clone()))?;
    let response = {
        let screen = conn.screen.lock().unwrap();
        build_read_modified_response(&screen, aid)
    };
    state
        .writers
        .lock()
        .unwrap()
        .get(&id)
        .ok_or_else(|| Tn3270Error::NotFound(id.clone()))?
        .send(response)
        .map_err(|e| Tn3270Error::ConnectionFailed(e.to_string()))
}

#[tauri::command]
pub fn tn3270_screen(id: String, state: tauri::State<'_, Tn3270State>) -> Result<Tn3270Screen, Tn3270Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn3270Error::NotFound(id.clone()))?;
    let screen = conn.screen.lock().unwrap();
    Ok(screen_snapshot(&id, &screen))
}

#[tauri::command]
pub fn tn3270_disconnect(id: String, state: tauri::State<'_, Tn3270State>) -> Result<(), Tn3270Error> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| Tn3270Error::NotFound(id.clone()))?;
    state.conns.lock().unwrap().remove(&id);
    state.writers.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn tn3270_list(state: tauri::State<'_, Tn3270State>) -> Vec<Tn3270SessionInfo> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_encode_address_roundtrip() {
        for addr in [0usize, 1, 63, 64, 100, 1919, 1920 - 1] {
            let [hi, lo] = encode_address(addr);
            assert_eq!(decode_address(hi, lo), addr, "roundtrip failed for {addr}");
        }
    }

    #[test]
    fn test_bit6_known_values() {
        // 0 -> low nibble 0, not in 1..10, not 48/33 -> | 0x40
        assert_eq!(bit6(0), 0x40);
        // 48 (0x30) is a special case -> | 0xC0
        assert_eq!(bit6(48), 0x30 | 0xC0);
        // 33 (0x21) is a special case -> | 0x40
        assert_eq!(bit6(33), 0x21 | 0x40);
        // low nibble 1-9 (not 48/33) -> | 0xC0
        assert_eq!(bit6(1), 1 | 0xC0);
    }

    #[test]
    fn test_parse_field_attr_protected_numeric_mdt() {
        let (protected, numeric, displayable, intensified, mdt) = parse_field_attr(0x20 | 0x10 | 0x01);
        assert!(protected);
        assert!(numeric);
        assert!(displayable);
        assert!(!intensified);
        assert!(mdt);
    }

    #[test]
    fn test_parse_field_attr_nondisplay() {
        let (_, _, displayable, _, _) = parse_field_attr(0x0C);
        assert!(!displayable);
    }

    #[test]
    fn test_process_command_erase_write_then_start_field_and_text() {
        let mut screen = ScreenBuffer::new(24, 80);
        // EW + WCC(0), SBA(0,0), SF(unprotected), "HI"
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_SF);
        data.push(0x00); // unprotected, normal
        data.push(b'H'); // raw byte treated as "ebcdic" char data in this synthetic test
        data.push(b'I');
        process_command(&mut screen, &data);

        assert!(screen.cells[0].field_start);
        assert!(!screen.cells[0].protected);
        assert_eq!(screen.cells[1].ebcdic, b'H');
        assert_eq!(screen.cells[2].ebcdic, b'I');
    }

    #[test]
    fn test_process_command_sba_ic_sets_cursor() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(42));
        data.push(ORDER_IC);
        process_command(&mut screen, &data);
        assert_eq!(screen.cursor_addr, 42);
    }

    #[test]
    fn test_repeat_to_address_fills_range() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_RA);
        data.extend_from_slice(&encode_address(5));
        data.push(b'.');
        process_command(&mut screen, &data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, b'.');
        }
        assert_ne!(screen.cells[5].ebcdic, b'.');
    }

    #[test]
    fn test_is_protected_scans_backward_to_field_start() {
        // Two fields: protected at 10, unprotected at 20 — a position must
        // pick up the attribute of the *nearest preceding* field, not just
        // any field on the screen (with only one field defined, that field
        // legitimately wraps around and governs the entire buffer, which
        // is what a real 3270 does — this test isolates the "nearest
        // preceding" behavior instead).
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(10));
        data.push(ORDER_SF);
        data.push(0x20); // protected
        data.push(ORDER_SBA);
        data.extend_from_slice(&encode_address(20));
        data.push(ORDER_SF);
        data.push(0x00); // unprotected
        process_command(&mut screen, &data);

        assert!(screen.is_protected(15)); // within the protected field (10..20)
        assert!(!screen.is_protected(25)); // within the unprotected field (20..10, wrapping)
    }

    #[test]
    fn test_build_read_modified_response_only_includes_mdt_fields() {
        let mut screen = ScreenBuffer::new(24, 80);
        // Two fields: [0]=unmodified field, [10]=modified field with "OK"
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_SF);
        data.push(0x00);
        data.extend_from_slice(&[ORDER_SBA]);
        data.extend_from_slice(&encode_address(10));
        data.push(ORDER_SF);
        data.push(0x00);
        data.push(b'O');
        data.push(b'K');
        process_command(&mut screen, &data);
        screen.cells[10].mdt = true; // simulate user having typed into this field

        let response = build_read_modified_response(&screen, Tn3270Aid::Enter);
        assert_eq!(response[0], Tn3270Aid::Enter.wire_value());
        // Only one SBA+data block should be present (for the modified field).
        let sba_count = response.iter().filter(|&&b| b == ORDER_SBA).count();
        assert_eq!(sba_count, 1);
        assert!(response.windows(2).any(|w| w == [b'O', b'K']));
    }

    #[test]
    fn test_type_into_screen_rejects_protected_field() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_SF);
        data.push(0x20); // protected
        process_command(&mut screen, &data);

        let result = type_into_screen(&mut screen, 5, "x");
        assert!(matches!(result, Err(Tn3270Error::FieldProtected(_))));
    }

    #[test]
    fn test_type_into_screen_sets_mdt_on_unprotected_field() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_SF);
        data.push(0x00); // unprotected
        process_command(&mut screen, &data);

        type_into_screen(&mut screen, 1, "hi").unwrap();
        assert!(screen.cells[0].mdt);
        assert_eq!(screen.cells[1].ebcdic, ascii_to_ebcdic(b'h'));
        assert_eq!(screen.cells[2].ebcdic, ascii_to_ebcdic(b'i'));
    }

    #[test]
    fn test_iac_escape_doubles_0xff() {
        let escaped = iac_escape(&[0x01, 0xFF, 0x02]);
        assert_eq!(escaped, vec![0x01, 0xFF, 0xFF, 0x02]);
    }

    #[test]
    fn test_record_reader_unstuffs_and_splits_on_eor() {
        let mut reader = RecordReader::new();
        // record containing a literal 0xFF (stuffed as IAC IAC), then IAC EOR
        let raw = [0x01u8, IAC, IAC, 0x02, IAC, EOR, 0x03];
        let records = reader.feed(&raw);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0], vec![0x01, 0xFF, 0x02]);
        assert_eq!(reader.pending, vec![0x03]); // leftover after the record boundary
    }

    #[test]
    fn test_record_reader_handles_split_across_feed_calls() {
        let mut reader = RecordReader::new();
        assert!(reader.feed(&[0x01, 0x02]).is_empty());
        let records = reader.feed(&[0x03, IAC, EOR]);
        assert_eq!(records, vec![vec![0x01, 0x02, 0x03]]);
    }

    #[test]
    fn test_record_reader_skips_interleaved_negotiation() {
        // DO OPT_BINARY interleaved mid-record should be skipped (3-byte
        // IAC/cmd/opt form), leaving only the real data bytes in the record.
        let mut reader = RecordReader::new();
        let raw = [0x01u8, IAC, DO, OPT_BINARY, 0x02, IAC, EOR];
        let records = reader.feed(&raw);
        assert_eq!(records, vec![vec![0x01, 0x02]]);
    }

    #[test]
    fn test_model_terminal_type_all_models() {
        assert_eq!(model_terminal_type(&Tn3270Model::Model2), "IBM-3278-2-E");
        assert_eq!(model_terminal_type(&Tn3270Model::Model3), "IBM-3278-3-E");
        assert_eq!(model_terminal_type(&Tn3270Model::Model4), "IBM-3278-4-E");
        assert_eq!(model_terminal_type(&Tn3270Model::Model5), "IBM-3278-5-E");
    }

    #[test]
    fn test_tn3270_model_dimensions_all() {
        assert_eq!(Tn3270Model::Model2.dimensions(), (24, 80));
        assert_eq!(Tn3270Model::Model3.dimensions(), (32, 80));
        assert_eq!(Tn3270Model::Model4.dimensions(), (43, 80));
        assert_eq!(Tn3270Model::Model5.dimensions(), (27, 132));
    }

    #[test]
    fn test_decode_address_masks_high_bits() {
        // Only the low 6 bits of each byte participate in addressing; any
        // higher bits set (e.g. from the bit6-encoded wire form) must not
        // affect the decoded address.
        assert_eq!(decode_address(0xC0, 0xC0), decode_address(0x00, 0x00));
        assert_eq!(decode_address(0x3F, 0x3F), 63 * 64 + 63);
    }

    #[test]
    fn test_screen_buffer_erase_resets_state() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.cells[0].ebcdic = b'X';
        screen.cursor_addr = 42;
        screen.buf_addr = 10;
        screen.erase();
        assert_eq!(screen.cells[0].ebcdic, 0);
        assert_eq!(screen.cursor_addr, 0);
        assert_eq!(screen.buf_addr, 0);
    }

    #[test]
    fn test_field_attr_for_no_fields_is_unprotected() {
        // A freshly-created (unformatted) screen has no field-start cells at
        // all, so field_attr_for must return None and is_protected must be
        // false everywhere.
        let screen = ScreenBuffer::new(24, 80);
        assert_eq!(screen.field_attr_for(0), None);
        assert!(!screen.is_protected(0));
        assert!(!screen.is_protected(1919));
    }

    #[test]
    fn test_process_command_empty_data_does_not_panic() {
        let mut screen = ScreenBuffer::new(24, 80);
        process_command(&mut screen, &[]);
        // Screen should remain untouched.
        assert_eq!(screen.cursor_addr, 0);
        assert_eq!(screen.buf_addr, 0);
    }

    #[test]
    fn test_process_command_read_commands_are_noop() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.cells[0].ebcdic = b'X';
        process_command(&mut screen, &[CMD_RB, 0xAA, 0xBB]);
        process_command(&mut screen, &[CMD_RM, 0xAA, 0xBB]);
        process_command(&mut screen, &[CMD_WSF, 0xAA, 0xBB]);
        // None of these carry orders to apply; the buffer must be unchanged.
        assert_eq!(screen.cells[0].ebcdic, b'X');
    }

    #[test]
    fn test_process_command_sfe_order_applies_basic_attribute() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        // SFE with one attribute pair: type 0xC0 (basic 3270 attribute) = protected (0x20)
        data.push(ORDER_SFE);
        data.push(1); // pair_count
        data.push(0xC0);
        data.push(0x20); // protected
        process_command(&mut screen, &data);

        assert!(screen.cells[0].field_start);
        assert!(screen.cells[0].protected);
    }

    #[test]
    fn test_process_command_eua_order_erases_unprotected_range() {
        let mut screen = ScreenBuffer::new(24, 80);
        // Write some data into cells 0..5, then EUA over that range.
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        for _ in 0..5 {
            data.push(b'.');
        }
        process_command(&mut screen, &data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, b'.');
        }

        // Use CMD_W (no implicit erase) so this call genuinely exercises EUA
        // overwriting the previously-written cells, rather than relying on
        // CMD_EW's own erase to blank them first.
        let mut erase_data = vec![CMD_W, 0x00, ORDER_SBA];
        erase_data.extend_from_slice(&encode_address(0));
        erase_data.push(ORDER_EUA);
        erase_data.extend_from_slice(&encode_address(5));
        process_command(&mut screen, &erase_data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, 0x40);
        }
    }

    #[test]
    fn test_screen_snapshot_reflects_buffer_state() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_EW, 0x00, ORDER_SBA];
        data.extend_from_slice(&encode_address(0));
        data.push(ORDER_IC);
        process_command(&mut screen, &data);

        let snapshot = screen_snapshot("sess-1", &screen);
        assert_eq!(snapshot.session_id, "sess-1");
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cols, 80);
        assert_eq!(snapshot.cursor_addr, 0);
        assert_eq!(snapshot.cells.len(), 24 * 80);
    }
}

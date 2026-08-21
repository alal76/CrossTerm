/// IBM AS/400 5250 terminal over TCP with TN5250E option negotiation.
///
/// The previous version of this module only handled Telnet negotiation and
/// piped raw bytes to the frontend — no 5250 record/order parsing, no
/// screen buffer, and (like the TN3270 module before its fix) no IAC
/// byte-stuffing.
///
/// 5250's data stream is structurally its own protocol, not a variant of
/// 3270's — despite SBA/SF/IC sharing the same order-code *numbers* as
/// 3270, their argument encodings differ completely (5250 uses plain
/// 1-based row/col byte pairs, not 3270's packed 12-bit buffer address),
/// RA uses a different code entirely (0x02 vs 3270's 0x3C), field
/// attributes are a 2-byte Field Format Word + a separate 1-byte color
/// attribute rather than 3270's single bitfield byte, and every record is
/// wrapped in its own 10-byte header (RFC 1205 §I) before the Telnet
/// EOR-delimited framing even starts. Every byte layout below — the record
/// header, order codes, the FFW bit assignments, and the SF field-vs-
/// attribute disambiguation rule (a byte belongs to the field format word
/// unless its top 3 bits equal 001) — is cross-checked against the classic
/// `tn5250` C client (github.com/hharte/tn5250, forked from
/// sourceforge.net/projects/tn5250) rather than recalled from memory.
/// Unverified against a real AS/400 — none reachable from this environment.
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
pub enum Tn5250Error {
    #[error("Connection not found: {0}")]
    NotFound(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Row {0}, column {1} is bypassed (protected)")]
    FieldBypassed(usize, usize),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for Tn5250Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250Config {
    pub host: String,
    pub port: u16,
    /// Optional virtual device name (up to 10 chars)
    pub device_name: Option<String>,
    /// Optional system name (host EBCDIC name)
    pub system_name: Option<String>,
    pub ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250SessionInfo {
    pub id: String,
    pub host: String,
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250CellInfo {
    pub ch: char,
    pub bypass: bool,
    pub numeric: bool,
    pub nondisplay: bool,
    pub mandatory: bool,
    pub field_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tn5250Screen {
    pub session_id: String,
    pub rows: usize,
    pub cols: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub cells: Vec<Tn5250CellInfo>,
}

// ── AID keys ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tn5250Aid {
    Enter,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Clear,
    Help,
    RollUp,
    RollDown,
}

impl Tn5250Aid {
    fn wire_value(self) -> u8 {
        match self {
            Tn5250Aid::Enter => 0xF1,
            Tn5250Aid::F1 => 0x31,
            Tn5250Aid::F2 => 0x32,
            Tn5250Aid::F3 => 0x33,
            Tn5250Aid::F4 => 0x34,
            Tn5250Aid::F5 => 0x35,
            Tn5250Aid::F6 => 0x36,
            Tn5250Aid::F7 => 0x37,
            Tn5250Aid::F8 => 0x38,
            Tn5250Aid::F9 => 0x39,
            Tn5250Aid::F10 => 0x3A,
            Tn5250Aid::F11 => 0x3B,
            Tn5250Aid::F12 => 0x3C,
            Tn5250Aid::Clear => 0xBD,
            Tn5250Aid::Help => 0xF3,
            Tn5250Aid::RollUp => 0xF5,
            Tn5250Aid::RollDown => 0xF4,
        }
    }
}

// ── Commands (verified against tn5250's codes5250.h) ──────────────────────
const CMD_CLEAR_UNIT: u8 = 0x40;
const CMD_CLEAR_UNIT_ALTERNATE: u8 = 0x20;
const CMD_WRITE_TO_DISPLAY: u8 = 0x11;
const CMD_READ_INPUT_FIELDS: u8 = 0x42;
const CMD_READ_MDT_FIELDS: u8 = 0x52;

// ── Orders — NOTE: RA differs from 3270 (0x02 here, not 0x3C); SBA/SF/IC
// share the same numeric codes as 3270 but completely different argument
// encodings (see module docs).
const ORDER_SOH: u8 = 0x01;
const ORDER_RA: u8 = 0x02;
const ORDER_EA: u8 = 0x03;
const ORDER_SBA: u8 = 0x11;
const ORDER_IC: u8 = 0x13;
const ORDER_SF: u8 = 0x1D;

// ── Field Format Word bits (verified against tn5250's field.h) ───────────
const FFW_BYPASS: u16 = 0x2000;
const FFW_MODIFIED: u16 = 0x0800;
const FFW_FIELD_TYPE_MASK: u16 = 0x0700;
const FFW_NUM_SHIFT: u16 = 0x0200;
const FFW_NUM_ONLY: u16 = 0x0300;
const FFW_DIGIT_ONLY: u16 = 0x0500;
const FFW_SIGNED_NUM: u16 = 0x0700;
const FFW_MANDATORY: u16 = 0x0008;

const ATTR_NONDISP: u8 = 0x27;

fn ffw_is_numeric(ffw: u16) -> bool {
    matches!(ffw & FFW_FIELD_TYPE_MASK, FFW_NUM_SHIFT | FFW_NUM_ONLY | FFW_DIGIT_ONLY | FFW_SIGNED_NUM)
}

/// A byte belongs to a Field Format/Control Word unless its top 3 bits are
/// `001` (0x20-0x3F range) — that pattern is reserved for the trailing
/// display-attribute byte. Verified against tn5250's
/// `tn5250_session_start_of_field` read loop (`(cur_char & 0xe0) != 0x20`).
fn is_attribute_byte(b: u8) -> bool {
    b & 0xE0 == 0x20
}

#[derive(Debug, Clone, Default)]
struct Cell {
    ebcdic: u8,
    field_start: bool,
    bypass: bool,
    numeric: bool,
    nondisplay: bool,
    mandatory: bool,
    mdt: bool,
}

pub struct ScreenBuffer {
    rows: usize,
    cols: usize,
    cells: Vec<Cell>,
    cursor_row: usize,
    cursor_col: usize,
}

impl ScreenBuffer {
    fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, cells: vec![Cell::default(); rows * cols], cursor_row: 0, cursor_col: 0 }
    }

    fn size(&self) -> usize {
        self.rows * self.cols
    }

    fn erase(&mut self) {
        self.cells = vec![Cell::default(); self.size()];
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn addr(&self, row: usize, col: usize) -> usize {
        (row % self.rows) * self.cols + (col % self.cols)
    }

    fn cursor_addr(&self) -> usize {
        self.addr(self.cursor_row, self.cursor_col)
    }

    fn advance_cursor(&mut self) {
        let size = self.size();
        let a = (self.cursor_addr() + 1) % size.max(1);
        self.cursor_row = a / self.cols;
        self.cursor_col = a % self.cols;
    }

    fn field_start_for(&self, addr: usize) -> Option<usize> {
        let n = self.size();
        for step in 0..n {
            let a = (addr + n - step) % n;
            if self.cells[a].field_start {
                return Some(a);
            }
        }
        None
    }

    fn is_bypassed(&self, addr: usize) -> bool {
        self.field_start_for(addr).map(|fa| self.cells[fa].bypass).unwrap_or(false)
    }
}

/// Reads a Start-of-Field's format/control/attribute bytes starting at
/// `data[i]` (the byte immediately after the `SF` order code). Returns the
/// parsed field plus the index just past its last consumed byte — orders
/// with an unexpectedly-truncated field just return `None` for FFW/attr
/// rather than panicking, so a malformed stream degrades instead of
/// crashing the parser.
fn parse_start_of_field(data: &[u8], i: usize) -> (Option<u16>, u8, usize) {
    if i >= data.len() {
        return (None, ATTR_NONDISP, i);
    }
    let mut j = i;
    let first = data[j];
    if is_attribute_byte(first) {
        // Output-only field: just the display attribute, no FFW.
        return (None, first, j + 1);
    }
    if j + 1 >= data.len() {
        return (None, ATTR_NONDISP, data.len());
    }
    let ffw = ((data[j] as u16) << 8) | data[j + 1] as u16;
    j += 2;
    // Zero or more 2-byte Field Control Words follow, until a byte with the
    // attribute-byte bit pattern is seen.
    while j < data.len() && !is_attribute_byte(data[j]) {
        j += 2; // skip FCW pair without modeling per-FCW behavior (rare, cosmetic)
    }
    let attr = if j < data.len() { data[j] } else { ATTR_NONDISP };
    (Some(ffw), attr, j + 1)
}

fn apply_field_attrs(cell: &mut Cell, ffw: Option<u16>, attr: u8) {
    cell.field_start = true;
    cell.ebcdic = 0x40;
    cell.nondisplay = attr == ATTR_NONDISP;
    match ffw {
        Some(ffw) => {
            cell.bypass = ffw & FFW_BYPASS != 0;
            cell.numeric = ffw_is_numeric(ffw);
            cell.mandatory = ffw & FFW_MANDATORY != 0;
            cell.mdt = ffw & FFW_MODIFIED != 0;
        }
        None => {
            // Output-only field: always bypassed (can't be typed into).
            cell.bypass = true;
            cell.numeric = false;
            cell.mandatory = false;
            cell.mdt = false;
        }
    }
}

/// Parses one 5250 command (the bytes after the 10-byte record header) and
/// applies it to the screen buffer. Row/col in the wire format are 1-based;
/// stored/returned addresses are 0-based.
fn process_command(screen: &mut ScreenBuffer, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let cmd = data[0];
    let mut i = match cmd {
        CMD_CLEAR_UNIT | CMD_CLEAR_UNIT_ALTERNATE => {
            screen.erase();
            return;
        }
        CMD_WRITE_TO_DISPLAY => 1,
        CMD_READ_INPUT_FIELDS | CMD_READ_MDT_FIELDS => return, // read commands carry no orders
        _ => 1,
    };

    // Write to Display carries a 2-byte Control Character before orders.
    if cmd == CMD_WRITE_TO_DISPLAY {
        if data.len() < 3 {
            return;
        }
        let cc1 = data[1];
        if cc1 & 0x40 != 0 {
            // "Clear all" control bit
            screen.erase();
        }
        i = 3;
    }

    while i < data.len() {
        let order = data[i];
        match order {
            ORDER_SOH => {
                // Start-of-Header — carries an error-recovery cursor
                // position we don't otherwise use; skip its length byte
                // plus that many bytes.
                if i + 1 < data.len() {
                    let len = data[i + 1] as usize;
                    i += 2 + len;
                } else {
                    break;
                }
            }
            ORDER_SBA if i + 2 < data.len() => {
                let row = data[i + 1].saturating_sub(1) as usize;
                let col = data[i + 2].saturating_sub(1) as usize;
                screen.cursor_row = row % screen.rows;
                screen.cursor_col = col % screen.cols;
                i += 3;
            }
            ORDER_IC if i + 2 < data.len() => {
                let row = data[i + 1].saturating_sub(1) as usize;
                let col = data[i + 2].saturating_sub(1) as usize;
                screen.cursor_row = row % screen.rows;
                screen.cursor_col = col % screen.cols;
                i += 3;
            }
            ORDER_RA if i + 3 < data.len() => {
                let target_row = data[i + 1].saturating_sub(1) as usize % screen.rows;
                let target_col = data[i + 2].saturating_sub(1) as usize % screen.cols;
                let fill = data[i + 3];
                let target_addr = screen.addr(target_row, target_col);
                loop {
                    let a = screen.cursor_addr();
                    screen.cells[a].ebcdic = fill;
                    screen.cells[a].field_start = false;
                    let reached = a == target_addr;
                    screen.advance_cursor();
                    if reached {
                        break;
                    }
                }
                i += 4;
            }
            ORDER_EA if i + 3 < data.len() => {
                let target_row = data[i + 1].saturating_sub(1) as usize % screen.rows;
                let target_col = data[i + 2].saturating_sub(1) as usize % screen.cols;
                let extra_len = (data[i + 3] as usize).saturating_sub(1);
                let target_addr = screen.addr(target_row, target_col);
                let mut a = screen.cursor_addr();
                while a != target_addr {
                    if !screen.cells[a].bypass {
                        screen.cells[a].ebcdic = 0x40;
                    }
                    a = (a + 1) % screen.size().max(1);
                }
                i += 4 + extra_len;
            }
            ORDER_SF => {
                let (ffw, attr, next_i) = parse_start_of_field(data, i + 1);
                let addr = screen.cursor_addr();
                let mut cell = Cell::default();
                apply_field_attrs(&mut cell, ffw, attr);
                screen.cells[addr] = cell;
                screen.advance_cursor();
                i = next_i;
            }
            _ => {
                // Ordinary character data.
                let a = screen.cursor_addr();
                screen.cells[a].ebcdic = order;
                screen.cells[a].field_start = false;
                screen.advance_cursor();
                i += 1;
            }
        }
    }
}

/// Builds the AID-key response for Read MDT Fields semantics: AID byte +
/// cursor row/col (1-based), followed by `SBA(field_start) + field_data`
/// for every field whose MDT bit is set.
fn build_aid_response(screen: &ScreenBuffer, aid: Tn5250Aid) -> Vec<u8> {
    let mut out = vec![aid.wire_value(), (screen.cursor_row + 1) as u8, (screen.cursor_col + 1) as u8];

    let size = screen.size();
    for i in 0..size {
        if screen.cells[i].field_start && screen.cells[i].mdt {
            let row = i / screen.cols;
            let col = i % screen.cols;
            out.push(ORDER_SBA);
            out.push((row + 1) as u8);
            out.push((col + 1) as u8);
            let data_start = (i + 1) % size;
            let mut a = data_start;
            while a != i && !screen.cells[a].field_start {
                out.push(screen.cells[a].ebcdic);
                a = (a + 1) % size;
            }
        }
    }
    out
}

fn screen_snapshot(session_id: &str, screen: &ScreenBuffer) -> Tn5250Screen {
    Tn5250Screen {
        session_id: session_id.to_string(),
        rows: screen.rows,
        cols: screen.cols,
        cursor_row: screen.cursor_row,
        cursor_col: screen.cursor_col,
        cells: screen
            .cells
            .iter()
            .map(|c| Tn5250CellInfo {
                ch: ebcdic_to_ascii(c.ebcdic) as char,
                bypass: c.bypass,
                numeric: c.numeric,
                nondisplay: c.nondisplay,
                mandatory: c.mandatory,
                field_start: c.field_start,
            })
            .collect(),
    }
}

fn type_into_screen(screen: &mut ScreenBuffer, row: usize, col: usize, text: &str) -> Result<(), Tn5250Error> {
    let addr = screen.addr(row, col);
    if screen.is_bypassed(addr) {
        return Err(Tn5250Error::FieldBypassed(row, col));
    }
    let field_start = screen.field_start_for(addr);
    screen.cursor_row = row % screen.rows;
    screen.cursor_col = col % screen.cols;
    for ch in text.chars() {
        let a = screen.cursor_addr();
        screen.cells[a].ebcdic = ascii_to_ebcdic(ch as u8);
        screen.advance_cursor();
    }
    if let Some(fa) = field_start {
        screen.cells[fa].mdt = true;
    }
    Ok(())
}

// ── Record framing (RFC 1205 §I: 10-byte GDS header, then Telnet
// IAC-EOR-delimited framing exactly like TN3270E) ──────────────────────────

const IAC: u8 = 0xFF;
const DO: u8 = 0xFD;
const WILL: u8 = 0xFB;
#[allow(dead_code)] // documents the full negotiation opcode set; server WONT is not distinguished today
const WONT: u8 = 0xFC;
#[allow(dead_code)] // documents the full negotiation opcode set; server DONT is not distinguished today
const DONT: u8 = 0xFE;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;
const OPT_BINARY: u8 = 0x00;
const OPT_EOR: u8 = 0x19;
const OPT_TTYPE: u8 = 0x18;
const OPT_TN5250E: u8 = 0x28; // RFC 2877
const EOR: u8 = 0xEF;

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

struct RecordReader {
    pending: Vec<u8>,
}

impl RecordReader {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

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

/// Strips the 10-byte GDS record header, returning the 5250 command bytes
/// that follow it (opcode onward, per `tn5250_record_opcode` = `data[9]`).
fn strip_record_header(record: &[u8]) -> &[u8] {
    if record.len() > 10 {
        &record[9..]
    } else {
        &[]
    }
}

fn build_gds_record_header() -> [u8; 10] {
    let mut h = [0u8; 10];
    // [0..2] logical record length is filled in by the caller once the
    // full record is assembled.
    h[2] = 0x12;
    h[3] = 0xA0; // GDS record type marker
    h[6] = 0x04; // variable header length
                 // flow type [4..6], flags [7], reserved [8], opcode [9] all default to
                 // DISPLAY/no-flags/no-op, overwritten by the caller as needed.
    h
}

pub struct Tn5250Conn {
    info: Tn5250SessionInfo,
    screen: Mutex<ScreenBuffer>,
}

pub struct Tn5250State {
    sessions: Mutex<HashMap<String, Tn5250SessionInfo>>,
    conns: Mutex<HashMap<String, std::sync::Arc<Tn5250Conn>>>,
    writers: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
}

impl Tn5250State {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()), conns: Mutex::new(HashMap::new()), writers: Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn tn5250_connect(config: Tn5250Config, state: tauri::State<'_, Tn5250State>, app: AppHandle) -> Result<String, Tn5250Error> {
    let addr = format!("{}:{}", config.host, config.port);
    let stream = TcpStream::connect(&addr).await?;
    let id = Uuid::new_v4().to_string();

    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let device = config.device_name.clone().unwrap_or_else(|| "QPADEV0001".to_string());

    tokio::spawn(async move {
        let _ = writer.write_all(&[IAC, DO, OPT_BINARY, IAC, DO, OPT_EOR, IAC, WILL, OPT_TN5250E, IAC, WILL, OPT_TTYPE]).await;

        let mut ttype: Vec<u8> = vec![IAC, SB, OPT_TN5250E, 0x02]; // TERMINAL-TYPE
        ttype.extend_from_slice(b"IBM-5555-C01");
        ttype.push(0x01); // separator
        ttype.extend_from_slice(device.as_bytes());
        ttype.extend_from_slice(&[IAC, SE]);
        let _ = writer.write_all(&ttype).await;

        while let Some(record_body) = rx.recv().await {
            // Wrap the 5250 command bytes in a GDS record header — length
            // covers header + body, per RFC 1205 (measured before IAC
            // doubling and excluding the trailing IAC EOR marker).
            let mut header = build_gds_record_header();
            let total_len = (header.len() + record_body.len()) as u16;
            header[0..2].copy_from_slice(&total_len.to_be_bytes());
            header[9] = 0; // opcode: no-op wrapper — the command bytes carry the real opcode

            let mut record = header.to_vec();
            record.extend_from_slice(&record_body);
            let escaped = iac_escape(&record);
            if writer.write_all(&escaped).await.is_err() {
                break;
            }
            let _ = writer.write_all(&[IAC, EOR]).await;
        }
    });

    let (rows, cols) = (24usize, 80usize);
    let conn = std::sync::Arc::new(Tn5250Conn {
        info: Tn5250SessionInfo { id: id.clone(), host: config.host.clone(), device_name: config.device_name.clone() },
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
                        let command = strip_record_header(&record);
                        if command.is_empty() {
                            continue;
                        }
                        {
                            let mut screen = conn_clone.screen.lock().unwrap();
                            process_command(&mut screen, command);
                        }
                        let screen = conn_clone.screen.lock().unwrap();
                        let _ = app_clone.emit("tn5250:screen", screen_snapshot(&id_clone, &screen));
                    }
                }
            }
        }
        let _ = app_clone.emit("tn5250:disconnected", &id_clone);
    });

    state.sessions.lock().unwrap().insert(id.clone(), conn.info.clone());
    state.conns.lock().unwrap().insert(id.clone(), conn);
    state.writers.lock().unwrap().insert(id.clone(), tx);
    Ok(id)
}

#[tauri::command]
pub fn tn5250_type(id: String, row: usize, col: usize, text: String, state: tauri::State<'_, Tn5250State>) -> Result<Tn5250Screen, Tn5250Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn5250Error::NotFound(id.clone()))?;
    let mut screen = conn.screen.lock().unwrap();
    type_into_screen(&mut screen, row, col, &text)?;
    Ok(screen_snapshot(&id, &screen))
}

#[tauri::command]
pub fn tn5250_aid(id: String, aid: Tn5250Aid, state: tauri::State<'_, Tn5250State>) -> Result<(), Tn5250Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn5250Error::NotFound(id.clone()))?;
    let response = {
        let screen = conn.screen.lock().unwrap();
        build_aid_response(&screen, aid)
    };
    state
        .writers
        .lock()
        .unwrap()
        .get(&id)
        .ok_or_else(|| Tn5250Error::NotFound(id.clone()))?
        .send(response)
        .map_err(|e| Tn5250Error::ConnectionFailed(e.to_string()))
}

#[tauri::command]
pub fn tn5250_screen(id: String, state: tauri::State<'_, Tn5250State>) -> Result<Tn5250Screen, Tn5250Error> {
    let conn = state.conns.lock().unwrap().get(&id).cloned().ok_or_else(|| Tn5250Error::NotFound(id.clone()))?;
    let screen = conn.screen.lock().unwrap();
    Ok(screen_snapshot(&id, &screen))
}

#[tauri::command]
pub fn tn5250_disconnect(id: String, state: tauri::State<'_, Tn5250State>) -> Result<(), Tn5250Error> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| Tn5250Error::NotFound(id.clone()))?;
    state.conns.lock().unwrap().remove(&id);
    state.writers.lock().unwrap().remove(&id);
    Ok(())
}

#[tauri::command]
pub fn tn5250_list(state: tauri::State<'_, Tn5250State>) -> Vec<Tn5250SessionInfo> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_attribute_byte() {
        assert!(is_attribute_byte(0x20));
        assert!(is_attribute_byte(0x27)); // nondisplay
        assert!(is_attribute_byte(0x3A)); // blue
        assert!(!is_attribute_byte(0x00));
        assert!(!is_attribute_byte(0xC1)); // an EBCDIC letter, not an attribute
    }

    #[test]
    fn test_parse_start_of_field_output_only() {
        let data = [0x22u8]; // just an attribute byte, no FFW
        let (ffw, attr, next) = parse_start_of_field(&data, 0);
        assert_eq!(ffw, None);
        assert_eq!(attr, 0x22);
        assert_eq!(next, 1);
    }

    #[test]
    fn test_parse_start_of_field_input_field_with_ffw() {
        // FFW1 must fall outside 0x20-0x3F or it collides with the
        // attribute-byte pattern (see FFW_BYPASS's own byte value, 0x20 —
        // a field with *only* bypass set is genuinely ambiguous in the
        // wire protocol itself, which is presumably why real hosts don't
        // emit bypass-only fields as input fields; this test instead uses
        // MANDATORY (0x0008), which doesn't collide).
        let data = [0x00u8, FFW_MANDATORY as u8, 0x20];
        let (ffw, parsed_attr, next) = parse_start_of_field(&data, 0);
        assert_eq!(ffw, Some(FFW_MANDATORY));
        assert_eq!(parsed_attr, 0x20);
        assert_eq!(next, 3);
    }

    #[test]
    fn test_parse_start_of_field_skips_fcw_before_attribute() {
        let data = [0x03u8, 0x00, 0x86, 0x01, 0x20]; // FFW, one FCW pair, then attr
        let (ffw, attr, next) = parse_start_of_field(&data, 0);
        assert_eq!(ffw, Some(0x0300));
        assert_eq!(attr, 0x20);
        assert_eq!(next, 5);
    }

    #[test]
    fn test_ffw_is_numeric() {
        assert!(ffw_is_numeric(FFW_NUM_ONLY));
        assert!(ffw_is_numeric(FFW_SIGNED_NUM));
        assert!(!ffw_is_numeric(0x0000)); // alpha shift
        assert!(!ffw_is_numeric(0x0100)); // alpha only
    }

    #[test]
    fn test_process_write_to_display_start_field_and_text() {
        let mut screen = ScreenBuffer::new(24, 80);
        // WTD, CC1=0, CC2=0, SBA(1,1), SF(output-only, always bypassed),
        // then SBA(1,2) SF(input, unprotected) "HI".
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_SF, 0x20]); // output-only field, attr green
        data.extend_from_slice(&[ORDER_SBA, 1, 2]);
        data.extend_from_slice(&[ORDER_SF, 0x00, 0x00, 0x20]); // input field, FFW=0 (not bypassed)
        data.push(b'H');
        data.push(b'I');
        process_command(&mut screen, &data);

        assert!(screen.cells[0].field_start);
        assert!(screen.cells[0].bypass);
        assert!(screen.cells[1].field_start);
        assert!(!screen.cells[1].bypass);
        assert_eq!(screen.cells[2].ebcdic, b'H');
        assert_eq!(screen.cells[3].ebcdic, b'I');
    }

    #[test]
    fn test_repeat_to_address_fills_from_cursor() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_RA, 1, 5, b'.']);
        process_command(&mut screen, &data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, b'.');
        }
        assert_ne!(screen.cells[5].ebcdic, b'.');
    }

    #[test]
    fn test_insert_cursor_sets_position() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_IC, 3, 10]);
        process_command(&mut screen, &data);
        assert_eq!(screen.cursor_row, 2);
        assert_eq!(screen.cursor_col, 9);
    }

    #[test]
    fn test_clear_unit_resets_buffer() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.cells[0].ebcdic = b'X';
        process_command(&mut screen, &[CMD_CLEAR_UNIT]);
        assert_eq!(screen.cells[0].ebcdic, 0);
    }

    #[test]
    fn test_type_into_screen_rejects_bypassed_field() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_SF, 0x20]); // output-only field, always bypassed
        process_command(&mut screen, &data);

        let result = type_into_screen(&mut screen, 0, 1, "x");
        assert!(matches!(result, Err(Tn5250Error::FieldBypassed(_, _))));
    }

    #[test]
    fn test_type_into_screen_sets_mdt() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_SF, 0x00, 0x00, 0x20]); // input, unprotected
        process_command(&mut screen, &data);

        type_into_screen(&mut screen, 0, 1, "hi").unwrap();
        assert!(screen.cells[0].mdt);
        assert_eq!(screen.cells[1].ebcdic, ascii_to_ebcdic(b'h'));
    }

    #[test]
    fn test_build_aid_response_includes_only_mdt_fields() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_SF, 0x00, 0x00, 0x20]);
        data.push(b'O');
        data.push(b'K');
        process_command(&mut screen, &data);
        screen.cells[0].mdt = true;

        let response = build_aid_response(&screen, Tn5250Aid::Enter);
        assert_eq!(response[0], Tn5250Aid::Enter.wire_value());
        let sba_count = response.iter().filter(|&&b| b == ORDER_SBA).count();
        assert_eq!(sba_count, 1);
        assert!(response.windows(2).any(|w| w == [b'O', b'K']));
    }

    #[test]
    fn test_iac_escape_and_record_reader_roundtrip() {
        let escaped = iac_escape(&[0x01, 0xFF, 0x02]);
        assert_eq!(escaped, vec![0x01, 0xFF, 0xFF, 0x02]);

        let mut reader = RecordReader::new();
        let raw = [0x01u8, IAC, IAC, 0x02, IAC, EOR];
        let records = reader.feed(&raw);
        assert_eq!(records, vec![vec![0x01, 0xFF, 0x02]]);
    }

    #[test]
    fn test_strip_record_header() {
        let mut record = build_gds_record_header().to_vec();
        record[9] = CMD_WRITE_TO_DISPLAY;
        record.extend_from_slice(&[0x00, 0x00]);
        let command = strip_record_header(&record);
        assert_eq!(command[0], CMD_WRITE_TO_DISPLAY);
    }

    #[test]
    fn test_build_gds_record_header_marker() {
        let h = build_gds_record_header();
        assert_eq!(&h[2..4], &[0x12, 0xA0]);
        assert_eq!(h[6], 0x04);
    }

    #[test]
    fn test_strip_record_header_short_record_returns_empty() {
        // Records at or below the 10-byte header length carry no command
        // bytes at all.
        assert_eq!(strip_record_header(&[0u8; 10]), &[] as &[u8]);
        assert_eq!(strip_record_header(&[0u8; 5]), &[] as &[u8]);
    }

    #[test]
    fn test_record_reader_skips_interleaved_negotiation() {
        let mut reader = RecordReader::new();
        let raw = [0x01u8, IAC, DO, OPT_BINARY, 0x02, IAC, EOR];
        let records = reader.feed(&raw);
        assert_eq!(records, vec![vec![0x01, 0x02]]);
    }

    #[test]
    fn test_process_command_empty_data_does_not_panic() {
        let mut screen = ScreenBuffer::new(24, 80);
        process_command(&mut screen, &[]);
        assert_eq!(screen.cursor_row, 0);
        assert_eq!(screen.cursor_col, 0);
    }

    #[test]
    fn test_process_command_read_commands_are_noop() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.cells[0].ebcdic = b'X';
        process_command(&mut screen, &[CMD_READ_INPUT_FIELDS, 0xAA]);
        process_command(&mut screen, &[CMD_READ_MDT_FIELDS, 0xAA]);
        assert_eq!(screen.cells[0].ebcdic, b'X');
    }

    #[test]
    fn test_process_command_clear_unit_alternate_resets_buffer() {
        let mut screen = ScreenBuffer::new(24, 80);
        screen.cells[0].ebcdic = b'X';
        process_command(&mut screen, &[CMD_CLEAR_UNIT_ALTERNATE]);
        assert_eq!(screen.cells[0].ebcdic, 0);
    }

    #[test]
    fn test_process_command_soh_skips_length_prefixed_bytes() {
        let mut screen = ScreenBuffer::new(24, 80);
        // WTD, CC1=0, CC2=0, SOH with length 2 (skip 2 bytes), then "HI"
        // written starting at the cursor's default position (0,0).
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SOH, 2, 0xAA, 0xBB]);
        data.push(b'H');
        data.push(b'I');
        process_command(&mut screen, &data);
        assert_eq!(screen.cells[0].ebcdic, b'H');
        assert_eq!(screen.cells[1].ebcdic, b'I');
    }

    #[test]
    fn test_process_command_ea_order_erases_unprotected_range() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        data.extend_from_slice(&[ORDER_RA, 1, 5, b'.']);
        process_command(&mut screen, &data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, b'.');
        }

        let mut erase_data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        erase_data.extend_from_slice(&[ORDER_SBA, 1, 1]);
        // EA: target row/col (1,6), extra_len byte = 1 (no extra bytes consumed)
        erase_data.extend_from_slice(&[ORDER_EA, 1, 6, 1]);
        process_command(&mut screen, &erase_data);
        for cell in &screen.cells[0..5] {
            assert_eq!(cell.ebcdic, 0x40);
        }
    }

    #[test]
    fn test_field_start_for_no_fields_is_not_bypassed() {
        let screen = ScreenBuffer::new(24, 80);
        assert_eq!(screen.field_start_for(0), None);
        assert!(!screen.is_bypassed(0));
    }

    #[test]
    fn test_advance_cursor_wraps_rows() {
        let mut screen = ScreenBuffer::new(2, 3);
        screen.cursor_row = 0;
        screen.cursor_col = 2;
        screen.advance_cursor();
        assert_eq!(screen.cursor_row, 1);
        assert_eq!(screen.cursor_col, 0);
    }

    #[test]
    fn test_screen_addr_wraps_out_of_range_row_col() {
        let screen = ScreenBuffer::new(24, 80);
        // row/col beyond bounds must wrap via modulo, never panic/OOB.
        assert_eq!(screen.addr(24, 0), screen.addr(0, 0));
        assert_eq!(screen.addr(0, 80), screen.addr(0, 0));
    }

    #[test]
    fn test_screen_snapshot_reflects_buffer_state() {
        let mut screen = ScreenBuffer::new(24, 80);
        let mut data = vec![CMD_WRITE_TO_DISPLAY, 0x00, 0x00];
        data.extend_from_slice(&[ORDER_IC, 3, 10]);
        process_command(&mut screen, &data);

        let snapshot = screen_snapshot("sess-1", &screen);
        assert_eq!(snapshot.session_id, "sess-1");
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cols, 80);
        assert_eq!(snapshot.cursor_row, 2);
        assert_eq!(snapshot.cursor_col, 9);
        assert_eq!(snapshot.cells.len(), 24 * 80);
    }
}

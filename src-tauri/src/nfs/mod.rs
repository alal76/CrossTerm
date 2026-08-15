/// Minimal read-only NFSv3 client, hand-rolled: no mature async NFS crate
/// exists for Rust, so this implements just enough of ONC RPC (RFC 1831),
/// XDR (RFC 4506), the portmapper (RFC 1833) GETPORT call, the MOUNT
/// protocol (RFC 1813 Appendix I), and NFSv3 (RFC 1813) to browse an
/// export and read file contents: LOOKUP, READDIRPLUS, READ, GETATTR.
/// Write operations (CREATE/WRITE/REMOVE/...) are intentionally out of
/// scope, matching the "Explorer" (read/browse) framing of this session
/// type rather than a full mount client. Kerberos (RPCSEC_GSS) auth is
/// not supported — only AUTH_NONE and AUTH_SYS (uid/gid) — the same
/// disclosed-limitation approach already used for WinRM's Kerberos gap.
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

const PMAP_PROG: u32 = 100000;
const PMAP_VERS: u32 = 2;
const PMAP_PROC_GETPORT: u32 = 3;
const IPPROTO_TCP: u32 = 6;

const MOUNT_PROG: u32 = 100005;
const MOUNT_VERS: u32 = 3;
const MOUNTPROC3_MNT: u32 = 1;

const NFS_PROG: u32 = 100003;
const NFS_VERS: u32 = 3;
const NFSPROC3_LOOKUP: u32 = 3;
const NFSPROC3_READ: u32 = 6;
const NFSPROC3_READDIRPLUS: u32 = 17;

/// Well-known NFS data port, used as a fallback when the portmapper is
/// unreachable — virtually every modern NFS server still listens here
/// even when rpcbind/portmapper (port 111) is firewalled off. mountd has
/// no equivalent universal default (it's assigned dynamically), so no
/// fallback is attempted for it; callers must supply `mount_port` in that
/// case.
const NFS_WELL_KNOWN_PORT: u16 = 2049;

#[derive(Debug, Error)]
pub enum NfsError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Network error: {0}")]
    Io(String),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("Mount failed: {0}")]
    MountFailed(String),
    #[error("NFS error: {0}")]
    NfsStatus(String),
    #[error("Protocol decode error: {0}")]
    Decode(String),
}

impl Serialize for NfsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for NfsError {
    fn from(e: std::io::Error) -> Self {
        NfsError::Io(e.to_string())
    }
}

fn mount_status_message(code: u32) -> String {
    let text = match code {
        1 => "not owner",
        2 => "no such file or directory",
        5 => "I/O error",
        13 => "permission denied",
        20 => "not a directory",
        22 => "invalid argument",
        63 => "name too long",
        10004 => "operation not supported",
        10006 => "server fault",
        _ => "unknown",
    };
    format!("{text} (status {code})")
}

fn nfs_status_message(code: u32) -> String {
    let text = match code {
        1 => "not owner",
        2 => "no such file or directory",
        5 => "I/O error",
        13 => "permission denied",
        20 => "not a directory",
        21 => "is a directory",
        22 => "invalid argument",
        28 => "no space left on device",
        63 => "name too long",
        70 => "stale file handle",
        10004 => "operation not supported",
        _ => "unknown",
    };
    format!("{text} (status {code})")
}

// ── XDR primitives (RFC 4506) ───────────────────────────────────────────

struct XdrWriter {
    buf: Vec<u8>,
}

impl XdrWriter {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    /// Fixed-size opaque: raw bytes, no length prefix (the length is
    /// implied by the XDR type declaration, e.g. cookieverf3[8]).
    fn opaque_fixed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
    /// Variable-length opaque: u32 length prefix + data, padded to a
    /// 4-byte boundary.
    fn opaque_var(&mut self, data: &[u8]) {
        self.u32(data.len() as u32);
        self.buf.extend_from_slice(data);
        let pad = (4 - (data.len() % 4)) % 4;
        self.buf.extend(std::iter::repeat_n(0u8, pad));
    }
    fn string(&mut self, s: &str) {
        self.opaque_var(s.as_bytes());
    }
    fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

struct XdrReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> XdrReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn u32(&mut self) -> Result<u32, NfsError> {
        if self.pos + 4 > self.buf.len() {
            return Err(NfsError::Decode("truncated u32".into()));
        }
        let v = u32::from_be_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }
    fn u64(&mut self) -> Result<u64, NfsError> {
        if self.pos + 8 > self.buf.len() {
            return Err(NfsError::Decode("truncated u64".into()));
        }
        let v = u64::from_be_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }
    fn bool(&mut self) -> Result<bool, NfsError> {
        Ok(self.u32()? != 0)
    }
    fn opaque_fixed(&mut self, n: usize) -> Result<Vec<u8>, NfsError> {
        if self.pos + n > self.buf.len() {
            return Err(NfsError::Decode("truncated fixed opaque".into()));
        }
        let data = self.buf[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(data)
    }
    fn opaque_var(&mut self) -> Result<Vec<u8>, NfsError> {
        let len = self.u32()? as usize;
        if self.pos + len > self.buf.len() {
            return Err(NfsError::Decode("truncated opaque".into()));
        }
        let data = self.buf[self.pos..self.pos + len].to_vec();
        self.pos += len;
        let pad = (4 - (len % 4)) % 4;
        if self.pos + pad > self.buf.len() {
            return Err(NfsError::Decode("truncated opaque padding".into()));
        }
        self.pos += pad;
        Ok(data)
    }
    fn string(&mut self) -> Result<String, NfsError> {
        let bytes = self.opaque_var()?;
        String::from_utf8(bytes).map_err(|e| NfsError::Decode(e.to_string()))
    }
    fn remaining(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }
}

// ── ONC RPC framing (RFC 1831) ──────────────────────────────────────────

#[derive(Debug, Clone)]
enum NfsAuth {
    None,
    Sys { uid: u32, gid: u32 },
}

fn encode_cred(auth: &NfsAuth) -> Vec<u8> {
    let mut w = XdrWriter::new();
    match auth {
        NfsAuth::None => {
            w.u32(0); // AUTH_NONE
            w.opaque_var(&[]);
        }
        NfsAuth::Sys { uid, gid } => {
            let mut body = XdrWriter::new();
            body.u32(0); // stamp
            body.string("crossterm"); // machinename
            body.u32(*uid);
            body.u32(*gid);
            body.u32(0); // aux gids<16>, count = 0
            w.u32(1); // AUTH_SYS
            w.opaque_var(&body.into_bytes());
        }
    }
    w.into_bytes()
}

fn build_rpc_call(xid: u32, prog: u32, vers: u32, proc_: u32, auth: &NfsAuth, args: &[u8]) -> Vec<u8> {
    let mut w = XdrWriter::new();
    w.u32(xid);
    w.u32(0); // msg_type = CALL
    w.u32(2); // rpcvers
    w.u32(prog);
    w.u32(vers);
    w.u32(proc_);
    w.opaque_fixed(&encode_cred(auth)); // cred (already length-framed)
    w.u32(0); // verf flavor = AUTH_NONE
    w.opaque_var(&[]); // verf body
    w.opaque_fixed(args);
    w.into_bytes()
}

/// Strips the rpc_msg reply header and returns the procedure-specific
/// result payload, or an error if the call was rejected/mismatched.
fn parse_rpc_reply(data: &[u8], expected_xid: u32) -> Result<Vec<u8>, NfsError> {
    let mut r = XdrReader::new(data);
    let xid = r.u32()?;
    if xid != expected_xid {
        return Err(NfsError::Rpc(format!("xid mismatch: expected {expected_xid}, got {xid}")));
    }
    let msg_type = r.u32()?;
    if msg_type != 1 {
        return Err(NfsError::Rpc("expected a REPLY message".into()));
    }
    let reply_stat = r.u32()?;
    if reply_stat != 0 {
        return Err(NfsError::Rpc(format!("RPC call denied (reply_stat={reply_stat})")));
    }
    let _verf_flavor = r.u32()?;
    let _verf_body = r.opaque_var()?;
    let accept_stat = r.u32()?;
    match accept_stat {
        0 => Ok(r.remaining().to_vec()),
        2 => {
            let low = r.u32()?;
            let high = r.u32()?;
            Err(NfsError::Rpc(format!("server only supports versions {low}-{high} of this program")))
        }
        n => Err(NfsError::Rpc(format!("RPC call not accepted (accept_stat={n})"))),
    }
}

async fn send_rpc(stream: &mut TcpStream, msg: &[u8]) -> Result<(), NfsError> {
    // TCP record marking: one fragment, high bit set to mark it "last".
    let marker = 0x8000_0000u32 | (msg.len() as u32);
    stream.write_all(&marker.to_be_bytes()).await?;
    stream.write_all(msg).await?;
    stream.flush().await?;
    Ok(())
}

async fn recv_rpc(stream: &mut TcpStream) -> Result<Vec<u8>, NfsError> {
    let mut out = Vec::new();
    loop {
        let mut marker_buf = [0u8; 4];
        stream.read_exact(&mut marker_buf).await?;
        let marker = u32::from_be_bytes(marker_buf);
        let last = marker & 0x8000_0000 != 0;
        let len = (marker & 0x7fff_ffff) as usize;
        let mut frag = vec![0u8; len];
        stream.read_exact(&mut frag).await?;
        out.extend_from_slice(&frag);
        if last {
            break;
        }
    }
    Ok(out)
}

async fn rpc_call(
    stream: &mut TcpStream,
    xid_counter: &AtomicU32,
    prog: u32,
    vers: u32,
    proc_: u32,
    auth: &NfsAuth,
    args: &[u8],
) -> Result<Vec<u8>, NfsError> {
    let xid = xid_counter.fetch_add(1, Ordering::SeqCst);
    let msg = build_rpc_call(xid, prog, vers, proc_, auth, args);
    send_rpc(stream, &msg).await?;
    let reply = recv_rpc(stream).await?;
    parse_rpc_reply(&reply, xid)
}

// ── Portmapper (RFC 1833) ───────────────────────────────────────────────

async fn pmap_getport(host: &str, prog: u32, vers: u32) -> Result<u16, NfsError> {
    let mut stream = TcpStream::connect((host, 111)).await?;
    let mut args = XdrWriter::new();
    args.u32(prog);
    args.u32(vers);
    args.u32(IPPROTO_TCP);
    args.u32(0); // port field of the request is ignored by the server
    let xid_counter = AtomicU32::new(1);
    let result = rpc_call(&mut stream, &xid_counter, PMAP_PROG, PMAP_VERS, PMAP_PROC_GETPORT, &NfsAuth::None, &args.into_bytes()).await?;
    let mut r = XdrReader::new(&result);
    let port = r.u32()?;
    if port == 0 {
        return Err(NfsError::Rpc(format!("portmapper has no registration for program {prog} version {vers}")));
    }
    Ok(port as u16)
}

// ── MOUNT protocol (RFC 1813 Appendix I) ────────────────────────────────

async fn mount_mnt(host: &str, port: u16, dirpath: &str, auth: &NfsAuth) -> Result<Vec<u8>, NfsError> {
    let mut stream = TcpStream::connect((host, port)).await?;
    let mut args = XdrWriter::new();
    args.string(dirpath);
    let xid_counter = AtomicU32::new(1);
    let result = rpc_call(&mut stream, &xid_counter, MOUNT_PROG, MOUNT_VERS, MOUNTPROC3_MNT, auth, &args.into_bytes()).await?;
    let mut r = XdrReader::new(&result);
    let status = r.u32()?;
    if status != 0 {
        return Err(NfsError::MountFailed(mount_status_message(status)));
    }
    r.opaque_var() // fhandle3
}

// ── NFSv3 attribute decoding (RFC 1813) ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NfsFileType {
    Regular,
    Directory,
    BlockDevice,
    CharDevice,
    Symlink,
    Socket,
    Fifo,
    Unknown,
}

impl From<u32> for NfsFileType {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Regular,
            2 => Self::Directory,
            3 => Self::BlockDevice,
            4 => Self::CharDevice,
            5 => Self::Symlink,
            6 => Self::Socket,
            7 => Self::Fifo,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
struct NfsAttr {
    file_type: NfsFileType,
    mode: u32,
    size: u64,
}

fn decode_fattr3(r: &mut XdrReader) -> Result<NfsAttr, NfsError> {
    let ftype = r.u32()?;
    let mode = r.u32()?;
    let _nlink = r.u32()?;
    let _uid = r.u32()?;
    let _gid = r.u32()?;
    let size = r.u64()?;
    let _used = r.u64()?;
    let _rdev1 = r.u32()?;
    let _rdev2 = r.u32()?;
    let _fsid = r.u64()?;
    let _fileid = r.u64()?;
    let _atime = (r.u32()?, r.u32()?);
    let _mtime = (r.u32()?, r.u32()?);
    let _ctime = (r.u32()?, r.u32()?);
    Ok(NfsAttr { file_type: ftype.into(), mode, size })
}

fn decode_post_op_attr(r: &mut XdrReader) -> Result<Option<NfsAttr>, NfsError> {
    if r.bool()? {
        Ok(Some(decode_fattr3(r)?))
    } else {
        Ok(None)
    }
}

fn decode_post_op_fh3(r: &mut XdrReader) -> Result<Option<Vec<u8>>, NfsError> {
    if r.bool()? {
        Ok(Some(r.opaque_var()?))
    } else {
        Ok(None)
    }
}

// ── LOOKUP (NFSPROC3_LOOKUP) ────────────────────────────────────────────

fn decode_lookup3resok(data: &[u8]) -> Result<(Vec<u8>, Option<NfsAttr>), NfsError> {
    let mut r = XdrReader::new(data);
    let fh = r.opaque_var()?;
    let attr = decode_post_op_attr(&mut r)?;
    Ok((fh, attr))
}

async fn nfs_lookup(session: &NfsSession, dir_fh: &[u8], name: &str) -> Result<(Vec<u8>, Option<NfsAttr>), NfsError> {
    let mut args = XdrWriter::new();
    args.opaque_var(dir_fh);
    args.string(name);
    let result = {
        let mut stream = session.stream.lock().await;
        rpc_call(&mut stream, &session.xid, NFS_PROG, NFS_VERS, NFSPROC3_LOOKUP, &session.auth, &args.into_bytes()).await?
    };
    let mut r = XdrReader::new(&result);
    let status = r.u32()?;
    if status != 0 {
        return Err(NfsError::NfsStatus(nfs_status_message(status)));
    }
    decode_lookup3resok(r.remaining())
}

async fn resolve_path(session: &NfsSession, path: &str) -> Result<Vec<u8>, NfsError> {
    let mut fh = session.root_fh.clone();
    for component in path.split('/').filter(|s| !s.is_empty()) {
        let (child_fh, _attr) = nfs_lookup(session, &fh, component).await?;
        fh = child_fh;
    }
    Ok(fh)
}

// ── READDIRPLUS (NFSPROC3_READDIRPLUS) ──────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NfsEntry {
    pub name: String,
    pub file_type: NfsFileType,
    pub size: u64,
    pub mode: u32,
}

struct ReaddirplusPage {
    cookieverf: [u8; 8],
    entries: Vec<(String, u64, Option<NfsAttr>)>,
    eof: bool,
}

fn decode_readdirplus_resok(data: &[u8]) -> Result<ReaddirplusPage, NfsError> {
    let mut r = XdrReader::new(data);
    let _dir_attr = decode_post_op_attr(&mut r)?;
    let verf = r.opaque_fixed(8)?;
    let mut cookieverf = [0u8; 8];
    cookieverf.copy_from_slice(&verf);

    let mut entries = Vec::new();
    let eof;
    loop {
        let has_entry = r.bool()?;
        if !has_entry {
            eof = r.bool()?;
            break;
        }
        let _fileid = r.u64()?;
        let name = r.string()?;
        let cookie = r.u64()?;
        let attr = decode_post_op_attr(&mut r)?;
        let _fh = decode_post_op_fh3(&mut r)?;
        entries.push((name, cookie, attr));
    }
    Ok(ReaddirplusPage { cookieverf, entries, eof })
}

async fn nfs_readdirplus(session: &NfsSession, dir_fh: &[u8]) -> Result<Vec<NfsEntry>, NfsError> {
    let mut out = Vec::new();
    let mut cookie: u64 = 0;
    let mut cookieverf = [0u8; 8];
    loop {
        let mut args = XdrWriter::new();
        args.opaque_var(dir_fh);
        args.u64(cookie);
        args.opaque_fixed(&cookieverf);
        args.u32(8192); // dircount
        args.u32(32768); // maxcount
        let result = {
            let mut stream = session.stream.lock().await;
            rpc_call(&mut stream, &session.xid, NFS_PROG, NFS_VERS, NFSPROC3_READDIRPLUS, &session.auth, &args.into_bytes()).await?
        };
        let mut r = XdrReader::new(&result);
        let status = r.u32()?;
        if status != 0 {
            return Err(NfsError::NfsStatus(nfs_status_message(status)));
        }
        let page = decode_readdirplus_resok(r.remaining())?;
        cookieverf = page.cookieverf;
        let eof = page.eof;
        for (name, entry_cookie, attr) in page.entries {
            cookie = entry_cookie;
            if name == "." || name == ".." {
                continue;
            }
            out.push(NfsEntry {
                name,
                file_type: attr.as_ref().map(|a| a.file_type).unwrap_or(NfsFileType::Unknown),
                size: attr.as_ref().map(|a| a.size).unwrap_or(0),
                mode: attr.as_ref().map(|a| a.mode).unwrap_or(0),
            });
        }
        if eof {
            break;
        }
    }
    Ok(out)
}

// ── READ (NFSPROC3_READ) ────────────────────────────────────────────────

fn decode_read3resok(data: &[u8]) -> Result<Vec<u8>, NfsError> {
    let mut r = XdrReader::new(data);
    let _attr = decode_post_op_attr(&mut r)?;
    let _count = r.u32()?;
    let _eof = r.bool()?;
    r.opaque_var()
}

async fn nfs_read_fh(session: &NfsSession, fh: &[u8], offset: u64, count: u32) -> Result<Vec<u8>, NfsError> {
    let mut args = XdrWriter::new();
    args.opaque_var(fh);
    args.u64(offset);
    args.u32(count);
    let result = {
        let mut stream = session.stream.lock().await;
        rpc_call(&mut stream, &session.xid, NFS_PROG, NFS_VERS, NFSPROC3_READ, &session.auth, &args.into_bytes()).await?
    };
    let mut r = XdrReader::new(&result);
    let status = r.u32()?;
    if status != 0 {
        return Err(NfsError::NfsStatus(nfs_status_message(status)));
    }
    decode_read3resok(r.remaining())
}

// ── Session state & Tauri commands ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct NfsConfig {
    pub host: String,
    pub export_path: String,
    /// Both must be set together to use AUTH_SYS; otherwise AUTH_NONE is
    /// used (typically mapped to `nobody` by the server's root-squash
    /// policy — fine for world-readable exports, not for private ones).
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    /// Override the auto-discovered (via portmapper) mountd/nfsd ports,
    /// for servers where port 111 is firewalled off.
    pub mount_port: Option<u16>,
    pub nfs_port: Option<u16>,
}

struct NfsSession {
    root_fh: Vec<u8>,
    auth: NfsAuth,
    stream: TokioMutex<TcpStream>,
    xid: AtomicU32,
}

pub struct NfsState {
    sessions: Mutex<HashMap<String, Arc<NfsSession>>>,
}

impl NfsState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn nfs_connect(config: NfsConfig, state: tauri::State<'_, NfsState>) -> Result<String, NfsError> {
    let auth = match (config.uid, config.gid) {
        (Some(uid), Some(gid)) => NfsAuth::Sys { uid, gid },
        _ => NfsAuth::None,
    };

    let mount_port = match config.mount_port {
        Some(p) => p,
        None => pmap_getport(&config.host, MOUNT_PROG, MOUNT_VERS).await?,
    };
    let root_fh = mount_mnt(&config.host, mount_port, &config.export_path, &auth).await?;

    let nfs_port = match config.nfs_port {
        Some(p) => p,
        None => match pmap_getport(&config.host, NFS_PROG, NFS_VERS).await {
            Ok(p) => p,
            Err(_) => NFS_WELL_KNOWN_PORT,
        },
    };
    let stream = TcpStream::connect((config.host.as_str(), nfs_port)).await?;

    let id = Uuid::new_v4().to_string();
    let session = Arc::new(NfsSession {
        root_fh,
        auth,
        stream: TokioMutex::new(stream),
        xid: AtomicU32::new(1),
    });
    state.sessions.lock().unwrap().insert(id.clone(), session);
    Ok(id)
}

#[tauri::command]
pub async fn nfs_list(id: String, path: String, state: tauri::State<'_, NfsState>) -> Result<Vec<NfsEntry>, NfsError> {
    let session = state.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| NfsError::NotFound(id.clone()))?;
    let dir_fh = resolve_path(&session, &path).await?;
    nfs_readdirplus(&session, &dir_fh).await
}

#[tauri::command]
pub async fn nfs_read(
    id: String,
    path: String,
    offset: u64,
    count: u32,
    state: tauri::State<'_, NfsState>,
) -> Result<Vec<u8>, NfsError> {
    let session = state.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| NfsError::NotFound(id.clone()))?;
    let file_fh = resolve_path(&session, &path).await?;
    nfs_read_fh(&session, &file_fh, offset, count).await
}

#[tauri::command]
pub fn nfs_disconnect(id: String, state: tauri::State<'_, NfsState>) -> Result<(), NfsError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| NfsError::NotFound(id))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdr_roundtrip_scalars() {
        let mut w = XdrWriter::new();
        w.u32(0xDEAD_BEEF);
        w.u64(0x0102_0304_0506_0708);
        let bytes = w.into_bytes();
        let mut r = XdrReader::new(&bytes);
        assert_eq!(r.u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.u64().unwrap(), 0x0102_0304_0506_0708);
    }

    #[test]
    fn xdr_opaque_var_pads_to_four_bytes() {
        let mut w = XdrWriter::new();
        w.opaque_var(b"abc"); // 3 bytes -> 1 byte pad
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 4 + 4); // len prefix + 3 data + 1 pad
        let mut r = XdrReader::new(&bytes);
        assert_eq!(r.opaque_var().unwrap(), b"abc");
        assert_eq!(r.pos, bytes.len());
    }

    #[test]
    fn xdr_string_roundtrip() {
        let mut w = XdrWriter::new();
        w.string("export/data");
        let bytes = w.into_bytes();
        let mut r = XdrReader::new(&bytes);
        assert_eq!(r.string().unwrap(), "export/data");
    }

    #[test]
    fn encode_cred_auth_none_is_flavor_zero_empty_body() {
        let cred = encode_cred(&NfsAuth::None);
        // flavor (u32=0) + length prefix (u32=0), no body/padding.
        assert_eq!(cred, vec![0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn encode_cred_auth_sys_round_trips_uid_gid() {
        let cred = encode_cred(&NfsAuth::Sys { uid: 1000, gid: 1000 });
        let mut r = XdrReader::new(&cred);
        let flavor = r.u32().unwrap();
        assert_eq!(flavor, 1); // AUTH_SYS
        let body = r.opaque_var().unwrap();
        let mut br = XdrReader::new(&body);
        let _stamp = br.u32().unwrap();
        let machine = br.string().unwrap();
        assert_eq!(machine, "crossterm");
        assert_eq!(br.u32().unwrap(), 1000); // uid
        assert_eq!(br.u32().unwrap(), 1000); // gid
        assert_eq!(br.u32().unwrap(), 0); // aux gids count
    }

    #[test]
    fn build_rpc_call_matches_expected_header_layout() {
        let msg = build_rpc_call(42, 100005, 3, 1, &NfsAuth::None, &[9, 9, 9, 9]);
        let mut r = XdrReader::new(&msg);
        assert_eq!(r.u32().unwrap(), 42); // xid
        assert_eq!(r.u32().unwrap(), 0); // CALL
        assert_eq!(r.u32().unwrap(), 2); // rpcvers
        assert_eq!(r.u32().unwrap(), 100005); // prog
        assert_eq!(r.u32().unwrap(), 3); // vers
        assert_eq!(r.u32().unwrap(), 1); // proc
        assert_eq!(r.u32().unwrap(), 0); // cred flavor = AUTH_NONE
        assert_eq!(r.opaque_var().unwrap(), Vec::<u8>::new()); // cred body
        assert_eq!(r.u32().unwrap(), 0); // verf flavor = AUTH_NONE
        assert_eq!(r.opaque_var().unwrap(), Vec::<u8>::new()); // verf body
        assert_eq!(r.remaining(), &[9, 9, 9, 9]);
    }

    fn synthetic_reply(xid: u32, payload: &[u8]) -> Vec<u8> {
        let mut w = XdrWriter::new();
        w.u32(xid);
        w.u32(1); // REPLY
        w.u32(0); // MSG_ACCEPTED
        w.u32(0); // verf flavor AUTH_NONE
        w.opaque_var(&[]); // verf body
        w.u32(0); // accept_stat SUCCESS
        w.opaque_fixed(payload);
        w.into_bytes()
    }

    #[test]
    fn parse_rpc_reply_extracts_payload_on_success() {
        let reply = synthetic_reply(7, &[1, 2, 3, 4]);
        let payload = parse_rpc_reply(&reply, 7).unwrap();
        assert_eq!(payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn parse_rpc_reply_rejects_xid_mismatch() {
        let reply = synthetic_reply(7, &[]);
        let err = parse_rpc_reply(&reply, 8).unwrap_err();
        assert!(matches!(err, NfsError::Rpc(_)));
    }

    #[test]
    fn parse_rpc_reply_rejects_msg_denied() {
        let mut w = XdrWriter::new();
        w.u32(1);
        w.u32(1); // REPLY
        w.u32(1); // MSG_DENIED
        let reply = w.into_bytes();
        let err = parse_rpc_reply(&reply, 1).unwrap_err();
        assert!(matches!(err, NfsError::Rpc(_)));
    }

    #[test]
    fn parse_rpc_reply_reports_prog_mismatch_versions() {
        let mut w = XdrWriter::new();
        w.u32(1);
        w.u32(1); // REPLY
        w.u32(0); // MSG_ACCEPTED
        w.u32(0);
        w.opaque_var(&[]); // verf
        w.u32(2); // accept_stat PROG_MISMATCH
        w.u32(2); // low
        w.u32(4); // high
        let reply = w.into_bytes();
        let err = parse_rpc_reply(&reply, 1).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains('2') && msg.contains('4'));
    }

    fn synthetic_fattr3(file_type: u32, mode: u32, size: u64) -> Vec<u8> {
        let mut w = XdrWriter::new();
        w.u32(file_type);
        w.u32(mode);
        w.u32(1); // nlink
        w.u32(0); // uid
        w.u32(0); // gid
        w.u64(size);
        w.u64(size); // used
        w.u32(0);
        w.u32(0); // rdev
        w.u64(0); // fsid
        w.u64(42); // fileid
        w.u32(0);
        w.u32(0); // atime
        w.u32(0);
        w.u32(0); // mtime
        w.u32(0);
        w.u32(0); // ctime
        w.into_bytes()
    }

    #[test]
    fn decode_fattr3_reads_type_mode_size_in_order() {
        let bytes = synthetic_fattr3(2, 0o755, 4096);
        let mut r = XdrReader::new(&bytes);
        let attr = decode_fattr3(&mut r).unwrap();
        assert_eq!(attr.file_type, NfsFileType::Directory);
        assert_eq!(attr.mode, 0o755);
        assert_eq!(attr.size, 4096);
    }

    #[test]
    fn decode_post_op_attr_handles_both_cases() {
        let mut absent = XdrWriter::new();
        absent.u32(0); // FALSE
        let bytes = absent.into_bytes();
        let mut r = XdrReader::new(&bytes);
        assert!(decode_post_op_attr(&mut r).unwrap().is_none());

        let mut present = XdrWriter::new();
        present.u32(1); // TRUE
        present.opaque_fixed(&synthetic_fattr3(1, 0o644, 10));
        let bytes = present.into_bytes();
        let mut r = XdrReader::new(&bytes);
        let attr = decode_post_op_attr(&mut r).unwrap().unwrap();
        assert_eq!(attr.file_type, NfsFileType::Regular);
        assert_eq!(attr.size, 10);
    }

    #[test]
    fn decode_lookup3resok_reads_handle_and_attrs() {
        let mut w = XdrWriter::new();
        w.opaque_var(&[1, 2, 3]); // fh
        w.u32(1); // obj_attributes present
        w.opaque_fixed(&synthetic_fattr3(1, 0o644, 123));
        w.u32(0); // dir_attributes absent
        let bytes = w.into_bytes();
        let (fh, attr) = decode_lookup3resok(&bytes).unwrap();
        assert_eq!(fh, vec![1, 2, 3]);
        assert_eq!(attr.unwrap().size, 123);
    }

    fn synthetic_entryplus3(name: &str, cookie: u64, has_attr: bool) -> XdrWriter {
        let mut w = XdrWriter::new();
        w.u64(99); // fileid
        w.string(name);
        w.u64(cookie);
        if has_attr {
            w.u32(1);
            w.opaque_fixed(&synthetic_fattr3(1, 0o644, 7));
        } else {
            w.u32(0);
        }
        w.u32(0); // name_handle absent
        w
    }

    #[test]
    fn decode_readdirplus_resok_walks_linked_list_and_stops_at_eof() {
        let mut w = XdrWriter::new();
        w.u32(0); // dir_attributes absent
        w.opaque_fixed(&[1, 2, 3, 4, 5, 6, 7, 8]); // cookieverf

        // entries: [".", "file.txt"], then eof=true
        w.u32(1); // has entry
        w.opaque_fixed(&synthetic_entryplus3(".", 1, false).into_bytes());
        w.u32(1); // has next entry
        w.opaque_fixed(&synthetic_entryplus3("file.txt", 2, true).into_bytes());
        w.u32(0); // no more entries
        w.u32(1); // eof = true

        let bytes = w.into_bytes();
        let page = decode_readdirplus_resok(&bytes).unwrap();
        assert_eq!(page.cookieverf, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(page.eof);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].0, ".");
        assert_eq!(page.entries[1].0, "file.txt");
        assert_eq!(page.entries[1].2.as_ref().unwrap().size, 7);
    }

    #[test]
    fn decode_readdirplus_resok_reports_eof_false_when_more_pages_remain() {
        let mut w = XdrWriter::new();
        w.u32(0); // dir_attributes absent
        w.opaque_fixed(&[0u8; 8]);
        w.u32(0); // no entries in this page
        w.u32(0); // eof = false
        let bytes = w.into_bytes();
        let page = decode_readdirplus_resok(&bytes).unwrap();
        assert!(!page.eof);
        assert!(page.entries.is_empty());
    }

    #[test]
    fn decode_read3resok_extracts_data_after_attrs_count_eof() {
        let mut w = XdrWriter::new();
        w.u32(0); // file_attributes absent
        w.u32(5); // count
        w.u32(1); // eof = true
        w.opaque_var(b"hello");
        let bytes = w.into_bytes();
        let data = decode_read3resok(&bytes).unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn nfs_status_message_maps_common_codes() {
        assert!(nfs_status_message(2).contains("no such file"));
        assert!(nfs_status_message(13).contains("permission denied"));
        assert!(mount_status_message(20).contains("not a directory"));
    }
}

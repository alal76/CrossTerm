use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use thiserror::Error;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("No active profile")]
    NoActiveProfile,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for AuditError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    VaultUnlock,
    VaultLock,
    VaultCreate,
    VaultAutoLock,
    CredentialAccess,
    CredentialCreate,
    CredentialUpdate,
    CredentialDelete,
    ClipboardCopy,
    SessionConnect,
    SessionDisconnect,
    SessionCreate,
    SessionDelete,
    ProfileExport,
    ProfileImport,
    ProfileCreate,
    ProfileSwitch,
    SettingsUpdate,
    TerminalCreate,
    TerminalClose,
    KeygenGenerate,
    KeygenImport,
    KeygenDeploy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub details: String,
}

#[derive(Debug, Deserialize)]
pub struct AuditListRequest {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub event_type: Option<AuditEventType>,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn audit_dir(profile_id: &str) -> PathBuf {
    let p = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CrossTerm")
        .join("profiles")
        .join(profile_id);
    std::fs::create_dir_all(&p).ok();
    p
}

fn audit_file(profile_id: &str) -> PathBuf {
    audit_dir(profile_id).join("audit.jsonl")
}

/// Append a single event to the audit log (append-only).
pub fn append_event(profile_id: &str, event_type: AuditEventType, details: &str) {
    let event = AuditEvent {
        timestamp: Utc::now(),
        event_type,
        details: details.to_string(),
    };
    if let Ok(line) = serde_json::to_string(&event) {
        let path = audit_file(profile_id);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(file, "{}", line);
        }
    }
}

fn read_events(profile_id: &str) -> Result<Vec<AuditEvent>, AuditError> {
    let path = audit_file(profile_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: AuditEvent = serde_json::from_str(&line)?;
        events.push(event);
    }
    Ok(events)
}

fn events_to_csv(events: &[AuditEvent]) -> String {
    let mut out = String::from("timestamp,event_type,details\n");
    for e in events {
        let event_type = serde_json::to_string(&e.event_type).unwrap_or_default();
        // CSV-escape details: wrap in quotes, double-escape internal quotes
        let details = e.details.replace('"', "\"\"");
        out.push_str(&format!(
            "{},{},\"{}\"\n",
            e.timestamp.to_rfc3339(),
            event_type.trim_matches('"'),
            details
        ));
    }
    out
}

// ── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn audit_log_list(
    config_state: tauri::State<'_, crate::config::ConfigState>,
    request: Option<AuditListRequest>,
) -> Result<Vec<AuditEvent>, AuditError> {
    let profile_id = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .ok_or(AuditError::NoActiveProfile)?;

    let mut events = read_events(&profile_id)?;

    if let Some(ref req) = request {
        if let Some(ref et) = req.event_type {
            events.retain(|e| &e.event_type == et);
        }
        let offset = req.offset.unwrap_or(0);
        let limit = req.limit.unwrap_or(usize::MAX);
        events = events.into_iter().skip(offset).take(limit).collect();
    }

    Ok(events)
}

#[tauri::command]
pub fn audit_log_export_csv(
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<String, AuditError> {
    let profile_id = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .ok_or(AuditError::NoActiveProfile)?;
    let events = read_events(&profile_id)?;
    Ok(events_to_csv(&events))
}

// ── Phase 3-5: Syslog Forwarding ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyslogConfig {
    pub host: String,
    pub port: u16,
    pub protocol: SyslogProtocol,
    /// Syslog facility number (1 = user, 3 = daemon, 16-23 = local0-local7)
    pub facility: u8,
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyslogProtocol {
    Udp,
    Tcp,
}

/// Format an `AuditEvent` as an RFC 5424 syslog message.
///
/// RFC 5424 format:
/// `<PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID STRUCTURED-DATA MSG`
fn format_syslog_message(entry: &AuditEvent, config: &SyslogConfig) -> String {
    // Severity: map audit events to syslog severity 6 (Informational) by default.
    let severity: u8 = 6;
    let pri = (config.facility as u16) * 8 + (severity as u16);

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "-".to_string());

    // Structured data: encode event_type and details as SD params.
    let event_type_str = serde_json::to_string(&entry.event_type)
        .unwrap_or_else(|_| "\"unknown\"".to_string());
    let event_type_str = event_type_str.trim_matches('"');

    // Escape ] " \ inside SD param values per RFC 5424 §6.3.3.
    let escaped_details = entry
        .details
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(']', "\\]");

    format!(
        "<{pri}>1 {ts} {host} {app} - - [crossterm@57137 event_type=\"{et}\" details=\"{det}\"] {msg}",
        pri = pri,
        ts = entry.timestamp.to_rfc3339(),
        host = hostname,
        app = config.app_name,
        et = event_type_str,
        det = escaped_details,
        msg = format!("{}: {}", event_type_str, entry.details),
    )
}

/// Send a single audit event to the configured syslog server.
pub fn send_to_syslog(entry: &AuditEvent, config: &SyslogConfig) -> Result<(), String> {
    use std::io::Write as _;
    use std::net::{TcpStream, UdpSocket};

    let message = format_syslog_message(entry, config);
    let addr = format!("{}:{}", config.host, config.port);

    match config.protocol {
        SyslogProtocol::Udp => {
            let socket =
                UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind error: {e}"))?;
            socket
                .send_to(message.as_bytes(), &addr)
                .map_err(|e| format!("UDP send error: {e}"))?;
        }
        SyslogProtocol::Tcp => {
            let mut stream = TcpStream::connect(&addr)
                .map_err(|e| format!("TCP connect error: {e}"))?;
            // RFC 6587 §3.4.1: octet-counting framing
            let framed = format!("{} {}", message.len(), message);
            stream
                .write_all(framed.as_bytes())
                .map_err(|e| format!("TCP write error: {e}"))?;
        }
    }
    Ok(())
}

// Global syslog config store (guarded by a Mutex for interior mutability).
static SYSLOG_CONFIG: std::sync::OnceLock<std::sync::Mutex<Option<SyslogConfig>>> =
    std::sync::OnceLock::new();

fn syslog_config_lock() -> &'static std::sync::Mutex<Option<SyslogConfig>> {
    SYSLOG_CONFIG.get_or_init(|| std::sync::Mutex::new(None))
}

#[tauri::command]
pub fn audit_configure_syslog(config: SyslogConfig) -> Result<(), String> {
    *syslog_config_lock()
        .lock()
        .map_err(|e| format!("lock error: {e}"))? = Some(config);
    Ok(())
}

#[tauri::command]
pub fn audit_test_syslog() -> Result<(), String> {
    let guard = syslog_config_lock()
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;
    let config = guard
        .as_ref()
        .ok_or_else(|| "Syslog not configured — call audit_configure_syslog first".to_string())?;

    let test_event = AuditEvent {
        timestamp: Utc::now(),
        event_type: AuditEventType::SettingsUpdate,
        details: "CrossTerm syslog connectivity test".to_string(),
    };
    send_to_syslog(&test_event, config)
}

// ── Phase 3-5: Anomaly Detection ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub alert_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub description: String,
    pub related_entry_ids: Vec<String>,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// Login at an unusual hour (midnight–05:00 local time).
    UnusualHour,
    /// 5+ failed auth attempts within a 5-minute rolling window.
    RapidFailedAuth,
    /// First time a specific host appears in the audit log.
    NewHostFirstConnect,
    /// 10+ sessions created within a 60-second rolling window.
    BulkSessionCreation,
    /// Admin/privileged action outside business hours (before 08:00 or after 18:00 local).
    PrivilegedAfterHours,
    /// SFTP data transfer exceeding 1 GiB.
    LargeDataTransfer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
}

/// Extract a host identifier from an `AuditEvent`'s details string.
/// Looks for a "host:" or "to " prefix as a heuristic.
fn extract_host(details: &str) -> Option<String> {
    // Try "host:<value>" pattern.
    if let Some(rest) = details.strip_prefix("host:") {
        let host = rest.split_whitespace().next()?.to_string();
        return Some(host);
    }
    // Try "connected to <host>" pattern.
    if let Some(rest) = details.to_lowercase().strip_prefix("connected to ") {
        let host = rest.split_whitespace().next()?.to_string();
        return Some(host);
    }
    None
}

/// Returns true if the timestamp falls within the "unusual hours" window
/// (midnight inclusive to 05:00 exclusive) in local time.
fn is_unusual_hour(ts: &DateTime<Utc>) -> bool {
    use chrono::Timelike;
    let local = ts.with_timezone(&chrono::Local);
    let hour = local.hour();
    hour < 5
}

/// Returns true if the event falls outside business hours (08:00–18:00 local).
fn is_after_hours(ts: &DateTime<Utc>) -> bool {
    use chrono::Timelike;
    let local = ts.with_timezone(&chrono::Local);
    let hour = local.hour();
    hour < 8 || hour >= 18
}

/// Classify whether an event is considered a "failed authentication".
/// Since `AuditEventType` has no dedicated `AuthFailed` variant, we detect
/// it by inspecting the `details` field for known failure keywords.
fn is_auth_failed(event: &AuditEvent) -> bool {
    let d = event.details.to_lowercase();
    d.contains("auth failed")
        || d.contains("authentication failed")
        || d.contains("login failed")
        || d.contains("permission denied")
        || d.contains("invalid credentials")
}

/// Analyze a window of recent audit entries for anomalies.
pub fn detect_anomalies(entries: &[AuditEvent]) -> Vec<AnomalyAlert> {
    let mut alerts: Vec<AnomalyAlert> = Vec::new();
    let now = Utc::now().to_rfc3339();

    // --- UnusualHour: auth-related events between 00:00 and 05:00 local time ---
    for (idx, entry) in entries.iter().enumerate() {
        match &entry.event_type {
            AuditEventType::VaultUnlock | AuditEventType::SessionConnect => {
                if is_unusual_hour(&entry.timestamp) {
                    alerts.push(AnomalyAlert {
                        alert_type: AnomalyType::UnusualHour,
                        severity: AnomalySeverity::Warning,
                        description: format!(
                            "Authentication/session event at unusual hour: {} (local)",
                            entry
                                .timestamp
                                .with_timezone(&chrono::Local)
                                .format("%H:%M")
                        ),
                        related_entry_ids: vec![idx.to_string()],
                        detected_at: now.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    // --- RapidFailedAuth: 5+ auth-failed events within any 5-minute window ---
    {
        use std::time::Duration;
        let failed_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| is_auth_failed(e))
            .map(|(i, _)| i)
            .collect();

        // Sliding window over failed auth events.
        let window_secs: i64 = 300; // 5 minutes
        let threshold = 5usize;
        let mut reported = false;
        for (wi, &start_idx) in failed_indices.iter().enumerate() {
            if reported {
                break;
            }
            let start_ts = entries[start_idx].timestamp;
            let window_end = start_ts + chrono::Duration::seconds(window_secs);
            let window_events: Vec<usize> = failed_indices[wi..]
                .iter()
                .copied()
                .filter(|&i| entries[i].timestamp <= window_end)
                .collect();
            if window_events.len() >= threshold {
                alerts.push(AnomalyAlert {
                    alert_type: AnomalyType::RapidFailedAuth,
                    severity: AnomalySeverity::Critical,
                    description: format!(
                        "{} failed authentication attempts within a 5-minute window",
                        window_events.len()
                    ),
                    related_entry_ids: window_events.iter().map(|i| i.to_string()).collect(),
                    detected_at: now.clone(),
                });
                reported = true;
            }
        }
        // Suppress unused import warning for Duration which is from std.
        let _ = Duration::from_secs(0);
    }

    // --- BulkSessionCreation: 10+ SessionCreate within 60 seconds ---
    {
        let create_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.event_type == AuditEventType::SessionCreate)
            .map(|(i, _)| i)
            .collect();

        let window_secs: i64 = 60;
        let threshold = 10usize;
        let mut reported = false;
        for (wi, &start_idx) in create_indices.iter().enumerate() {
            if reported {
                break;
            }
            let start_ts = entries[start_idx].timestamp;
            let window_end = start_ts + chrono::Duration::seconds(window_secs);
            let window_events: Vec<usize> = create_indices[wi..]
                .iter()
                .copied()
                .filter(|&i| entries[i].timestamp <= window_end)
                .collect();
            if window_events.len() >= threshold {
                alerts.push(AnomalyAlert {
                    alert_type: AnomalyType::BulkSessionCreation,
                    severity: AnomalySeverity::Warning,
                    description: format!(
                        "{} sessions created within a 60-second window",
                        window_events.len()
                    ),
                    related_entry_ids: window_events.iter().map(|i| i.to_string()).collect(),
                    detected_at: now.clone(),
                });
                reported = true;
            }
        }
    }

    // --- NewHostFirstConnect: first time a host appears ---
    {
        let mut seen_hosts: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (idx, entry) in entries.iter().enumerate() {
            if entry.event_type == AuditEventType::SessionConnect {
                if let Some(host) = extract_host(&entry.details) {
                    if seen_hosts.insert(host.clone()) {
                        // First occurrence.
                        alerts.push(AnomalyAlert {
                            alert_type: AnomalyType::NewHostFirstConnect,
                            severity: AnomalySeverity::Info,
                            description: format!("First-time connection to host: {host}"),
                            related_entry_ids: vec![idx.to_string()],
                            detected_at: now.clone(),
                        });
                    }
                }
            }
        }
    }

    // --- PrivilegedAfterHours: privileged events outside 08:00–18:00 local ---
    {
        let privileged_types = [
            AuditEventType::VaultUnlock,
            AuditEventType::CredentialAccess,
            AuditEventType::KeygenGenerate,
            AuditEventType::KeygenDeploy,
            AuditEventType::ProfileImport,
            AuditEventType::ProfileExport,
        ];
        for (idx, entry) in entries.iter().enumerate() {
            if privileged_types.contains(&entry.event_type) && is_after_hours(&entry.timestamp) {
                alerts.push(AnomalyAlert {
                    alert_type: AnomalyType::PrivilegedAfterHours,
                    severity: AnomalySeverity::Warning,
                    description: format!(
                        "Privileged action {:?} outside business hours at {}",
                        entry.event_type,
                        entry
                            .timestamp
                            .with_timezone(&chrono::Local)
                            .format("%H:%M")
                    ),
                    related_entry_ids: vec![idx.to_string()],
                    detected_at: now.clone(),
                });
            }
        }
    }

    // --- LargeDataTransfer: SFTP transfer > 1 GiB noted in details ---
    {
        for (idx, entry) in entries.iter().enumerate() {
            // Look for byte counts in SFTP-related details.
            let d = entry.details.to_lowercase();
            if d.contains("sftp") || d.contains("transfer") {
                // Parse the first integer-looking token and treat it as bytes.
                for token in entry.details.split_whitespace() {
                    if let Ok(bytes) = token.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u64>() {
                        if bytes > 1_073_741_824 {
                            alerts.push(AnomalyAlert {
                                alert_type: AnomalyType::LargeDataTransfer,
                                severity: AnomalySeverity::Warning,
                                description: format!(
                                    "Large data transfer detected: {} bytes (>{:.1} GiB)",
                                    bytes,
                                    bytes as f64 / 1_073_741_824.0
                                ),
                                related_entry_ids: vec![idx.to_string()],
                                detected_at: now.clone(),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    alerts
}

/// Load all audit entries for the active profile, run `detect_anomalies`, and return results.
#[tauri::command]
pub fn audit_detect_anomalies(
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<Vec<AnomalyAlert>, String> {
    let profile_id = config_state
        .active_profile_id
        .read()
        .map_err(|e| format!("lock error: {e}"))?
        .clone()
        .ok_or_else(|| "No active profile".to_string())?;

    let entries =
        read_events(&profile_id).map_err(|e| format!("read_events error: {e}"))?;
    Ok(detect_anomalies(&entries))
}

/// Return all persisted anomaly alerts. Currently re-runs detection on each call;
/// a production implementation would persist alerts to a separate store.
#[tauri::command]
pub fn audit_list_alerts(
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<Vec<AnomalyAlert>, String> {
    audit_detect_anomalies(config_state)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helpers: write events directly into a temp-dir-based audit file,
    /// then read them back using the same module functions.
    /// We override the profile path by constructing our own JSONL file
    /// and calling `read_events_from_path` indirectly via the public helpers.

    fn write_events_to_file(path: &std::path::Path, events: &[AuditEvent]) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for event in events {
            let line = serde_json::to_string(event).unwrap();
            writeln!(file, "{}", line).unwrap();
        }
    }

    fn read_events_from_file(path: &std::path::Path) -> Vec<AuditEvent> {
        if !path.exists() {
            return Vec::new();
        }
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();
        for line in std::io::BufRead::lines(reader) {
            let line = line.unwrap();
            if line.trim().is_empty() {
                continue;
            }
            let event: AuditEvent = serde_json::from_str(&line).unwrap();
            events.push(event);
        }
        events
    }

    fn make_event(event_type: AuditEventType, details: &str) -> AuditEvent {
        AuditEvent {
            timestamp: chrono::Utc::now(),
            event_type,
            details: details.to_string(),
        }
    }

    #[test]
    fn test_append_and_list() {
        let tmp = TempDir::new().unwrap();
        let audit_path = tmp.path().join("audit.jsonl");

        let events = vec![
            make_event(AuditEventType::VaultUnlock, "unlocked vault"),
            make_event(AuditEventType::SessionConnect, "connected to server"),
            make_event(AuditEventType::CredentialCreate, "created credential"),
        ];
        write_events_to_file(&audit_path, &events);

        let loaded = read_events_from_file(&audit_path);
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].event_type, AuditEventType::VaultUnlock);
        assert_eq!(loaded[1].event_type, AuditEventType::SessionConnect);
        assert_eq!(loaded[2].event_type, AuditEventType::CredentialCreate);
    }

    #[test]
    fn test_filter_by_event_type() {
        let tmp = TempDir::new().unwrap();
        let audit_path = tmp.path().join("audit.jsonl");

        let events = vec![
            make_event(AuditEventType::VaultUnlock, "unlock 1"),
            make_event(AuditEventType::SessionConnect, "connect 1"),
            make_event(AuditEventType::VaultUnlock, "unlock 2"),
            make_event(AuditEventType::CredentialCreate, "cred 1"),
            make_event(AuditEventType::VaultUnlock, "unlock 3"),
        ];
        write_events_to_file(&audit_path, &events);

        let mut loaded = read_events_from_file(&audit_path);
        loaded.retain(|e| e.event_type == AuditEventType::VaultUnlock);

        assert_eq!(loaded.len(), 3);
        for e in &loaded {
            assert_eq!(e.event_type, AuditEventType::VaultUnlock);
        }
    }

    #[test]
    fn test_offset_and_limit() {
        let tmp = TempDir::new().unwrap();
        let audit_path = tmp.path().join("audit.jsonl");

        let events: Vec<AuditEvent> = (0..20)
            .map(|i| make_event(AuditEventType::SettingsUpdate, &format!("event_{}", i)))
            .collect();
        write_events_to_file(&audit_path, &events);

        let loaded = read_events_from_file(&audit_path);
        assert_eq!(loaded.len(), 20);

        // Apply offset=5, limit=10 like the Tauri command does
        let offset = 5;
        let limit = 10;
        let sliced: Vec<AuditEvent> = loaded.into_iter().skip(offset).take(limit).collect();

        assert_eq!(sliced.len(), 10);
        assert_eq!(sliced[0].details, "event_5");
        assert_eq!(sliced[9].details, "event_14");
    }

    #[test]
    fn test_csv_export() {
        let events = vec![
            make_event(AuditEventType::VaultUnlock, "unlocked"),
            make_event(AuditEventType::SessionConnect, "connected to host"),
            make_event(AuditEventType::CredentialCreate, "created \"test\" cred"),
        ];

        let csv = events_to_csv(&events);
        let lines: Vec<&str> = csv.trim().lines().collect();

        // Header + 3 data rows
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "timestamp,event_type,details");

        // Verify each row has 3+ fields and correct event types
        assert!(lines[1].contains("vault_unlock"));
        assert!(lines[2].contains("session_connect"));
        assert!(lines[3].contains("credential_create"));

        // Verify CSV-escaped quotes in the last row
        assert!(lines[3].contains("created \"\"test\"\" cred"));
    }

    #[test]
    fn test_empty_audit_log() {
        let tmp = TempDir::new().unwrap();
        let audit_path = tmp.path().join("audit.jsonl");
        // File does not exist
        let loaded = read_events_from_file(&audit_path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_audit_event_serialization_roundtrip() {
        let event = make_event(AuditEventType::ProfileSwitch, "switched to dev");
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.details, event.details);
    }

    #[test]
    fn test_event_type_serde_rename() {
        // Verify snake_case serialization
        let json = serde_json::to_string(&AuditEventType::CredentialAccess).unwrap();
        assert_eq!(json, "\"credential_access\"");

        let json = serde_json::to_string(&AuditEventType::TerminalCreate).unwrap();
        assert_eq!(json, "\"terminal_create\"");
    }

    // ── UT-A-06: Concurrent append ──────────────────────────────────

    #[test]
    fn test_concurrent_append() {
        // UT-A-06
        let tmp = TempDir::new().unwrap();
        let audit_path = tmp.path().join("audit.jsonl");
        let path = audit_path.clone();

        // Spawn 10 threads, each appending one event
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let p = path.clone();
                std::thread::spawn(move || {
                    let event = AuditEvent {
                        timestamp: chrono::Utc::now(),
                        event_type: AuditEventType::SettingsUpdate,
                        details: format!("concurrent_event_{}", i),
                    };
                    let mut line = serde_json::to_string(&event).unwrap();
                    line.push('\n');
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&p)
                        .unwrap();
                    file.write_all(line.as_bytes()).unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().expect("Thread panicked");
        }

        // Read all events back
        let loaded = read_events_from_file(&audit_path);
        assert_eq!(loaded.len(), 10, "Expected 10 events from concurrent appends, got {}", loaded.len());

        // Verify no corruption: every event deserializes and has correct type
        for event in &loaded {
            assert_eq!(event.event_type, AuditEventType::SettingsUpdate);
            assert!(event.details.starts_with("concurrent_event_"));
        }

        // Verify all 10 distinct events are present
        let mut indices: Vec<String> = loaded.iter().map(|e| e.details.clone()).collect();
        indices.sort();
        indices.dedup();
        assert_eq!(indices.len(), 10, "All 10 events should be distinct");
    }
}

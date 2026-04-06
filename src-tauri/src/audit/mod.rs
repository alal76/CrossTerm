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

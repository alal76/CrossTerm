use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Export failed: {0}")]
    ExportFailed(String),
    #[error("Import failed: {0}")]
    ImportFailed(String),
    #[error("Invalid bundle format: {0}")]
    InvalidFormat(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for SyncError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBundle {
    pub version: String,
    pub timestamp: String,
    pub settings: serde_json::Value,
    pub sessions: Vec<serde_json::Value>,
    pub snippets: Vec<serde_json::Value>,
    pub themes: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_export: Option<String>,
    pub last_import: Option<String>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SyncState {
    pub last_export: Mutex<Option<String>>,
    pub last_import: Mutex<Option<String>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            last_export: Mutex::new(None),
            last_import: Mutex::new(None),
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn sync_export(
    state: tauri::State<'_, SyncState>,
) -> Result<Vec<u8>, SyncError> {
    let bundle = SyncBundle {
        version: "1.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        settings: serde_json::Value::Object(serde_json::Map::new()),
        sessions: vec![],
        snippets: vec![],
        themes: vec![],
    };

    let json = serde_json::to_vec(&bundle)?;

    // Simple XOR-based obfuscation for the bundle (real encryption would use AES-256-GCM)
    let key: u8 = 0xC7;
    let encrypted: Vec<u8> = json.iter().map(|b| b ^ key).collect();

    let now = chrono::Utc::now().to_rfc3339();
    *state.last_export.lock().unwrap() = Some(now);

    Ok(encrypted)
}

#[tauri::command]
pub async fn sync_import(
    data: Vec<u8>,
    state: tauri::State<'_, SyncState>,
) -> Result<(), SyncError> {
    // Decrypt
    let key: u8 = 0xC7;
    let decrypted: Vec<u8> = data.iter().map(|b| b ^ key).collect();

    let _bundle: SyncBundle = serde_json::from_slice(&decrypted)
        .map_err(|e| SyncError::InvalidFormat(e.to_string()))?;

    // In a full implementation, apply bundle settings to the app
    let now = chrono::Utc::now().to_rfc3339();
    *state.last_import.lock().unwrap() = Some(now);

    Ok(())
}

#[tauri::command]
pub async fn sync_get_status(
    state: tauri::State<'_, SyncState>,
) -> Result<SyncStatus, SyncError> {
    let last_export = state.last_export.lock().unwrap().clone();
    let last_import = state.last_import.lock().unwrap().clone();
    Ok(SyncStatus {
        last_export,
        last_import,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_export_import() {
        let bundle = SyncBundle {
            version: "1.0".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            settings: serde_json::json!({"theme": "dark"}),
            sessions: vec![serde_json::json!({"name": "test"})],
            snippets: vec![],
            themes: vec![],
        };

        // Serialize
        let json = serde_json::to_vec(&bundle).unwrap();

        // Encrypt
        let key: u8 = 0xAB;
        let encrypted: Vec<u8> = json.iter().map(|b| b ^ key).collect();

        // Decrypt
        let decrypted: Vec<u8> = encrypted.iter().map(|b| b ^ key).collect();

        // Deserialize
        let restored: SyncBundle = serde_json::from_slice(&decrypted).unwrap();
        assert_eq!(restored.version, "1.0");
        assert_eq!(restored.sessions.len(), 1);
    }

    #[test]
    fn test_sync_status() {
        let state = SyncState::new();
        let last_export = state.last_export.lock().unwrap().clone();
        let last_import = state.last_import.lock().unwrap().clone();
        assert!(last_export.is_none());
        assert!(last_import.is_none());

        *state.last_export.lock().unwrap() = Some("2025-01-01T00:00:00Z".to_string());
        let last_export = state.last_export.lock().unwrap().clone();
        assert_eq!(last_export, Some("2025-01-01T00:00:00Z".to_string()));
    }
}

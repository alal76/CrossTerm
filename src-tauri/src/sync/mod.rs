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

// ── Phase 3: Encrypted Sync Package ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPackage {
    pub version: u32,
    pub created_at: String,
    pub profile_id: String,
    pub checksum: String,        // SHA-256 hex of the encrypted_payload bytes
    pub encrypted_payload: String, // base64-standard: AES-256-GCM(kek, json_sessions)
    pub nonce: String,           // base64-standard: 12-byte GCM nonce
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub session_id: String,
    pub local_updated_at: String,
    pub remote_updated_at: String,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeepLocal,
    KeepRemote,
    Merge,
}

// ── Phase 3 helpers ─────────────────────────────────────────────────────

/// Derive a 32-byte AES key from a base64-encoded KEK.
/// Accepts exactly 32 raw bytes (base64-encoded) as the KEK.
fn kek_to_aes_key(kek_b64: &str) -> Result<[u8; 32], String> {
    use base64::Engine as _;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(kek_b64)
        .map_err(|e| format!("kek_b64 base64 decode error: {e}"))?;
    if raw.len() != 32 {
        return Err(format!(
            "KEK must be 32 bytes, got {}",
            raw.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&raw);
    Ok(key)
}

/// SHA-256 hex digest of a byte slice.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

/// AES-256-GCM encrypt `plaintext` with a 32-byte key and 12-byte nonce.
fn aes256gcm_encrypt(key: &[u8; 32], nonce_bytes: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("AES-256-GCM encrypt error: {e}"))
}

/// AES-256-GCM decrypt `ciphertext` with a 32-byte key and 12-byte nonce.
fn aes256gcm_decrypt(key: &[u8; 32], nonce_bytes: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("AES-256-GCM decrypt error: {e}"))
}

// ── Phase 3 Tauri Commands ───────────────────────────────────────────────

/// Create an encrypted sync package from all sessions in a profile.
/// `kek_b64`: base64-standard-encoded 32-byte Key Encryption Key from the vault.
#[tauri::command]
pub fn sync_create_package(
    profile_id: String,
    kek_b64: String,
) -> Result<SyncPackage, String> {
    use base64::Engine as _;
    use rand::RngCore;

    let key = kek_to_aes_key(&kek_b64)?;

    // Build a placeholder sessions payload. In a full implementation this
    // would load real session data from the profile's database.
    let sessions_payload = serde_json::json!({
        "profile_id": profile_id,
        "sessions": [],
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });
    let plaintext = serde_json::to_vec(&sessions_payload)
        .map_err(|e| format!("JSON serialization error: {e}"))?;

    // Generate a random 12-byte nonce.
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let ciphertext = aes256gcm_encrypt(&key, &nonce_bytes, &plaintext)?;

    let encrypted_payload = base64::engine::general_purpose::STANDARD.encode(&ciphertext);
    let nonce = base64::engine::general_purpose::STANDARD.encode(nonce_bytes);
    let checksum = sha256_hex(&ciphertext);

    Ok(SyncPackage {
        version: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        profile_id,
        checksum,
        encrypted_payload,
        nonce,
    })
}

/// Decrypt and import a sync package, returning any conflicts detected.
#[tauri::command]
pub fn sync_import_package(
    package: SyncPackage,
    kek_b64: String,
    conflict_resolution: ConflictResolution,
) -> Result<Vec<SyncConflict>, String> {
    use base64::Engine as _;

    let key = kek_to_aes_key(&kek_b64)?;

    // Decode the ciphertext.
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(&package.encrypted_payload)
        .map_err(|e| format!("encrypted_payload base64 decode error: {e}"))?;

    // Verify checksum before decrypting.
    let actual_checksum = sha256_hex(&ciphertext);
    if actual_checksum != package.checksum {
        return Err(format!(
            "Checksum mismatch: expected {}, got {}",
            package.checksum, actual_checksum
        ));
    }

    // Decode nonce.
    let nonce_raw = base64::engine::general_purpose::STANDARD
        .decode(&package.nonce)
        .map_err(|e| format!("nonce base64 decode error: {e}"))?;
    if nonce_raw.len() != 12 {
        return Err(format!("Nonce must be 12 bytes, got {}", nonce_raw.len()));
    }
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&nonce_raw);

    let plaintext = aes256gcm_decrypt(&key, &nonce_bytes, &ciphertext)?;

    let payload: serde_json::Value = serde_json::from_slice(&plaintext)
        .map_err(|e| format!("JSON deserialization error: {e}"))?;

    // In a full implementation: compare imported sessions against local sessions,
    // produce SyncConflict entries for any that collide, then apply
    // `conflict_resolution` to determine which side wins.
    let sessions = payload
        .get("sessions")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    let conflicts: Vec<SyncConflict> = sessions
        .iter()
        .filter_map(|s| {
            let session_id = s.get("id")?.as_str()?;
            // Placeholder: no actual local sessions exist in this stub, so no
            // real conflicts are produced. A production implementation would
            // compare against the local DB here.
            let _ = &conflict_resolution; // consumed to suppress unused warning
            Some(SyncConflict {
                session_id: session_id.to_string(),
                local_updated_at: String::new(),
                remote_updated_at: String::new(),
                resolution: ConflictResolution::KeepRemote,
            })
        })
        .collect();

    Ok(conflicts)
}

/// Generate a sync share code (base64url of the serialised `SyncPackage`).
/// The result is safe for QR code display and URL embedding.
#[tauri::command]
pub fn sync_generate_share_code(package: SyncPackage) -> Result<String, String> {
    use base64::Engine as _;
    let json = serde_json::to_vec(&package)
        .map_err(|e| format!("JSON serialization error: {e}"))?;
    let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&json);
    Ok(code)
}

/// Parse a sync share code back into a `SyncPackage`.
#[tauri::command]
pub fn sync_parse_share_code(code: String) -> Result<SyncPackage, String> {
    use base64::Engine as _;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(code.trim())
        .map_err(|e| format!("share code base64url decode error: {e}"))?;
    let package: SyncPackage = serde_json::from_slice(&json)
        .map_err(|e| format!("share code JSON parse error: {e}"))?;
    Ok(package)
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

    // ── Phase 3 tests ───────────────────────────────────────────────────

    /// Build a deterministic 32-byte KEK for tests and return it base64-encoded.
    fn test_kek_b64() -> String {
        use base64::Engine as _;
        let key = [0x42u8; 32];
        base64::engine::general_purpose::STANDARD.encode(key)
    }

    #[test]
    fn test_sync_package_checksum_matches_payload() {
        use base64::Engine as _;

        let kek = test_kek_b64();
        let pkg = sync_create_package("profile-test".to_string(), kek)
            .expect("create_package failed");

        // Decode the stored ciphertext and recompute the SHA-256.
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&pkg.encrypted_payload)
            .expect("encrypted_payload decode failed");

        let recomputed = sha256_hex(&ciphertext);
        assert_eq!(
            pkg.checksum, recomputed,
            "stored checksum must equal SHA-256 of encrypted_payload"
        );
    }

    #[test]
    fn test_sync_create_and_import_roundtrip() {
        let kek = test_kek_b64();
        let profile_id = "roundtrip-profile".to_string();

        // Create a package.
        let pkg = sync_create_package(profile_id.clone(), kek.clone())
            .expect("create_package failed");

        assert_eq!(pkg.version, 1);
        assert_eq!(pkg.profile_id, profile_id);
        assert!(!pkg.encrypted_payload.is_empty());
        assert!(!pkg.nonce.is_empty());
        assert_eq!(pkg.checksum.len(), 64); // SHA-256 hex = 64 chars

        // Import it back — should succeed with no conflicts (empty sessions list).
        let conflicts =
            sync_import_package(pkg, kek, ConflictResolution::KeepRemote)
                .expect("import_package failed");

        assert!(
            conflicts.is_empty(),
            "no conflicts expected for an empty sessions payload"
        );
    }

    #[test]
    fn test_sync_conflict_resolution_keep_remote() {
        // Directly verify the ConflictResolution enum serialises correctly
        // and that a SyncConflict can be round-tripped through JSON.
        let conflict = SyncConflict {
            session_id: "sess-001".to_string(),
            local_updated_at: "2026-01-01T10:00:00Z".to_string(),
            remote_updated_at: "2026-01-02T10:00:00Z".to_string(),
            resolution: ConflictResolution::KeepRemote,
        };

        let json = serde_json::to_string(&conflict).expect("serialize conflict");
        assert!(json.contains("keep_remote"), "expected snake_case keep_remote in JSON");

        let restored: SyncConflict =
            serde_json::from_str(&json).expect("deserialize conflict");
        assert_eq!(restored.session_id, "sess-001");

        // Verify KeepLocal and Merge variants as well.
        let keep_local = serde_json::to_string(&ConflictResolution::KeepLocal).unwrap();
        assert_eq!(keep_local, "\"keep_local\"");

        let merge = serde_json::to_string(&ConflictResolution::Merge).unwrap();
        assert_eq!(merge, "\"merge\"");
    }
}

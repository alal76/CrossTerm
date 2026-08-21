use crate::config::{ConfigState, Settings};
use crate::snippets::SnippetState;
use rand::RngCore;
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

#[derive(Debug, Clone, Serialize)]
pub struct SyncImportSummary {
    pub sessions_imported: u32,
    pub snippets_imported: u32,
    pub settings_applied: bool,
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

/// Encrypt `plaintext` with a password-derived AES-256-GCM key, returning a
/// self-contained blob: `[32-byte salt][12-byte nonce][ciphertext]`. The
/// salt and nonce travel with the ciphertext so import only needs the
/// password, not any separately-managed key material.
fn encrypt_with_password(plaintext: &[u8], password: &str) -> Result<Vec<u8>, SyncError> {
    let mut salt = vec![0u8; crate::vault::crypto::SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = crate::vault::crypto::derive_key(password.as_bytes(), &salt)
        .map_err(|e| SyncError::Encryption(e.to_string()))?;
    let (ciphertext, nonce) = crate::vault::crypto::encrypt(plaintext, &key)
        .map_err(|e| SyncError::Encryption(e.to_string()))?;

    let mut out = Vec::with_capacity(salt.len() + nonce.len() + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Inverse of `encrypt_with_password`.
fn decrypt_with_password(data: &[u8], password: &str) -> Result<Vec<u8>, SyncError> {
    let salt_len = crate::vault::crypto::SALT_LEN;
    let nonce_len = crate::vault::crypto::NONCE_LEN;
    if data.len() < salt_len + nonce_len {
        return Err(SyncError::InvalidFormat("bundle is too short to contain a salt and nonce".into()));
    }
    let (salt, rest) = data.split_at(salt_len);
    let (nonce, ciphertext) = rest.split_at(nonce_len);

    let key = crate::vault::crypto::derive_key(password.as_bytes(), salt)
        .map_err(|e| SyncError::Encryption(e.to_string()))?;
    crate::vault::crypto::decrypt(ciphertext, nonce, &key)
        .map_err(|_| SyncError::ImportFailed("wrong password, or the bundle is corrupted".into()))
}

#[tauri::command]
pub async fn sync_export(
    password: String,
    config_state: tauri::State<'_, ConfigState>,
    snippet_state: tauri::State<'_, SnippetState>,
    state: tauri::State<'_, SyncState>,
) -> Result<Vec<u8>, SyncError> {
    let sessions = crate::config::session_list(config_state.clone())
        .map_err(|e| SyncError::ExportFailed(e.to_string()))?
        .into_iter()
        .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
        .collect();
    let settings = crate::config::settings_get(config_state)
        .map_err(|e| SyncError::ExportFailed(e.to_string()))
        .and_then(|s| serde_json::to_value(s).map_err(SyncError::from))?;
    let snippets = crate::snippets::snippet_list(snippet_state)
        .map_err(|e| SyncError::ExportFailed(e.to_string()))?
        .into_iter()
        .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
        .collect();

    let bundle = SyncBundle {
        version: "2.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        settings,
        sessions,
        snippets,
        // Custom themes live in frontend localStorage (appStore), not any
        // backend store this command has access to — left empty rather than
        // fabricated. Built-in themes need no export; they ship with the app.
        themes: vec![],
    };

    let json = serde_json::to_vec(&bundle)?;
    let encrypted = encrypt_with_password(&json, &password)?;

    let now = chrono::Utc::now().to_rfc3339();
    *state.last_export.lock().unwrap() = Some(now);

    Ok(encrypted)
}

#[tauri::command]
pub async fn sync_import(
    data: Vec<u8>,
    password: String,
    config_state: tauri::State<'_, ConfigState>,
    snippet_state: tauri::State<'_, SnippetState>,
    state: tauri::State<'_, SyncState>,
) -> Result<SyncImportSummary, SyncError> {
    let decrypted = decrypt_with_password(&data, &password)?;

    let bundle: SyncBundle = serde_json::from_slice(&decrypted)
        .map_err(|e| SyncError::InvalidFormat(e.to_string()))?;

    // Sessions carry their original id, so re-importing the same bundle
    // (e.g. syncing again later) updates the existing session in place
    // rather than duplicating it — session_create/do_session_create writes
    // by id.
    let mut sessions_imported = 0u32;
    for session_json in bundle.sessions {
        if let Ok(session) = serde_json::from_value::<crate::config::SessionDefinition>(session_json) {
            let request = crate::config::SessionCreateRequest {
                id: Some(session.id),
                name: session.name,
                session_type: session.session_type,
                group: session.group,
                tags: Some(session.tags),
                icon: session.icon,
                color_label: session.color_label,
                credential_ref: session.credential_ref,
                connection: session.connection,
                startup_script: session.startup_script,
                environment_variables: Some(session.environment_variables),
                notes: session.notes,
                auto_reconnect: Some(session.auto_reconnect),
                keep_alive_interval_seconds: Some(session.keep_alive_interval_seconds),
                favorite: Some(session.favorite),
                settings_override: session.settings_override,
            };
            if crate::config::session_create(config_state.clone(), request).is_ok() {
                sessions_imported += 1;
            }
        }
    }

    // Snippets always get a fresh id (snippet_create doesn't support
    // preserving one), so re-importing the same bundle will duplicate them —
    // a real but minor rough edge, not a silent no-op.
    let mut snippets_imported = 0u32;
    for snippet_json in bundle.snippets {
        if let Ok(snippet) = serde_json::from_value::<crate::snippets::Snippet>(snippet_json) {
            if crate::snippets::snippet_create(snippet.name, snippet.command, snippet.tags, snippet_state.clone()).is_ok() {
                snippets_imported += 1;
            }
        }
    }

    let settings_applied = serde_json::from_value::<Settings>(bundle.settings)
        .ok()
        .and_then(|settings| crate::config::settings_update(config_state, settings).ok())
        .is_some();

    let now = chrono::Utc::now().to_rfc3339();
    *state.last_import.lock().unwrap() = Some(now);

    Ok(SyncImportSummary {
        sessions_imported,
        snippets_imported,
        settings_applied,
    })
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
    fn test_sync_bundle_roundtrips_through_serde() {
        let bundle = SyncBundle {
            version: "2.0".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            settings: serde_json::json!({"theme": "dark"}),
            sessions: vec![serde_json::json!({"name": "test"})],
            snippets: vec![],
            themes: vec![],
        };
        let json = serde_json::to_vec(&bundle).unwrap();
        let restored: SyncBundle = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.version, "2.0");
        assert_eq!(restored.sessions.len(), 1);
    }

    // Regression coverage: sync_export/sync_import used to "encrypt" with a
    // single-byte XOR (trivially reversible, not encryption at all) and the
    // export bundle was always empty regardless of real state. These test
    // the real AES-256-GCM + Argon2id password-based encryption that
    // replaced it.
    #[test]
    fn test_encrypt_decrypt_with_password_roundtrips() {
        let plaintext = b"{\"sessions\":[{\"name\":\"prod-db\"}]}";
        let encrypted = encrypt_with_password(plaintext, "correct horse battery staple").unwrap();

        // Salt + nonce + at least some ciphertext, and it must not just be
        // the plaintext bytes shifted (real encryption, not obfuscation).
        assert!(encrypted.len() > plaintext.len());
        assert_ne!(&encrypted[..plaintext.len().min(encrypted.len())], &plaintext[..]);

        let decrypted = decrypt_with_password(&encrypted, "correct horse battery staple").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_password_fails() {
        let plaintext = b"secret bundle contents";
        let encrypted = encrypt_with_password(plaintext, "right-password").unwrap();

        let result = decrypt_with_password(&encrypted, "wrong-password");
        assert!(result.is_err(), "decrypting with the wrong password must fail, not silently return garbage");
    }

    #[test]
    fn test_decrypt_rejects_too_short_input() {
        let result = decrypt_with_password(&[1, 2, 3], "any-password");
        assert!(matches!(result, Err(SyncError::InvalidFormat(_))));
    }

    #[test]
    fn test_two_exports_with_the_same_password_use_different_salt_and_nonce() {
        // Same plaintext + same password must still produce different
        // ciphertext bytes each time (fresh random salt/nonce per call) —
        // otherwise identical exports would leak that fact to an observer.
        let plaintext = b"identical content";
        let a = encrypt_with_password(plaintext, "same-password").unwrap();
        let b = encrypt_with_password(plaintext, "same-password").unwrap();
        assert_ne!(a, b);

        // Both still decrypt correctly with the right password.
        assert_eq!(decrypt_with_password(&a, "same-password").unwrap(), plaintext);
        assert_eq!(decrypt_with_password(&b, "same-password").unwrap(), plaintext);
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

    // ── kek_to_aes_key error paths ────────────────────────────────────

    #[test]
    fn test_kek_to_aes_key_invalid_base64_errors() {
        let result = kek_to_aes_key("not valid base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_kek_to_aes_key_wrong_length_errors() {
        use base64::Engine as _;
        let short_key = base64::engine::general_purpose::STANDARD.encode([0x01u8; 16]);
        let result = kek_to_aes_key(&short_key);
        let err = result.unwrap_err();
        assert!(err.contains("32 bytes"));
        assert!(err.contains("16"));
    }

    #[test]
    fn test_kek_to_aes_key_valid_roundtrips() {
        use base64::Engine as _;
        let raw = [0xABu8; 32];
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
        let key = kek_to_aes_key(&encoded).unwrap();
        assert_eq!(key, raw);
    }

    // ── sha256_hex ───────────────────────────────────────────────────

    #[test]
    fn test_sha256_hex_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let digest = sha256_hex(b"");
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // ── sync_import_package error paths ───────────────────────────────

    #[test]
    fn test_sync_import_package_checksum_mismatch_errors() {
        let kek = test_kek_b64();
        let mut pkg = sync_create_package("p1".to_string(), kek.clone()).unwrap();
        pkg.checksum = "0".repeat(64); // corrupt the checksum

        let result = sync_import_package(pkg, kek, ConflictResolution::KeepRemote);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Checksum mismatch"));
    }

    #[test]
    fn test_sync_import_package_bad_nonce_length_errors() {
        use base64::Engine as _;
        let kek = test_kek_b64();
        let mut pkg = sync_create_package("p1".to_string(), kek.clone()).unwrap();

        // Recompute checksum so it still matches (unchanged payload), but
        // shrink the nonce to an invalid length.
        pkg.nonce = base64::engine::general_purpose::STANDARD.encode([0u8; 5]);

        let result = sync_import_package(pkg, kek, ConflictResolution::KeepRemote);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("12 bytes"));
    }

    #[test]
    fn test_sync_import_package_wrong_kek_fails_to_decrypt() {
        use base64::Engine as _;
        let kek = test_kek_b64();
        let pkg = sync_create_package("p1".to_string(), kek).unwrap();

        let wrong_kek = base64::engine::general_purpose::STANDARD.encode([0x99u8; 32]);
        let result = sync_import_package(pkg, wrong_kek, ConflictResolution::KeepRemote);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_import_package_invalid_base64_payload_errors() {
        let kek = test_kek_b64();
        let mut pkg = sync_create_package("p1".to_string(), kek.clone()).unwrap();
        pkg.encrypted_payload = "not base64 !!!".to_string();

        let result = sync_import_package(pkg, kek, ConflictResolution::KeepRemote);
        assert!(result.is_err());
    }

    // ── Share code round-trip ────────────────────────────────────────

    #[test]
    fn test_sync_generate_and_parse_share_code_roundtrip() {
        let kek = test_kek_b64();
        let pkg = sync_create_package("share-profile".to_string(), kek).unwrap();

        let code = sync_generate_share_code(pkg.clone()).unwrap();
        assert!(!code.is_empty());
        // URL-safe base64 must not contain '+' or '/'.
        assert!(!code.contains('+') && !code.contains('/'));

        let parsed = sync_parse_share_code(code).unwrap();
        assert_eq!(parsed.profile_id, pkg.profile_id);
        assert_eq!(parsed.checksum, pkg.checksum);
        assert_eq!(parsed.encrypted_payload, pkg.encrypted_payload);
        assert_eq!(parsed.nonce, pkg.nonce);
    }

    #[test]
    fn test_sync_parse_share_code_invalid_base64_errors() {
        let result = sync_parse_share_code("not valid base64url!!!".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_parse_share_code_invalid_json_errors() {
        use base64::Engine as _;
        // Valid base64url, but not a SyncPackage once decoded.
        let bogus = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"not json");
        let result = sync_parse_share_code(bogus);
        assert!(result.is_err());
    }

    // ── SyncError Display / Serialize ───────────────────────────────

    #[test]
    fn test_sync_error_display() {
        assert_eq!(
            SyncError::ExportFailed("disk full".into()).to_string(),
            "Export failed: disk full"
        );
        assert_eq!(
            SyncError::ImportFailed("bad data".into()).to_string(),
            "Import failed: bad data"
        );
        assert_eq!(
            SyncError::InvalidFormat("truncated".into()).to_string(),
            "Invalid bundle format: truncated"
        );
    }

    #[test]
    fn test_sync_error_serialize() {
        let err = SyncError::ExportFailed("oops".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Export failed: oops\"");
    }
}

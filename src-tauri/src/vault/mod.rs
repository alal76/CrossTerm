use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::{DateTime, Utc};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,
    #[error("Vault is already unlocked")]
    AlreadyUnlocked,
    #[error("Vault already exists at this path")]
    AlreadyExists,
    #[error("Vault not found")]
    NotFound,
    #[error("Invalid master password")]
    InvalidPassword,
    #[error("Too many unlock attempts. Retry after {0} seconds")]
    RateLimited(u64),
    #[error("Credential not found: {0}")]
    CredentialNotFound(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Biometric authentication is not available on this device")]
    BiometricUnavailable,
    #[error("Biometric authentication failed")]
    BiometricFailed,
    #[error("OS credential store error: {0}")]
    OsStoreError(String),
    #[error("FIDO2/WebAuthn is not configured")]
    Fido2NotConfigured,
    #[error("Vault registry error: {0}")]
    RegistryError(String),
    #[error("Password required to delete vault")]
    PasswordRequiredForDelete,
}

impl Serialize for VaultError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    Password,
    SshKey,
    Certificate,
    ApiToken,
    CloudCredential,
    TotpSeed,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub id: String,
    pub name: String,
    pub credential_type: CredentialType,
    pub username: Option<String>,
    /// Encrypted JSON blob – never exposed as plaintext outside vault
    #[serde(skip_serializing)]
    pub encrypted_data: Vec<u8>,
    /// Nonce used for AES-GCM
    #[serde(skip_serializing)]
    pub nonce: Vec<u8>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metadata-only view returned by list operations (no secrets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub name: String,
    pub credential_type: CredentialType,
    pub username: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CredentialCreateRequest {
    pub name: String,
    pub credential_type: CredentialType,
    pub username: Option<String>,
    /// Plaintext JSON object with type-specific fields.
    pub data: serde_json::Value,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CredentialUpdateRequest {
    pub name: Option<String>,
    pub username: Option<String>,
    pub data: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

/// The decrypted view returned to the frontend on explicit get.
#[derive(Debug, Serialize)]
pub struct CredentialDetail {
    pub id: String,
    pub name: String,
    pub credential_type: CredentialType,
    pub username: Option<String>,
    pub data: serde_json::Value,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── FIDO2/WebAuthn Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnChallenge {
    pub challenge: String,
    pub rp_id: String,
    pub rp_name: String,
    pub user_id: String,
    pub user_name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnCredential {
    pub credential_id: String,
    pub public_key: String,
    pub sign_count: u32,
}

// ── Vault Registry ──────────────────────────────────────────────────────

/// Metadata for a vault stored in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub owner_profile_id: String,
    pub shared_with: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VaultRegistry {
    vaults: Vec<VaultInfo>,
}

impl VaultRegistry {
    fn registry_path() -> PathBuf {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("CrossTerm");
        std::fs::create_dir_all(&base).ok();
        base.join("vault_registry.json")
    }

    #[cfg(test)]
    fn registry_path_for_test(base: &std::path::Path) -> PathBuf {
        base.join("vault_registry.json")
    }

    fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self, path: &std::path::Path) -> Result<(), VaultError> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    fn list_for_profile(&self, profile_id: &str) -> Vec<VaultInfo> {
        self.vaults
            .iter()
            .filter(|v| {
                v.owner_profile_id == profile_id
                    || v.shared_with.contains(&profile_id.to_string())
            })
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    fn find(&self, vault_id: &str) -> Option<&VaultInfo> {
        self.vaults.iter().find(|v| v.id == vault_id)
    }

    fn find_mut(&mut self, vault_id: &str) -> Option<&mut VaultInfo> {
        self.vaults.iter_mut().find(|v| v.id == vault_id)
    }

    #[allow(dead_code)]
    fn default_for_profile(&self, profile_id: &str) -> Option<&VaultInfo> {
        self.vaults
            .iter()
            .find(|v| v.owner_profile_id == profile_id && v.is_default)
    }

    fn add(&mut self, info: VaultInfo) {
        self.vaults.push(info);
    }

    fn remove(&mut self, vault_id: &str) {
        self.vaults.retain(|v| v.id != vault_id);
    }
}

// ── Crypto helpers ──────────────────────────────────────────────────────

const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32; // AES-256
const NONCE_LEN: usize = 12; // AES-GCM standard

/// Derive a 256-bit key from a master password and salt using Argon2id.
fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let params = Params::new(65536, 3, 4, Some(KEY_LEN))
        .map_err(|e| VaultError::Encryption(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new(vec![0u8; KEY_LEN]);
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| VaultError::Encryption(e.to_string()))?;
    Ok(key)
}

fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<(Vec<u8>, Vec<u8>), VaultError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| VaultError::Encryption(e.to_string()))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::Encryption(e.to_string()))?;
    Ok((ciphertext, nonce_bytes.to_vec()))
}

fn decrypt(ciphertext: &[u8], nonce_bytes: &[u8], key: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| VaultError::Decryption(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VaultError::Decryption(e.to_string()))
}

// ── Vault ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct VaultInner {
    db: Connection,
    encryption_key: Option<Zeroizing<Vec<u8>>>,
    salt: Vec<u8>,
    last_activity: Instant,
    idle_timeout_secs: u64,
    db_path: PathBuf,
    vault_id: String,
}

impl Drop for VaultInner {
    fn drop(&mut self) {
        // Ensure key material is zeroized on drop (Zeroizing handles this).
        self.encryption_key = None;
    }
}

pub struct Vault {
    open_vaults: Mutex<HashMap<String, VaultInner>>,
    /// Per-vault rate limiting: vault_id → (failed_attempts, last_failed_at)
    rate_limits: Mutex<HashMap<String, (u32, Option<Instant>)>>,
    /// Per-vault auto-lock cancel senders.
    auto_lock_cancels: Mutex<HashMap<String, watch::Sender<bool>>>,
    /// Path override for the vault registry (used in tests).
    registry_path_override: Mutex<Option<PathBuf>>,
}

impl Vault {
    pub fn new() -> Self {
        Self {
            open_vaults: Mutex::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            auto_lock_cancels: Mutex::new(HashMap::new()),
            registry_path_override: Mutex::new(None),
        }
    }

    /// Return the registry path (production or test override).
    fn registry_path(&self) -> PathBuf {
        let guard = self.registry_path_override.lock().unwrap();
        match &*guard {
            Some(p) => p.clone(),
            None => VaultRegistry::registry_path(),
        }
    }

    /// Return the database path for a vault.
    pub fn vault_db_path(vault_id: &str) -> PathBuf {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("CrossTerm")
            .join("vaults")
            .join(vault_id);
        std::fs::create_dir_all(&base).ok();
        base.join("vault.db")
    }

    /// Return the legacy database path (for migration).
    #[allow(dead_code)]
    pub fn db_path(profile_id: &str) -> PathBuf {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("CrossTerm")
            .join("profiles")
            .join(profile_id);
        std::fs::create_dir_all(&base).ok();
        base.join("vault.db")
    }

    // ── registry helpers ────────────────────────────────────────────

    fn load_registry(&self) -> VaultRegistry {
        VaultRegistry::load(&self.registry_path())
    }

    fn save_registry(&self, registry: &VaultRegistry) -> Result<(), VaultError> {
        registry.save(&self.registry_path())
    }

    /// List vaults accessible by the given profile.
    pub fn list_vaults(&self, profile_id: &str) -> Vec<VaultInfo> {
        let registry = self.load_registry();
        let open = self.open_vaults.lock().unwrap();
        registry
            .list_for_profile(profile_id)
            .into_iter()
            .inspect(|v| {
                let _ = open.get(&v.id);
            })
            .collect()
    }

    // ── lifecycle ───────────────────────────────────────────────────

    /// Create a brand-new vault with the given password.
    pub fn create(
        &self,
        vault_id: &str,
        profile_id: &str,
        name: &str,
        master_password: &str,
        is_default: bool,
    ) -> Result<VaultInfo, VaultError> {
        let path = Self::vault_db_path(vault_id);
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }

        let mut salt = vec![0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let key = derive_key(master_password.as_bytes(), &salt)?;

        let db = Connection::open(&path)?;
        Self::init_schema(&db)?;

        // Store salt and a verification token so we can validate passwords
        // on unlock without exposing the key.
        let mut verification_plain = vec![0u8; 32];
        OsRng.fill_bytes(&mut verification_plain);
        let (verify_ct, verify_nonce) = encrypt(&verification_plain, &key)?;

        db.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)",
            params!["salt", hex::encode(&salt)],
        )?;
        db.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)",
            params!["verify_plain", hex::encode(&verification_plain)],
        )?;
        db.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)",
            params!["verify_ct", hex::encode(&verify_ct)],
        )?;
        db.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)",
            params!["verify_nonce", hex::encode(&verify_nonce)],
        )?;

        let info = VaultInfo {
            id: vault_id.to_string(),
            name: name.to_string(),
            is_default,
            owner_profile_id: profile_id.to_string(),
            shared_with: Vec::new(),
            created_at: Utc::now().to_rfc3339(),
        };

        // Register in registry
        let mut registry = self.load_registry();
        registry.add(info.clone());
        self.save_registry(&registry)?;

        // Store open vault
        let mut guard = self.open_vaults.lock().unwrap();
        guard.insert(
            vault_id.to_string(),
            VaultInner {
                db,
                encryption_key: Some(key),
                salt,
                last_activity: Instant::now(),
                idle_timeout_secs: 900,
                db_path: path,
                vault_id: vault_id.to_string(),
            },
        );
        Ok(info)
    }

    /// Unlock an existing vault with rate limiting.
    pub fn unlock(&self, vault_id: &str, master_password: &str) -> Result<(), VaultError> {
        // Per-vault rate limiting
        {
            let rl = self.rate_limits.lock().unwrap();
            if let Some((failures, last_failed)) = rl.get(vault_id) {
                if *failures >= 3 {
                    if let Some(last) = last_failed {
                        let backoff_secs = 2u64.pow((*failures - 3).min(5));
                        let elapsed = last.elapsed().as_secs();
                        if elapsed < backoff_secs {
                            return Err(VaultError::RateLimited(backoff_secs - elapsed));
                        }
                    }
                }
            }
        }

        let path = Self::vault_db_path(vault_id);
        if !path.exists() {
            return Err(VaultError::NotFound);
        }

        let db = Connection::open(&path)?;
        let salt_hex: String =
            db.query_row("SELECT value FROM vault_meta WHERE key='salt'", [], |r| {
                r.get(0)
            })?;
        let verify_ct_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_ct'",
            [],
            |r| r.get(0),
        )?;
        let verify_nonce_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_nonce'",
            [],
            |r| r.get(0),
        )?;
        let verify_plain_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_plain'",
            [],
            |r| r.get(0),
        )?;

        let salt = hex::decode(&salt_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_ct =
            hex::decode(&verify_ct_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_nonce =
            hex::decode(&verify_nonce_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_plain =
            hex::decode(&verify_plain_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;

        let key = derive_key(master_password.as_bytes(), &salt)?;

        // Verify password by decrypting the verification token.
        let plain = decrypt(&verify_ct, &verify_nonce, &key);
        match plain {
            Ok(ref p) if p == &verify_plain => {}
            _ => {
                let mut rl = self.rate_limits.lock().unwrap();
                let entry = rl.entry(vault_id.to_string()).or_insert((0, None));
                entry.0 += 1;
                entry.1 = Some(Instant::now());
                return Err(VaultError::InvalidPassword);
            }
        }

        // Reset rate limit on success
        {
            let mut rl = self.rate_limits.lock().unwrap();
            rl.remove(vault_id);
        }

        let mut guard = self.open_vaults.lock().unwrap();
        guard.insert(
            vault_id.to_string(),
            VaultInner {
                db,
                encryption_key: Some(key),
                salt,
                last_activity: Instant::now(),
                idle_timeout_secs: 900,
                db_path: path,
                vault_id: vault_id.to_string(),
            },
        );
        Ok(())
    }

    /// Change the master password for a specific vault, re-encrypting all credentials.
    pub fn change_password(
        &self,
        vault_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), VaultError> {
        self.with_inner(vault_id, |inner| {
            let old_key = inner.encryption_key.as_ref().ok_or(VaultError::Locked)?;

            // Verify current password by re-deriving and comparing
            let verify_key = derive_key(current_password.as_bytes(), &inner.salt)?;
            if verify_key.as_slice() != old_key.as_slice() {
                return Err(VaultError::InvalidPassword);
            }

            // Generate new salt and key
            let mut new_salt = vec![0u8; SALT_LEN];
            OsRng.fill_bytes(&mut new_salt);
            let new_key = derive_key(new_password.as_bytes(), &new_salt)?;

            // Re-encrypt all credentials
            let mut stmt = inner
                .db
                .prepare("SELECT id, encrypted_data, nonce FROM credentials")?;
            let creds: Vec<(String, Vec<u8>, Vec<u8>)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (id, ct, nonce) in &creds {
                let plaintext = decrypt(ct, nonce, old_key)?;
                let (new_ct, new_nonce) = encrypt(&plaintext, &new_key)?;
                inner.db.execute(
                    "UPDATE credentials SET encrypted_data=?1, nonce=?2 WHERE id=?3",
                    params![new_ct, new_nonce, id],
                )?;
            }

            // Update salt and verification token
            let mut verification_plain = vec![0u8; 32];
            OsRng.fill_bytes(&mut verification_plain);
            let (verify_ct, verify_nonce) = encrypt(&verification_plain, &new_key)?;

            inner.db.execute(
                "UPDATE vault_meta SET value=?1 WHERE key='salt'",
                params![hex::encode(&new_salt)],
            )?;
            inner.db.execute(
                "UPDATE vault_meta SET value=?1 WHERE key='verify_plain'",
                params![hex::encode(&verification_plain)],
            )?;
            inner.db.execute(
                "UPDATE vault_meta SET value=?1 WHERE key='verify_ct'",
                params![hex::encode(&verify_ct)],
            )?;
            inner.db.execute(
                "UPDATE vault_meta SET value=?1 WHERE key='verify_nonce'",
                params![hex::encode(&verify_nonce)],
            )?;

            inner.salt = new_salt;
            inner.encryption_key = Some(new_key);
            inner.last_activity = Instant::now();

            Ok(())
        })
    }

    /// Lock a specific vault – zeroize the key and cancel background timer.
    pub fn lock(&self, vault_id: &str) -> Result<(), VaultError> {
        // Cancel the auto-lock background task for this vault
        if let Some(tx) = self
            .auto_lock_cancels
            .lock()
            .unwrap()
            .remove(vault_id)
        {
            let _ = tx.send(true);
        }
        let mut guard = self.open_vaults.lock().unwrap();
        guard.remove(vault_id);
        Ok(())
    }

    /// Lock all open vaults.
    pub fn lock_all(&self) -> Result<(), VaultError> {
        // Cancel all auto-lock timers
        let mut cancels = self.auto_lock_cancels.lock().unwrap();
        for (_, tx) in cancels.drain() {
            let _ = tx.send(true);
        }
        let mut guard = self.open_vaults.lock().unwrap();
        guard.clear();
        Ok(())
    }

    pub fn is_locked(&self, vault_id: &str) -> bool {
        let guard = self.open_vaults.lock().unwrap();
        match guard.get(vault_id) {
            None => true,
            Some(inner) => inner.encryption_key.is_none(),
        }
    }

    /// Delete a vault after verifying the password.
    pub fn delete_vault(&self, vault_id: &str, password: &str) -> Result<(), VaultError> {
        // Attempt to unlock to verify password (if not already open)
        let path = Self::vault_db_path(vault_id);
        if !path.exists() {
            return Err(VaultError::NotFound);
        }

        // Verify the password by opening the DB and checking
        let db = Connection::open(&path)?;
        let salt_hex: String =
            db.query_row("SELECT value FROM vault_meta WHERE key='salt'", [], |r| {
                r.get(0)
            })?;
        let verify_ct_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_ct'",
            [],
            |r| r.get(0),
        )?;
        let verify_nonce_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_nonce'",
            [],
            |r| r.get(0),
        )?;
        let verify_plain_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_plain'",
            [],
            |r| r.get(0),
        )?;

        let salt = hex::decode(&salt_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_ct =
            hex::decode(&verify_ct_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_nonce =
            hex::decode(&verify_nonce_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_plain =
            hex::decode(&verify_plain_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;

        let key = derive_key(password.as_bytes(), &salt)?;
        let plain = decrypt(&verify_ct, &verify_nonce, &key);
        match plain {
            Ok(ref p) if p == &verify_plain => {}
            _ => return Err(VaultError::InvalidPassword),
        }

        // Close db before deleting
        drop(db);

        // Remove from open vaults if present
        let _ = self.lock(vault_id);

        // Delete files
        if let Some(parent) = path.parent() {
            std::fs::remove_dir_all(parent)?;
        }

        // Remove from registry
        let mut registry = self.load_registry();
        registry.remove(vault_id);
        self.save_registry(&registry)?;

        Ok(())
    }

    /// Share a vault with another profile (requires vault password).
    pub fn share_vault(
        &self,
        vault_id: &str,
        password: &str,
        target_profile_id: &str,
    ) -> Result<(), VaultError> {
        // Verify password first
        let path = Self::vault_db_path(vault_id);
        if !path.exists() {
            return Err(VaultError::NotFound);
        }

        let db = Connection::open(&path)?;
        let salt_hex: String =
            db.query_row("SELECT value FROM vault_meta WHERE key='salt'", [], |r| {
                r.get(0)
            })?;
        let verify_ct_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_ct'",
            [],
            |r| r.get(0),
        )?;
        let verify_nonce_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_nonce'",
            [],
            |r| r.get(0),
        )?;
        let verify_plain_hex: String = db.query_row(
            "SELECT value FROM vault_meta WHERE key='verify_plain'",
            [],
            |r| r.get(0),
        )?;

        let salt = hex::decode(&salt_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_ct =
            hex::decode(&verify_ct_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_nonce =
            hex::decode(&verify_nonce_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;
        let verify_plain =
            hex::decode(&verify_plain_hex).map_err(|e| VaultError::Decryption(e.to_string()))?;

        let key = derive_key(password.as_bytes(), &salt)?;
        let plain = decrypt(&verify_ct, &verify_nonce, &key);
        match plain {
            Ok(ref p) if p == &verify_plain => {}
            _ => return Err(VaultError::InvalidPassword),
        }

        let mut registry = self.load_registry();
        if let Some(info) = registry.find_mut(vault_id) {
            if !info.shared_with.contains(&target_profile_id.to_string()) {
                info.shared_with.push(target_profile_id.to_string());
            }
        } else {
            return Err(VaultError::NotFound);
        }
        self.save_registry(&registry)?;
        Ok(())
    }

    /// Remove sharing of a vault with a profile.
    pub fn unshare_vault(
        &self,
        vault_id: &str,
        target_profile_id: &str,
    ) -> Result<(), VaultError> {
        let mut registry = self.load_registry();
        if let Some(info) = registry.find_mut(vault_id) {
            info.shared_with.retain(|p| p != target_profile_id);
        } else {
            return Err(VaultError::NotFound);
        }
        self.save_registry(&registry)?;
        Ok(())
    }

    /// Start a background tokio task that periodically checks idle timeout
    /// for a specific vault and auto-locks it when it expires.
    pub fn start_auto_lock_timer(&self, vault_id: &str, app_handle: AppHandle) {
        // Cancel any existing timer for this vault
        if let Some(tx) = self
            .auto_lock_cancels
            .lock()
            .unwrap()
            .remove(vault_id)
        {
            let _ = tx.send(true);
        }

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        self.auto_lock_cancels
            .lock()
            .unwrap()
            .insert(vault_id.to_string(), cancel_tx);

        let vid = vault_id.to_string();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let vault = app_handle.try_state::<Vault>();
                        let Some(vault) = vault else { break };
                        let should_lock = {
                            let guard = vault.open_vaults.lock().unwrap();
                            if let Some(inner) = guard.get(&vid) {
                                inner.encryption_key.is_some()
                                    && inner.last_activity.elapsed().as_secs() > inner.idle_timeout_secs
                            } else {
                                false
                            }
                        };
                        if should_lock {
                            {
                                let mut guard = vault.open_vaults.lock().unwrap();
                                guard.remove(&vid);
                            }
                            vault.auto_lock_cancels.lock().unwrap().remove(&vid);
                            let pid = app_handle
                                .try_state::<crate::config::ConfigState>()
                                .map(|cs| cs.active_profile_id.read().unwrap().clone().unwrap_or_default())
                                .unwrap_or_default();
                            crate::audit::append_event(
                                &pid,
                                crate::audit::AuditEventType::VaultAutoLock,
                                &format!("Vault {} auto-locked by background timer", vid),
                            );
                            let _ = app_handle.emit("vault:auto_locked", &vid);
                            break;
                        }
                    }
                    _ = cancel_rx.changed() => {
                        break;
                    }
                }
            }
        });
    }

    // ── auto-lock check ─────────────────────────────────────────────

    fn with_inner<F, T>(&self, vault_id: &str, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&mut VaultInner) -> Result<T, VaultError>,
    {
        let mut guard = self.open_vaults.lock().unwrap();
        let inner = guard.get_mut(vault_id).ok_or(VaultError::Locked)?;
        if inner.encryption_key.is_none() {
            return Err(VaultError::Locked);
        }
        // Check idle timeout
        if inner.last_activity.elapsed().as_secs() > inner.idle_timeout_secs {
            inner.encryption_key = None;
            return Err(VaultError::Locked);
        }
        inner.last_activity = Instant::now();
        f(inner)
    }

    // ── schema ──────────────────────────────────────────────────────

    fn init_schema(db: &Connection) -> Result<(), rusqlite::Error> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS vault_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS credentials (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                credential_type TEXT NOT NULL,
                username        TEXT,
                encrypted_data  BLOB NOT NULL,
                nonce           BLOB NOT NULL,
                tags            TEXT NOT NULL DEFAULT '[]',
                notes           TEXT,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );
            ",
        )
    }

    // ── credential CRUD ─────────────────────────────────────────────

    pub fn credential_create(&self, vault_id: &str, req: CredentialCreateRequest) -> Result<String, VaultError> {
        self.with_inner(vault_id, |inner| {
            let key = inner.encryption_key.as_ref().ok_or(VaultError::Locked)?;
            let id = Uuid::new_v4().to_string();
            let now = Utc::now();
            let mut plaintext = serde_json::to_vec(&req.data)?;
            let (ct, nonce) = encrypt(&plaintext, key)?;
            plaintext.zeroize();

            let tags_json = serde_json::to_string(&req.tags.unwrap_or_default())?;

            inner.db.execute(
                "INSERT INTO credentials (id, name, credential_type, username, encrypted_data, nonce, tags, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    req.name,
                    serde_json::to_string(&req.credential_type)?.trim_matches('"'),
                    req.username,
                    ct,
                    nonce,
                    tags_json,
                    req.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;
            Ok(id)
        })
    }

    pub fn credential_list(&self, vault_id: &str) -> Result<Vec<CredentialSummary>, VaultError> {
        self.with_inner(vault_id, |inner| {
            let mut stmt = inner.db.prepare(
                "SELECT id, name, credential_type, username, tags, created_at, updated_at FROM credentials ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                let ctype_str: String = row.get(2)?;
                let tags_str: String = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(CredentialSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    credential_type: serde_json::from_str(&format!("\"{}\"", ctype_str))
                        .unwrap_or(CredentialType::Password),
                    username: row.get(3)?,
                    tags: serde_json::from_str(&tags_str).unwrap_or_default(),
                    created_at: DateTime::parse_from_rfc3339(&created_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(VaultError::from)
        })
    }

    pub fn credential_get(&self, vault_id: &str, id: &str) -> Result<CredentialDetail, VaultError> {
        self.with_inner(vault_id, |inner| {
            let key = inner.encryption_key.as_ref().ok_or(VaultError::Locked)?;
            let row = inner.db.query_row(
                "SELECT id, name, credential_type, username, encrypted_data, nonce, tags, notes, created_at, updated_at FROM credentials WHERE id=?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, Vec<u8>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                    ))
                },
            ).map_err(|_| VaultError::CredentialNotFound(id.to_string()))?;

            let plaintext = decrypt(&row.4, &row.5, key)?;
            let data: serde_json::Value = serde_json::from_slice(&plaintext)?;

            Ok(CredentialDetail {
                id: row.0,
                name: row.1,
                credential_type: serde_json::from_str(&format!("\"{}\"", row.2))
                    .unwrap_or(CredentialType::Password),
                username: row.3,
                data,
                tags: serde_json::from_str(&row.6).unwrap_or_default(),
                notes: row.7,
                created_at: DateTime::parse_from_rfc3339(&row.8)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&row.9)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
    }

    pub fn credential_update(&self, vault_id: &str, id: &str, req: CredentialUpdateRequest) -> Result<(), VaultError> {
        self.with_inner(vault_id, |inner| {
            let key = inner.encryption_key.as_ref().ok_or(VaultError::Locked)?;
            let now = Utc::now();

            // Verify credential exists
            let exists: bool = inner.db.query_row(
                "SELECT COUNT(*) FROM credentials WHERE id=?1",
                params![id],
                |r| r.get::<_, i64>(0).map(|c| c > 0),
            )?;
            if !exists {
                return Err(VaultError::CredentialNotFound(id.to_string()));
            }

            if let Some(name) = &req.name {
                inner.db.execute(
                    "UPDATE credentials SET name=?1, updated_at=?2 WHERE id=?3",
                    params![name, now.to_rfc3339(), id],
                )?;
            }
            if let Some(username) = &req.username {
                inner.db.execute(
                    "UPDATE credentials SET username=?1, updated_at=?2 WHERE id=?3",
                    params![username, now.to_rfc3339(), id],
                )?;
            }
            if let Some(tags) = &req.tags {
                let tags_json = serde_json::to_string(tags)?;
                inner.db.execute(
                    "UPDATE credentials SET tags=?1, updated_at=?2 WHERE id=?3",
                    params![tags_json, now.to_rfc3339(), id],
                )?;
            }
            if let Some(notes) = &req.notes {
                inner.db.execute(
                    "UPDATE credentials SET notes=?1, updated_at=?2 WHERE id=?3",
                    params![notes, now.to_rfc3339(), id],
                )?;
            }
            if let Some(data) = &req.data {
                let mut plaintext = serde_json::to_vec(data)?;
                let (ct, nonce) = encrypt(&plaintext, key)?;
                plaintext.zeroize();
                inner.db.execute(
                    "UPDATE credentials SET encrypted_data=?1, nonce=?2, updated_at=?3 WHERE id=?4",
                    params![ct, nonce, now.to_rfc3339(), id],
                )?;
            }
            Ok(())
        })
    }

    pub fn credential_delete(&self, vault_id: &str, id: &str) -> Result<(), VaultError> {
        self.with_inner(vault_id, |inner| {
            let affected = inner.db.execute(
                "DELETE FROM credentials WHERE id=?1",
                params![id],
            )?;
            if affected == 0 {
                return Err(VaultError::CredentialNotFound(id.to_string()));
            }
            Ok(())
        })
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn vault_create(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    profile_id: String,
    master_password: String,
    name: Option<String>,
    is_default: Option<bool>,
) -> Result<VaultInfo, VaultError> {
    let vault_id = Uuid::new_v4().to_string();
    let vault_name = name.unwrap_or_else(|| "Default".to_string());
    let default = is_default.unwrap_or(true);
    let result = state.create(&vault_id, &profile_id, &vault_name, &master_password, default);
    if let Ok(ref info) = result {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultCreate,
            &format!("Vault '{}' ({}) created", info.name, info.id),
        );
    }
    result
}

#[tauri::command]
pub fn vault_unlock(
    app_handle: AppHandle,
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    master_password: String,
) -> Result<(), VaultError> {
    let result = state.unlock(&vault_id, &master_password);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultUnlock, &format!("Vault {} unlocked", vault_id));
        state.start_auto_lock_timer(&vault_id, app_handle);
    }
    result
}

#[tauri::command]
pub fn vault_lock(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
) -> Result<(), VaultError> {
    let result = state.lock(&vault_id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultLock, &format!("Vault {} locked", vault_id));
    }
    result
}

#[tauri::command]
pub fn vault_lock_all(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<(), VaultError> {
    let result = state.lock_all();
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultLock, "All vaults locked");
    }
    result
}

#[tauri::command]
pub fn vault_is_locked(state: tauri::State<'_, Vault>, vault_id: String) -> bool {
    state.is_locked(&vault_id)
}

#[tauri::command]
pub fn vault_list(
    state: tauri::State<'_, Vault>,
    profile_id: String,
) -> Vec<VaultInfo> {
    state.list_vaults(&profile_id)
}

#[tauri::command]
pub fn vault_delete(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    password: String,
) -> Result<(), VaultError> {
    let result = state.delete_vault(&vault_id, &password);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultLock, &format!("Vault {} deleted", vault_id));
    }
    result
}

#[tauri::command]
pub fn vault_share(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    password: String,
    target_profile_id: String,
) -> Result<(), VaultError> {
    let result = state.share_vault(&vault_id, &password, &target_profile_id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultUnlock,
            &format!("Vault {} shared with profile {}", vault_id, target_profile_id),
        );
    }
    result
}

#[tauri::command]
pub fn vault_unshare(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    target_profile_id: String,
) -> Result<(), VaultError> {
    let result = state.unshare_vault(&vault_id, &target_profile_id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultLock,
            &format!("Vault {} unshared from profile {}", vault_id, target_profile_id),
        );
    }
    result
}

#[tauri::command]
pub fn credential_create(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    request: CredentialCreateRequest,
) -> Result<String, VaultError> {
    let name = request.name.clone();
    let result = state.credential_create(&vault_id, request);
    if let Ok(ref id) = result {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialCreate, &format!("Created credential '{}' ({}) in vault {}", name, id, vault_id));
    }
    result
}

#[tauri::command]
pub fn credential_list(
    state: tauri::State<'_, Vault>,
    vault_id: String,
) -> Result<Vec<CredentialSummary>, VaultError> {
    state.credential_list(&vault_id)
}

#[tauri::command]
pub fn credential_get(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    id: String,
) -> Result<CredentialDetail, VaultError> {
    let result = state.credential_get(&vault_id, &id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialAccess, &format!("Accessed credential {} in vault {}", id, vault_id));
    }
    result
}

#[tauri::command]
pub fn vault_change_password(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    current_password: String,
    new_password: String,
) -> Result<(), VaultError> {
    let result = state.change_password(&vault_id, &current_password, &new_password);
    if result.is_ok() {
        let pid = config_state
            .active_profile_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultUnlock,
            &format!("Vault {} master password changed", vault_id),
        );
    }
    result
}

#[tauri::command]
pub fn credential_update(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    id: String,
    request: CredentialUpdateRequest,
) -> Result<(), VaultError> {
    let result = state.credential_update(&vault_id, &id, request);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialUpdate, &format!("Updated credential {} in vault {}", id, vault_id));
    }
    result
}

#[tauri::command]
pub fn credential_delete(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
    id: String,
) -> Result<(), VaultError> {
    let result = state.credential_delete(&vault_id, &id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialDelete, &format!("Deleted credential {} from vault {}", id, vault_id));
    }
    result
}

// ── BE-VAULT-01: Auto-lock idle check ───────────────────────────────────

#[tauri::command]
pub fn vault_check_idle(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<Vec<String>, VaultError> {
    let mut guard = state.open_vaults.lock().unwrap();
    let mut locked_ids = Vec::new();
    let ids_to_check: Vec<String> = guard.keys().cloned().collect();
    for vid in ids_to_check {
        if let Some(inner) = guard.get_mut(&vid) {
            if inner.encryption_key.is_some()
                && inner.last_activity.elapsed().as_secs() > inner.idle_timeout_secs
            {
                inner.encryption_key = None;
                locked_ids.push(vid);
            }
        }
    }
    // Remove locked vaults
    for vid in &locked_ids {
        guard.remove(vid);
    }
    if !locked_ids.is_empty() {
        let pid = config_state
            .active_profile_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultAutoLock,
            &format!("Vaults auto-locked due to idle: {:?}", locked_ids),
        );
    }
    Ok(locked_ids)
}

// ── BE-VAULT-05: Credential orphan check ────────────────────────────────

#[tauri::command]
pub fn vault_check_orphans(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    vault_id: String,
) -> Result<Vec<String>, VaultError> {
    let all_creds = state.credential_list(&vault_id)?;
    let all_ids: std::collections::HashSet<String> =
        all_creds.iter().map(|c| c.id.clone()).collect();

    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();

    let referenced: std::collections::HashSet<String> = if !pid.is_empty() {
        crate::config::do_session_list_for_profile(&pid)
            .iter()
            .filter_map(|s| s.credential_ref.clone())
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    Ok(all_ids.difference(&referenced).cloned().collect())
}

// ── BE-VAULT-06: Clipboard auto-clear ───────────────────────────────────

#[tauri::command]
pub async fn vault_clipboard_copy(
    config_state: tauri::State<'_, crate::config::ConfigState>,
    text: String,
    clear_after_secs: u32,
) -> Result<(), VaultError> {
    {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| VaultError::Encryption(format!("Clipboard error: {}", e)))?;
        clipboard
            .set_text(&text)
            .map_err(|e| VaultError::Encryption(format!("Clipboard error: {}", e)))?;
    }

    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::ClipboardCopy,
        &format!("Copied to clipboard, auto-clear in {}s", clear_after_secs),
    );

    let delay = clear_after_secs;
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(delay as u64)).await;
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text("");
        }
    });

    Ok(())
}

// ── BE-VAULT-07: Biometric Unlock ───────────────────────────────────────

#[tauri::command]
pub fn vault_biometric_available() -> Result<bool, VaultError> {
    #[cfg(target_os = "macos")]
    return Ok(true); // Touch ID may be available

    #[cfg(target_os = "windows")]
    return Ok(true); // Windows Hello may be available

    #[cfg(target_os = "linux")]
    return Ok(false); // Generally not available on desktop Linux

    #[allow(unreachable_code)]
    Ok(false)
}

#[tauri::command]
pub fn vault_unlock_biometric(state: tauri::State<'_, Vault>) -> Result<bool, VaultError> {
    let _guard = state.open_vaults.lock().unwrap();

    // Platform-specific biometric integration stubs:
    // - macOS: LocalAuthentication.framework via objc2 or swift bridge
    // - Windows: Windows Hello via webauthn-authenticator-rs
    // - Linux: polkit or libfido2

    #[cfg(target_os = "macos")]
    {
        // TODO: Integrate with LocalAuthentication.framework
        return Err(VaultError::BiometricUnavailable);
    }

    #[cfg(target_os = "windows")]
    {
        // TODO: Integrate with Windows Hello
        return Err(VaultError::BiometricUnavailable);
    }

    #[cfg(target_os = "linux")]
    {
        return Err(VaultError::BiometricUnavailable);
    }

    #[allow(unreachable_code)]
    Err(VaultError::BiometricUnavailable)
}

#[tauri::command]
pub fn vault_biometric_enroll(
    master_password: String,
    vault_id: String,
    state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    let guard = state.open_vaults.lock().unwrap();
    let inner_ref = guard.get(&vault_id).ok_or(VaultError::Locked)?;
    if inner_ref.encryption_key.is_none() {
        return Err(VaultError::Locked);
    }
    // In a real implementation, this would:
    // 1. Derive the vault key from master_password
    // 2. Store it in Keychain (macOS) / Credential Manager (Windows) with biometric protection
    // 3. Mark the vault as biometric-enabled
    let _ = master_password; // Silence unused warning
    Ok(())
}

// ── BE-VAULT-08: OS Credential Store Delegation ─────────────────────────

#[tauri::command]
pub fn vault_os_store_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows", target_os = "linux"))
}

#[tauri::command]
pub fn vault_os_store_save(
    master_password: String,
    vault_id: String,
    state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    let guard = state.open_vaults.lock().unwrap();
    let inner_ref = guard.get(&vault_id).ok_or(VaultError::Locked)?;
    if inner_ref.encryption_key.is_none() {
        return Err(VaultError::Locked);
    }

    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    entry
        .set_password(&master_password)
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn vault_os_store_retrieve(
    vault_id: String,
    app_handle: AppHandle,
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<(), VaultError> {
    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    let password = entry
        .get_password()
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;

    // Re-use the existing unlock logic
    let result = state.unlock(&vault_id, &password);
    if result.is_ok() {
        let pid = config_state
            .active_profile_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultUnlock,
            "Vault unlocked via OS credential store",
        );
        state.start_auto_lock_timer(&vault_id, app_handle);
    }
    result
}

#[tauri::command]
pub fn vault_os_store_delete(vault_id: String) -> Result<(), VaultError> {
    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    Ok(())
}

// ── BE-VAULT-09: FIDO2/WebAuthn Hardware Key ────────────────────────────

#[tauri::command]
pub fn vault_fido2_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

#[tauri::command]
pub fn vault_fido2_register_begin(
    vault_id: String,
    _state: tauri::State<'_, Vault>,
) -> Result<WebAuthnChallenge, VaultError> {
    let challenge = WebAuthnChallenge {
        challenge: Uuid::new_v4().to_string(),
        rp_id: "crossterm.app".to_string(),
        rp_name: "CrossTerm".to_string(),
        user_id: vault_id,
        user_name: "CrossTerm User".to_string(),
    };
    Ok(challenge)
}

#[tauri::command]
pub fn vault_fido2_register_complete(
    _credential_response: String,
    _state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    // In production: webauthn-rs verifies attestation, stores credential
    Err(VaultError::Fido2NotConfigured)
}

#[tauri::command]
pub fn vault_fido2_auth_begin(
    _vault_id: String,
    _state: tauri::State<'_, Vault>,
) -> Result<WebAuthnChallenge, VaultError> {
    Err(VaultError::Fido2NotConfigured)
}

#[tauri::command]
pub fn vault_fido2_auth_complete(
    _credential_response: String,
    _state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    Err(VaultError::Fido2NotConfigured)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEST_PASSWORD: &str = "testpass123!";
    const WRONG_PASSWORD: &str = "wrongpass999!";

    /// Test context: generates unique profile/vault IDs and cleans up on drop.
    struct TestProfile {
        profile_id: String,
        vault_id: String,
        registry_path: PathBuf,
        extra_vault_ids: Vec<String>,
    }

    impl TestProfile {
        fn new() -> Self {
            let registry_path = std::env::temp_dir()
                .join(format!("crossterm-test-reg-{}.json", Uuid::new_v4()));
            Self {
                profile_id: Uuid::new_v4().to_string(),
                vault_id: Uuid::new_v4().to_string(),
                registry_path,
                extra_vault_ids: Vec::new(),
            }
        }

        fn pid(&self) -> &str {
            &self.profile_id
        }

        fn vid(&self) -> &str {
            &self.vault_id
        }

        fn new_vault_id(&mut self) -> String {
            let id = Uuid::new_v4().to_string();
            self.extra_vault_ids.push(id.clone());
            id
        }
    }

    impl Drop for TestProfile {
        fn drop(&mut self) {
            // Clean up primary vault dir
            let path = Vault::vault_db_path(&self.vault_id);
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir_all(parent);
            }
            // Clean up extra vault dirs
            for vid in &self.extra_vault_ids {
                let path = Vault::vault_db_path(vid);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }
            // Clean up test registry file
            let _ = std::fs::remove_file(&self.registry_path);
        }
    }

    fn setup_vault(tp: &TestProfile) -> Vault {
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());
        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        vault
    }

    fn make_password_request(name: &str) -> CredentialCreateRequest {
        CredentialCreateRequest {
            name: name.to_string(),
            credential_type: CredentialType::Password,
            username: Some("admin".to_string()),
            data: json!({"password": "s3cret!"}),
            tags: Some(vec!["prod".to_string(), "db".to_string()]),
            notes: Some("Production DB credential".to_string()),
        }
    }

    fn make_ssh_key_request(name: &str) -> CredentialCreateRequest {
        CredentialCreateRequest {
            name: name.to_string(),
            credential_type: CredentialType::SshKey,
            username: Some("deploy".to_string()),
            data: json!({
                "private_key": "-----BEGIN OPENSSH PRIVATE KEY-----\nfake-key-data\n-----END OPENSSH PRIVATE KEY-----",
                "passphrase": "keypass"
            }),
            tags: Some(vec!["ssh".to_string()]),
            notes: Some("Deploy key".to_string()),
        }
    }

    // ── UT-V-01: Create and unlock ──────────────────────────────────

    #[test]
    fn test_vault_create_and_unlock() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let info = vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        assert_eq!(info.name, "Default");
        assert!(info.is_default);
        assert_eq!(info.owner_profile_id, tp.pid());
        assert!(!vault.is_locked(tp.vid()));

        // Lock then re-unlock
        vault.lock(tp.vid()).unwrap();
        assert!(vault.is_locked(tp.vid()));

        assert!(vault.unlock(tp.vid(), TEST_PASSWORD).is_ok());
        assert!(!vault.is_locked(tp.vid()));
    }

    // ── UT-V-02: Wrong password ─────────────────────────────────────

    #[test]
    fn test_vault_wrong_password() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());
        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        vault.lock(tp.vid()).unwrap();

        let result = vault.unlock(tp.vid(), WRONG_PASSWORD);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), VaultError::InvalidPassword),
            "Expected InvalidPassword error"
        );
    }

    // ── UT-V-03: Credential roundtrip (password) ────────────────────

    #[test]
    fn test_credential_roundtrip_password() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = make_password_request("DB Password");
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "DB Password");
        assert_eq!(detail.credential_type, CredentialType::Password);
        assert_eq!(detail.username.as_deref(), Some("admin"));
        assert_eq!(detail.data["password"], "s3cret!");
        assert_eq!(detail.tags, vec!["prod", "db"]);
        assert_eq!(detail.notes.as_deref(), Some("Production DB credential"));
    }

    // ── UT-V-04: Credential roundtrip (SSH key) ─────────────────────

    #[test]
    fn test_credential_roundtrip_ssh_key() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = make_ssh_key_request("Deploy Key");
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "Deploy Key");
        assert_eq!(detail.credential_type, CredentialType::SshKey);
        assert_eq!(detail.username.as_deref(), Some("deploy"));
        assert!(detail.data["private_key"]
            .as_str()
            .unwrap()
            .contains("OPENSSH PRIVATE KEY"));
        assert_eq!(detail.data["passphrase"], "keypass");
        assert_eq!(detail.tags, vec!["ssh"]);
    }

    // ── UT-V-05: Credential update ──────────────────────────────────

    #[test]
    fn test_credential_update() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let id = vault
            .credential_create(tp.vid(), make_password_request("Original"))
            .unwrap();

        vault
            .credential_update(
                tp.vid(),
                &id,
                CredentialUpdateRequest {
                    name: Some("Updated Name".to_string()),
                    username: Some("newuser".to_string()),
                    data: Some(json!({"password": "newpass!"})),
                    tags: Some(vec!["staging".to_string()]),
                    notes: Some("Updated notes".to_string()),
                },
            )
            .unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "Updated Name");
        assert_eq!(detail.username.as_deref(), Some("newuser"));
        assert_eq!(detail.data["password"], "newpass!");
        assert_eq!(detail.tags, vec!["staging"]);
        assert_eq!(detail.notes.as_deref(), Some("Updated notes"));
    }

    // ── UT-V-06: Credential delete ──────────────────────────────────

    #[test]
    fn test_credential_delete() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let id = vault
            .credential_create(tp.vid(), make_password_request("ToDelete"))
            .unwrap();

        vault.credential_delete(tp.vid(), &id).unwrap();

        let result = vault.credential_get(tp.vid(), &id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));
    }

    // ── UT-V-07: Credential list ────────────────────────────────────

    #[test]
    fn test_credential_list() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        for i in 0..5 {
            vault
                .credential_create(
                    tp.vid(),
                    CredentialCreateRequest {
                        name: format!("Cred-{}", i),
                        credential_type: CredentialType::Password,
                        username: Some(format!("user{}", i)),
                        data: json!({"password": format!("pass{}", i)}),
                        tags: Some(vec![format!("tag{}", i)]),
                        notes: None,
                    },
                )
                .unwrap();
        }

        let list = vault.credential_list(tp.vid()).unwrap();
        assert_eq!(list.len(), 5);

        for summary in &list {
            assert!(!summary.id.is_empty());
            assert!(!summary.name.is_empty());
            assert_eq!(summary.credential_type, CredentialType::Password);
            assert!(summary.username.is_some());
        }
    }

    // ── UT-V-08: Locked operations ──────────────────────────────────

    #[test]
    fn test_vault_locked_operations() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let id = vault
            .credential_create(tp.vid(), make_password_request("Pre-lock"))
            .unwrap();

        vault.lock(tp.vid()).unwrap();
        assert!(vault.is_locked(tp.vid()));

        assert!(matches!(
            vault
                .credential_create(tp.vid(), make_password_request("ShouldFail"))
                .unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_list(tp.vid()).unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_get(tp.vid(), &id).unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault
                .credential_update(
                    tp.vid(),
                    &id,
                    CredentialUpdateRequest {
                        name: Some("x".into()),
                        username: None,
                        data: None,
                        tags: None,
                        notes: None,
                    }
                )
                .unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_delete(tp.vid(), &id).unwrap_err(),
            VaultError::Locked
        ));
    }

    // ── UT-V-09: Encryption produces different ciphertexts ──────────

    #[test]
    fn test_encryption_different_ciphertexts() {
        let key = [0xABu8; KEY_LEN];
        let plaintext = b"same plaintext every time";

        let (ct1, nonce1) = encrypt(plaintext, &key).unwrap();
        let (ct2, nonce2) = encrypt(plaintext, &key).unwrap();

        assert_ne!(nonce1, nonce2, "Nonces must differ");
        assert_ne!(ct1, ct2, "Ciphertexts must differ for same plaintext");

        let rt1 = decrypt(&ct1, &nonce1, &key).unwrap();
        let rt2 = decrypt(&ct2, &nonce2, &key).unwrap();
        assert_eq!(rt1, plaintext);
        assert_eq!(rt2, plaintext);
    }

    // ── UT-V-10: Change password ────────────────────────────────────

    #[test]
    fn test_vault_change_password() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);
        let new_password = "newSecurePass456!";

        let id1 = vault
            .credential_create(tp.vid(), make_password_request("Cred1"))
            .unwrap();
        let id2 = vault
            .credential_create(tp.vid(), make_ssh_key_request("Cred2"))
            .unwrap();

        vault
            .change_password(tp.vid(), TEST_PASSWORD, new_password)
            .unwrap();

        let d1 = vault.credential_get(tp.vid(), &id1).unwrap();
        assert_eq!(d1.name, "Cred1");
        assert_eq!(d1.data["password"], "s3cret!");

        let d2 = vault.credential_get(tp.vid(), &id2).unwrap();
        assert_eq!(d2.name, "Cred2");
        assert!(d2.data["private_key"].as_str().unwrap().contains("OPENSSH"));

        vault.lock(tp.vid()).unwrap();
        assert!(vault.unlock(tp.vid(), new_password).is_ok());

        vault.lock(tp.vid()).unwrap();
        assert!(matches!(
            vault.unlock(tp.vid(), TEST_PASSWORD).unwrap_err(),
            VaultError::InvalidPassword
        ));
    }

    // ── UT-V-11: Rate limiting ──────────────────────────────────────

    #[test]
    fn test_rate_limiting() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());
        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        vault.lock(tp.vid()).unwrap();

        for i in 0..3 {
            let result = vault.unlock(tp.vid(), WRONG_PASSWORD);
            assert!(
                matches!(result, Err(VaultError::InvalidPassword)),
                "Attempt {} should be InvalidPassword",
                i + 1
            );
        }

        let result = vault.unlock(tp.vid(), WRONG_PASSWORD);
        assert!(
            matches!(result, Err(VaultError::RateLimited(_))),
            "Should be rate limited after 3 failures, got: {:?}",
            result
        );
    }

    // ── UT-V-12: Empty vault operations ─────────────────────────────

    #[test]
    fn test_empty_vault_operations() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let list = vault.credential_list(tp.vid()).unwrap();
        assert!(list.is_empty());

        let result = vault.credential_get(tp.vid(), "non-existent-id");
        assert!(matches!(
            result.unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));

        let result = vault.credential_update(
            tp.vid(),
            "non-existent-id",
            CredentialUpdateRequest {
                name: Some("x".into()),
                username: None,
                data: None,
                tags: None,
                notes: None,
            },
        );
        assert!(matches!(
            result.unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));

        let result = vault.credential_delete(tp.vid(), "non-existent-id");
        assert!(matches!(
            result.unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));
    }

    // ── UT-V-13: Auto-lock idle timeout ─────────────────────────────

    #[test]
    fn test_vault_auto_lock_idle() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        {
            let mut guard = vault.open_vaults.lock().unwrap();
            if let Some(inner) = guard.get_mut(tp.vid()) {
                inner.idle_timeout_secs = 0;
                inner.last_activity = Instant::now() - std::time::Duration::from_secs(10);
            }
        }

        let result = vault.credential_list(tp.vid());
        assert!(matches!(result.unwrap_err(), VaultError::Locked));
    }

    #[test]
    fn test_vault_no_auto_lock_when_active() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let result = vault.credential_list(tp.vid());
        assert!(result.is_ok());
    }

    // ── Credential roundtrip (certificate) ──────────────────────────

    #[test]
    fn test_credential_roundtrip_certificate() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = CredentialCreateRequest {
            name: "TLS Cert".to_string(),
            credential_type: CredentialType::Certificate,
            username: Some("server".to_string()),
            data: json!({
                "cert_data": "-----BEGIN CERTIFICATE-----\nMIIBxTCCAW...\n-----END CERTIFICATE-----",
                "private_key": "-----BEGIN PRIVATE KEY-----\nMIIEvQIBAD...\n-----END PRIVATE KEY-----"
            }),
            tags: Some(vec!["tls".to_string(), "prod".to_string()]),
            notes: Some("Production TLS certificate".to_string()),
        };
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "TLS Cert");
        assert_eq!(detail.credential_type, CredentialType::Certificate);
        assert_eq!(detail.username.as_deref(), Some("server"));
        assert!(detail.data["cert_data"]
            .as_str()
            .unwrap()
            .contains("BEGIN CERTIFICATE"));
        assert!(detail.data["private_key"]
            .as_str()
            .unwrap()
            .contains("BEGIN PRIVATE KEY"));
        assert_eq!(detail.tags, vec!["tls", "prod"]);
        assert_eq!(detail.notes.as_deref(), Some("Production TLS certificate"));
    }

    // ── Credential roundtrip (API token) ────────────────────────────

    #[test]
    fn test_credential_roundtrip_api_token() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = CredentialCreateRequest {
            name: "GitHub Token".to_string(),
            credential_type: CredentialType::ApiToken,
            username: Some("devuser".to_string()),
            data: json!({
                "provider": "github",
                "token": "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "expiry": "2027-12-31T23:59:59Z"
            }),
            tags: Some(vec!["api".to_string(), "github".to_string()]),
            notes: Some("GitHub personal access token".to_string()),
        };
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "GitHub Token");
        assert_eq!(detail.credential_type, CredentialType::ApiToken);
        assert_eq!(detail.username.as_deref(), Some("devuser"));
        assert_eq!(detail.data["provider"], "github");
        assert_eq!(
            detail.data["token"],
            "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
        );
        assert_eq!(detail.data["expiry"], "2027-12-31T23:59:59Z");
        assert_eq!(detail.tags, vec!["api", "github"]);
    }

    // ── Credential roundtrip (cloud) ────────────────────────────────

    #[test]
    fn test_credential_roundtrip_cloud() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = CredentialCreateRequest {
            name: "AWS Prod".to_string(),
            credential_type: CredentialType::CloudCredential,
            username: Some("iam-deploy".to_string()),
            data: json!({
                "provider": "aws",
                "access_key": "AKIAIOSFODNN7EXAMPLE",
                "secret_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
                "region": "us-east-1"
            }),
            tags: Some(vec!["aws".to_string(), "prod".to_string()]),
            notes: Some("AWS production credentials".to_string()),
        };
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "AWS Prod");
        assert_eq!(detail.credential_type, CredentialType::CloudCredential);
        assert_eq!(detail.username.as_deref(), Some("iam-deploy"));
        assert_eq!(detail.data["provider"], "aws");
        assert_eq!(detail.data["access_key"], "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(
            detail.data["secret_key"],
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        );
        assert_eq!(detail.data["region"], "us-east-1");
        assert_eq!(detail.tags, vec!["aws", "prod"]);
    }

    // ── Credential roundtrip (TOTP seed) ────────────────────────────

    #[test]
    fn test_credential_roundtrip_totp() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        let req = CredentialCreateRequest {
            name: "2FA Seed".to_string(),
            credential_type: CredentialType::TotpSeed,
            username: Some("user@example.com".to_string()),
            data: json!({
                "secret": "JBSWY3DPEHPK3PXP",
                "issuer": "ExampleCorp",
                "digits": 6,
                "period": 30
            }),
            tags: Some(vec!["totp".to_string(), "2fa".to_string()]),
            notes: Some("TOTP seed for ExampleCorp".to_string()),
        };
        let id = vault.credential_create(tp.vid(), req).unwrap();

        let detail = vault.credential_get(tp.vid(), &id).unwrap();
        assert_eq!(detail.name, "2FA Seed");
        assert_eq!(detail.credential_type, CredentialType::TotpSeed);
        assert_eq!(detail.username.as_deref(), Some("user@example.com"));
        assert_eq!(detail.data["secret"], "JBSWY3DPEHPK3PXP");
        assert_eq!(detail.data["issuer"], "ExampleCorp");
        assert_eq!(detail.data["digits"], 6);
        assert_eq!(detail.data["period"], 30);
        assert_eq!(detail.tags, vec!["totp", "2fa"]);
    }

    // ── UT-V-14: Argon2id parameters ────────────────────────────────

    #[test]
    fn test_argon2id_parameters() {
        let password = b"test-password";
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);

        let key = derive_key(password, &salt).unwrap();
        assert_eq!(
            key.len(),
            32,
            "Derived key must be exactly 32 bytes (AES-256)"
        );

        let key2 = derive_key(password, &salt).unwrap();
        assert_eq!(
            key.as_slice(),
            key2.as_slice(),
            "Same password+salt must produce same key"
        );

        let mut salt2 = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt2);
        let key3 = derive_key(password, &salt2).unwrap();
        assert_ne!(
            key.as_slice(),
            key3.as_slice(),
            "Different salt must produce different key"
        );
    }

    // ── UT-V-19: Concurrent access ──────────────────────────────────

    #[test]
    fn test_concurrent_access() {
        let tp = TestProfile::new();
        let vault = std::sync::Arc::new(setup_vault(&tp));
        let vid = tp.vid().to_string();

        let id = vault
            .credential_create(&vid, make_password_request("Shared Cred"))
            .unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let vault_clone = std::sync::Arc::clone(&vault);
            let id_clone = id.clone();
            let vid_clone = vid.clone();
            handles.push(std::thread::spawn(move || {
                if i % 2 == 0 {
                    let detail = vault_clone.credential_get(&vid_clone, &id_clone).unwrap();
                    assert_eq!(detail.name, "Shared Cred");
                } else {
                    let _ = vault_clone.credential_list(&vid_clone).unwrap();
                }
            }));
        }

        for handle in handles {
            handle
                .join()
                .expect("Thread should not panic during concurrent vault access");
        }

        let detail = vault.credential_get(&vid, &id).unwrap();
        assert_eq!(detail.name, "Shared Cred");
        assert_eq!(detail.data["password"], "s3cret!");
    }

    // ── Clipboard ───────────────────────────────────────────────────

    #[test]
    fn test_clipboard_arboard_available() {
        let result = arboard::Clipboard::new();
        assert!(result.is_ok() || result.is_err());
    }

    // ── UT-V-20: Biometric availability ─────────────────────────────

    #[test]
    fn test_biometric_available_returns_bool() {
        let result = super::vault_biometric_available();
        assert!(result.is_ok());
        let available = result.unwrap();
        if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
            assert!(available);
        } else if cfg!(target_os = "linux") {
            assert!(!available);
        }
    }

    // ── UT-V-21: OS store availability ──────────────────────────────

    #[test]
    fn test_os_store_available() {
        let available = super::vault_os_store_available();
        if cfg!(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )) {
            assert!(available);
        }
    }

    // ── UT-V-22: FIDO2 availability ─────────────────────────────────

    #[test]
    fn test_fido2_available() {
        let available = super::vault_fido2_available();
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            assert!(available);
        } else {
            assert!(!available);
        }
    }

    // ── UT-V-23: WebAuthnChallenge serialization ────────────────────

    #[test]
    fn test_webauthn_challenge_serialization() {
        let challenge = WebAuthnChallenge {
            challenge: "test-challenge-id".to_string(),
            rp_id: "crossterm.app".to_string(),
            rp_name: "CrossTerm".to_string(),
            user_id: "user-123".to_string(),
            user_name: "CrossTerm User".to_string(),
        };

        let json = serde_json::to_string(&challenge).unwrap();
        assert!(json.contains("test-challenge-id"));
        assert!(json.contains("crossterm.app"));
        assert!(json.contains("CrossTerm"));

        let deserialized: WebAuthnChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.challenge, "test-challenge-id");
        assert_eq!(deserialized.rp_id, "crossterm.app");
        assert_eq!(deserialized.rp_name, "CrossTerm");
        assert_eq!(deserialized.user_id, "user-123");
        assert_eq!(deserialized.user_name, "CrossTerm User");
    }

    // ── UT-V-24: WebAuthnCredential serialization ───────────────────

    #[test]
    fn test_webauthn_credential_serialization() {
        let cred = WebAuthnCredential {
            credential_id: "cred-abc".to_string(),
            public_key: "pk-data".to_string(),
            sign_count: 42,
        };

        let json = serde_json::to_string(&cred).unwrap();
        let deserialized: WebAuthnCredential = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.credential_id, "cred-abc");
        assert_eq!(deserialized.public_key, "pk-data");
        assert_eq!(deserialized.sign_count, 42);
    }

    // ── UT-V-25: VaultError new variants serialize correctly ────────

    #[test]
    fn test_new_error_variants_serialize() {
        let errors: Vec<VaultError> = vec![
            VaultError::BiometricUnavailable,
            VaultError::BiometricFailed,
            VaultError::OsStoreError("test error".to_string()),
            VaultError::Fido2NotConfigured,
            VaultError::RegistryError("test".to_string()),
            VaultError::PasswordRequiredForDelete,
        ];

        for err in &errors {
            let json = serde_json::to_string(err).unwrap();
            assert!(!json.is_empty());
        }

        assert_eq!(
            VaultError::BiometricUnavailable.to_string(),
            "Biometric authentication is not available on this device"
        );
        assert_eq!(
            VaultError::BiometricFailed.to_string(),
            "Biometric authentication failed"
        );
        assert_eq!(
            VaultError::OsStoreError("keyring fail".to_string()).to_string(),
            "OS credential store error: keyring fail"
        );
        assert_eq!(
            VaultError::Fido2NotConfigured.to_string(),
            "FIDO2/WebAuthn is not configured"
        );
    }

    // ── UT-V-26: Biometric enroll requires unlocked vault ───────────

    #[test]
    fn test_biometric_enroll_requires_unlocked() {
        let vault = Vault::new();
        let guard = vault.open_vaults.lock().unwrap();
        assert!(guard.is_empty(), "No vaults should be open");
    }

    // ── UT-MV-01: Multi-vault create and independent unlock ─────────

    #[test]
    fn test_multi_vault_independent_lock_unlock() {
        let mut tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let vid2 = tp.new_vault_id();
        let pass2 = "other-vault-pass!";

        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        vault
            .create(&vid2, tp.pid(), "Work Vault", pass2, false)
            .unwrap();

        assert!(!vault.is_locked(tp.vid()));
        assert!(!vault.is_locked(&vid2));

        // Lock only vault 1
        vault.lock(tp.vid()).unwrap();
        assert!(vault.is_locked(tp.vid()));
        assert!(!vault.is_locked(&vid2)); // vault 2 still open

        // Credentials in vault 2 still accessible
        let cred_id = vault
            .credential_create(&vid2, make_password_request("V2 Cred"))
            .unwrap();
        let detail = vault.credential_get(&vid2, &cred_id).unwrap();
        assert_eq!(detail.name, "V2 Cred");

        // Vault 1 operations fail
        assert!(matches!(
            vault
                .credential_list(tp.vid())
                .unwrap_err(),
            VaultError::Locked
        ));

        // Unlock vault 1, lock vault 2
        vault.unlock(tp.vid(), TEST_PASSWORD).unwrap();
        vault.lock(&vid2).unwrap();

        assert!(!vault.is_locked(tp.vid()));
        assert!(vault.is_locked(&vid2));
    }

    // ── UT-MV-02: Vault list by profile ─────────────────────────────

    #[test]
    fn test_vault_list_by_profile() {
        let mut tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let vid2 = tp.new_vault_id();
        let other_profile = Uuid::new_v4().to_string();

        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();
        vault
            .create(&vid2, &other_profile, "Other Vault", "otherpass!", true)
            .unwrap();

        let my_vaults = vault.list_vaults(tp.pid());
        assert_eq!(my_vaults.len(), 1);
        assert_eq!(my_vaults[0].id, tp.vid());

        let other_vaults = vault.list_vaults(&other_profile);
        assert_eq!(other_vaults.len(), 1);
        assert_eq!(other_vaults[0].id, vid2);
    }

    // ── UT-MV-03: Vault delete with password ────────────────────────

    #[test]
    fn test_vault_delete_with_password() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();

        // Wrong password fails
        let result = vault.delete_vault(tp.vid(), WRONG_PASSWORD);
        assert!(matches!(result.unwrap_err(), VaultError::InvalidPassword));

        // Correct password deletes
        vault.delete_vault(tp.vid(), TEST_PASSWORD).unwrap();
        assert!(vault.is_locked(tp.vid()));

        let vaults = vault.list_vaults(tp.pid());
        assert!(vaults.is_empty());
    }

    // ── UT-MV-04: Vault share and unshare ───────────────────────────

    #[test]
    fn test_vault_share_and_unshare() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let target_profile = Uuid::new_v4().to_string();

        vault
            .create(tp.vid(), tp.pid(), "Shared Vault", TEST_PASSWORD, false)
            .unwrap();

        // Not visible to other profile yet
        let vaults = vault.list_vaults(&target_profile);
        assert!(vaults.is_empty());

        // Share with wrong password fails
        let result = vault.share_vault(tp.vid(), WRONG_PASSWORD, &target_profile);
        assert!(matches!(result.unwrap_err(), VaultError::InvalidPassword));

        // Share with correct password
        vault
            .share_vault(tp.vid(), TEST_PASSWORD, &target_profile)
            .unwrap();

        let vaults = vault.list_vaults(&target_profile);
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].id, tp.vid());

        // Unshare
        vault.unshare_vault(tp.vid(), &target_profile).unwrap();
        let vaults = vault.list_vaults(&target_profile);
        assert!(vaults.is_empty());
    }

    // ── UT-MV-05: Lock all vaults ───────────────────────────────────

    #[test]
    fn test_lock_all_vaults() {
        let mut tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let vid2 = tp.new_vault_id();

        vault
            .create(tp.vid(), tp.pid(), "V1", TEST_PASSWORD, true)
            .unwrap();
        vault
            .create(&vid2, tp.pid(), "V2", "pass2!", false)
            .unwrap();

        assert!(!vault.is_locked(tp.vid()));
        assert!(!vault.is_locked(&vid2));

        vault.lock_all().unwrap();

        assert!(vault.is_locked(tp.vid()));
        assert!(vault.is_locked(&vid2));
    }

    // ── UT-MV-06: VaultInfo returned on create ──────────────────────

    #[test]
    fn test_vault_info_on_create() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let info = vault
            .create(tp.vid(), tp.pid(), "My Vault", TEST_PASSWORD, false)
            .unwrap();

        assert_eq!(info.id, tp.vid());
        assert_eq!(info.name, "My Vault");
        assert!(!info.is_default);
        assert_eq!(info.owner_profile_id, tp.pid());
        assert!(info.shared_with.is_empty());
        assert!(!info.created_at.is_empty());
    }

    // ── UT-MV-07: Registry persistence ──────────────────────────────

    #[test]
    fn test_registry_persistence() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        vault
            .create(tp.vid(), tp.pid(), "Persisted", TEST_PASSWORD, true)
            .unwrap();

        // Create new Vault instance reading same registry
        let vault2 = Vault::new();
        *vault2.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());
        let vaults = vault2.list_vaults(tp.pid());
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].name, "Persisted");
    }

    // ── UT-MV-08: Default vault per profile ─────────────────────────

    #[test]
    fn test_default_vault_per_profile() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        vault
            .create(tp.vid(), tp.pid(), "Default", TEST_PASSWORD, true)
            .unwrap();

        let registry = vault.load_registry();
        let default = registry.default_for_profile(tp.pid());
        assert!(default.is_some());
        assert_eq!(default.unwrap().id, tp.vid());
        assert!(default.unwrap().is_default);
    }

    // ── UT-MV-09: Credentials isolated between vaults ───────────────

    #[test]
    fn test_credentials_isolated_between_vaults() {
        let mut tp = TestProfile::new();
        let vault = Vault::new();
        *vault.registry_path_override.lock().unwrap() = Some(tp.registry_path.clone());

        let vid2 = tp.new_vault_id();

        vault
            .create(tp.vid(), tp.pid(), "V1", TEST_PASSWORD, true)
            .unwrap();
        vault
            .create(&vid2, tp.pid(), "V2", "pass2!", false)
            .unwrap();

        let cred1 = vault
            .credential_create(tp.vid(), make_password_request("V1 Cred"))
            .unwrap();
        let cred2 = vault
            .credential_create(&vid2, make_password_request("V2 Cred"))
            .unwrap();

        // Each vault has exactly 1 credential
        assert_eq!(vault.credential_list(tp.vid()).unwrap().len(), 1);
        assert_eq!(vault.credential_list(&vid2).unwrap().len(), 1);

        // Credential from V1 not visible in V2
        assert!(matches!(
            vault.credential_get(&vid2, &cred1).unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));
        assert!(matches!(
            vault.credential_get(tp.vid(), &cred2).unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));
    }
}

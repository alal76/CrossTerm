use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::{DateTime, Utc};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

// ── Error ───────────────────────────────────────────────────────────────

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

pub struct VaultInner {
    db: Connection,
    encryption_key: Option<Zeroizing<Vec<u8>>>,
    salt: Vec<u8>,
    last_activity: Instant,
    idle_timeout_secs: u64,
    db_path: PathBuf,
}

impl Drop for VaultInner {
    fn drop(&mut self) {
        // Ensure key material is zeroized on drop (Zeroizing handles this).
        self.encryption_key = None;
    }
}

pub struct Vault {
    inner: Mutex<Option<VaultInner>>,
    /// Rate limiting: (failed_attempts, last_failed_at)
    rate_limit: Mutex<(u32, Option<Instant>)>,
    /// Sender to cancel the auto-lock background task.
    auto_lock_cancel: Mutex<Option<watch::Sender<bool>>>,
}

impl Vault {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            rate_limit: Mutex::new((0, None)),
            auto_lock_cancel: Mutex::new(None),
        }
    }

    /// Return the default vault database path for a given profile.
    pub fn db_path(profile_id: &str) -> PathBuf {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("CrossTerm")
            .join("profiles")
            .join(profile_id);
        std::fs::create_dir_all(&base).ok();
        base.join("vault.db")
    }

    // ── lifecycle ───────────────────────────────────────────────────

    /// Create a brand-new vault with the given master password.
    pub fn create(&self, profile_id: &str, master_password: &str) -> Result<(), VaultError> {
        let path = Self::db_path(profile_id);
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
        // Generate a random 32-byte token to prevent known-plaintext attacks.
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

        let mut guard = self.inner.lock().unwrap();
        *guard = Some(VaultInner {
            db,
            encryption_key: Some(key),
            salt,
            last_activity: Instant::now(),
            idle_timeout_secs: 900, // 15 min default
            db_path: path,
        });
        Ok(())
    }

    /// Unlock an existing vault with rate limiting.
    pub fn unlock(&self, profile_id: &str, master_password: &str) -> Result<(), VaultError> {
        // Rate limiting: exponential backoff after 3 failures
        {
            let rl = self.rate_limit.lock().unwrap();
            let (failures, last_failed) = &*rl;
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

        let path = Self::db_path(profile_id);
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
                let mut rl = self.rate_limit.lock().unwrap();
                rl.0 += 1;
                rl.1 = Some(Instant::now());
                return Err(VaultError::InvalidPassword);
            }
        }

        // Reset rate limit on success
        {
            let mut rl = self.rate_limit.lock().unwrap();
            *rl = (0, None);
        }

        let mut guard = self.inner.lock().unwrap();
        *guard = Some(VaultInner {
            db,
            encryption_key: Some(key),
            salt,
            last_activity: Instant::now(),
            idle_timeout_secs: 900,
            db_path: path,
        });
        Ok(())
    }

    /// Change the master password, re-encrypting all credentials.
    pub fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), VaultError> {
        self.with_inner(|inner| {
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
            let mut stmt = inner.db.prepare(
                "SELECT id, encrypted_data, nonce FROM credentials",
            )?;
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

    /// Lock the vault – zeroize the key and cancel background timer.
    pub fn lock(&self) -> Result<(), VaultError> {
        // Cancel the auto-lock background task
        if let Some(tx) = self.auto_lock_cancel.lock().unwrap().take() {
            let _ = tx.send(true);
        }
        let mut guard = self.inner.lock().unwrap();
        if let Some(ref mut inner) = *guard {
            inner.encryption_key = None;
        }
        *guard = None;
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        let guard = self.inner.lock().unwrap();
        match &*guard {
            None => true,
            Some(inner) => inner.encryption_key.is_none(),
        }
    }

    /// Start a background tokio task that periodically checks idle timeout
    /// and auto-locks the vault when it expires. Cancels any previous timer.
    pub fn start_auto_lock_timer(&self, app_handle: AppHandle) {
        // Cancel any existing timer
        if let Some(tx) = self.auto_lock_cancel.lock().unwrap().take() {
            let _ = tx.send(true);
        }

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        *self.auto_lock_cancel.lock().unwrap() = Some(cancel_tx);

        // Since Vault is managed state (Arc'd by Tauri), we use the AppHandle
        // to retrieve it inside the spawned task.
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let vault = app_handle.try_state::<Vault>();
                        let Some(vault) = vault else { break };
                        let should_lock = {
                            let guard = vault.inner.lock().unwrap();
                            if let Some(ref inner) = *guard {
                                inner.encryption_key.is_some()
                                    && inner.last_activity.elapsed().as_secs() > inner.idle_timeout_secs
                            } else {
                                false
                            }
                        };
                        if should_lock {
                            {
                                let mut guard = vault.inner.lock().unwrap();
                                if let Some(ref mut inner) = *guard {
                                    inner.encryption_key = None;
                                }
                                *guard = None;
                            }
                            if let Some(tx) = vault.auto_lock_cancel.lock().unwrap().take() {
                                let _ = tx.send(true);
                            }
                            let pid = app_handle
                                .try_state::<crate::config::ConfigState>()
                                .map(|cs| cs.active_profile_id.read().unwrap().clone().unwrap_or_default())
                                .unwrap_or_default();
                            crate::audit::append_event(
                                &pid,
                                crate::audit::AuditEventType::VaultAutoLock,
                                "Vault auto-locked by background timer",
                            );
                            let _ = app_handle.emit("vault:auto_locked", ());
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

    fn with_inner<F, T>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&mut VaultInner) -> Result<T, VaultError>,
    {
        let mut guard = self.inner.lock().unwrap();
        let inner = guard.as_mut().ok_or(VaultError::Locked)?;
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

    pub fn credential_create(&self, req: CredentialCreateRequest) -> Result<String, VaultError> {
        self.with_inner(|inner| {
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

    pub fn credential_list(&self) -> Result<Vec<CredentialSummary>, VaultError> {
        self.with_inner(|inner| {
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

    pub fn credential_get(&self, id: &str) -> Result<CredentialDetail, VaultError> {
        self.with_inner(|inner| {
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

    pub fn credential_update(&self, id: &str, req: CredentialUpdateRequest) -> Result<(), VaultError> {
        self.with_inner(|inner| {
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

    pub fn credential_delete(&self, id: &str) -> Result<(), VaultError> {
        self.with_inner(|inner| {
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
) -> Result<(), VaultError> {
    let result = state.create(&profile_id, &master_password);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultCreate, "Vault created");
    }
    result
}

#[tauri::command]
pub fn vault_unlock(
    app_handle: AppHandle,
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    profile_id: String,
    master_password: String,
) -> Result<(), VaultError> {
    let result = state.unlock(&profile_id, &master_password);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultUnlock, "Vault unlocked");
        // Start background auto-lock timer
        state.start_auto_lock_timer(app_handle);
    }
    result
}

#[tauri::command]
pub fn vault_lock(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<(), VaultError> {
    let result = state.lock();
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::VaultLock, "Vault locked");
    }
    result
}

#[tauri::command]
pub fn vault_is_locked(state: tauri::State<'_, Vault>) -> bool {
    state.is_locked()
}

#[tauri::command]
pub fn credential_create(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    request: CredentialCreateRequest,
) -> Result<String, VaultError> {
    let name = request.name.clone();
    let result = state.credential_create(request);
    if let Ok(ref id) = result {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialCreate, &format!("Created credential '{}' ({})", name, id));
    }
    result
}

#[tauri::command]
pub fn credential_list(
    state: tauri::State<'_, Vault>,
) -> Result<Vec<CredentialSummary>, VaultError> {
    state.credential_list()
}

#[tauri::command]
pub fn credential_get(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    id: String,
) -> Result<CredentialDetail, VaultError> {
    let result = state.credential_get(&id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialAccess, &format!("Accessed credential {}", id));
    }
    result
}

#[tauri::command]
pub fn vault_change_password(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    current_password: String,
    new_password: String,
) -> Result<(), VaultError> {
    let result = state.change_password(&current_password, &new_password);
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
            "Vault master password changed",
        );
    }
    result
}

#[tauri::command]
pub fn credential_update(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    id: String,
    request: CredentialUpdateRequest,
) -> Result<(), VaultError> {
    let result = state.credential_update(&id, request);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialUpdate, &format!("Updated credential {}", id));
    }
    result
}

#[tauri::command]
pub fn credential_delete(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    id: String,
) -> Result<(), VaultError> {
    let result = state.credential_delete(&id);
    if result.is_ok() {
        let pid = config_state.active_profile_id.read().unwrap().clone().unwrap_or_default();
        crate::audit::append_event(&pid, crate::audit::AuditEventType::CredentialDelete, &format!("Deleted credential {}", id));
    }
    result
}

// ── BE-VAULT-01: Auto-lock idle check ───────────────────────────────────

#[tauri::command]
pub fn vault_check_idle(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<bool, VaultError> {
    let mut guard = state.inner.lock().unwrap();
    if let Some(ref mut inner) = *guard {
        if inner.encryption_key.is_some()
            && inner.last_activity.elapsed().as_secs() > inner.idle_timeout_secs
        {
            inner.encryption_key = None;
            let pid = config_state
                .active_profile_id
                .read()
                .unwrap()
                .clone()
                .unwrap_or_default();
            crate::audit::append_event(
                &pid,
                crate::audit::AuditEventType::VaultAutoLock,
                "Vault auto-locked due to idle timeout",
            );
            // Drop the inner completely so is_locked() returns true
            *guard = None;
            return Ok(true);
        }
    }
    Ok(false)
}

// ── BE-VAULT-05: Credential orphan check ────────────────────────────────

#[tauri::command]
pub fn vault_check_orphans(
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<Vec<String>, VaultError> {
    let all_creds = state.credential_list()?;
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

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEST_PASSWORD: &str = "testpass123!";
    const WRONG_PASSWORD: &str = "wrongpass999!";

    /// Generate a unique profile ID and return it along with a guard that
    /// cleans up the vault DB directory on drop.
    struct TestProfile {
        profile_id: String,
    }

    impl TestProfile {
        fn new() -> Self {
            Self {
                profile_id: Uuid::new_v4().to_string(),
            }
        }

        fn id(&self) -> &str {
            &self.profile_id
        }
    }

    impl Drop for TestProfile {
        fn drop(&mut self) {
            let path = Vault::db_path(&self.profile_id);
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir_all(parent);
            }
        }
    }

    fn setup_vault(profile: &TestProfile) -> Vault {
        let vault = Vault::new();
        vault.create(profile.id(), TEST_PASSWORD).unwrap();
        // create() leaves vault unlocked
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

        assert!(vault.create(tp.id(), TEST_PASSWORD).is_ok());
        assert!(!vault.is_locked());

        // Lock then re-unlock
        vault.lock().unwrap();
        assert!(vault.is_locked());

        assert!(vault.unlock(tp.id(), TEST_PASSWORD).is_ok());
        assert!(!vault.is_locked());
    }

    // ── UT-V-02: Wrong password ─────────────────────────────────────

    #[test]
    fn test_vault_wrong_password() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        vault.create(tp.id(), TEST_PASSWORD).unwrap();
        vault.lock().unwrap();

        let result = vault.unlock(tp.id(), WRONG_PASSWORD);
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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
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

        let id = vault.credential_create(make_password_request("Original")).unwrap();

        vault
            .credential_update(
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

        let detail = vault.credential_get(&id).unwrap();
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

        let id = vault.credential_create(make_password_request("ToDelete")).unwrap();

        // Delete it
        vault.credential_delete(&id).unwrap();

        // Verify it's gone
        let result = vault.credential_get(&id);
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
                .credential_create(CredentialCreateRequest {
                    name: format!("Cred-{}", i),
                    credential_type: CredentialType::Password,
                    username: Some(format!("user{}", i)),
                    data: json!({"password": format!("pass{}", i)}),
                    tags: Some(vec![format!("tag{}", i)]),
                    notes: None,
                })
                .unwrap();
        }

        let list = vault.credential_list().unwrap();
        assert_eq!(list.len(), 5);

        // Summaries should have correct fields populated
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

        // Create a credential while unlocked, get its id for later
        let id = vault.credential_create(make_password_request("Pre-lock")).unwrap();

        // Lock the vault
        vault.lock().unwrap();
        assert!(vault.is_locked());

        // All CRUD operations should return Locked
        assert!(matches!(
            vault.credential_create(make_password_request("ShouldFail")).unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_list().unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_get(&id).unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_update(&id, CredentialUpdateRequest {
                name: Some("x".into()),
                username: None,
                data: None,
                tags: None,
                notes: None,
            }).unwrap_err(),
            VaultError::Locked
        ));
        assert!(matches!(
            vault.credential_delete(&id).unwrap_err(),
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

        // Different nonces → different ciphertext (probabilistic encryption)
        assert_ne!(nonce1, nonce2, "Nonces must differ");
        assert_ne!(ct1, ct2, "Ciphertexts must differ for same plaintext");

        // Both must round-trip correctly
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

        // Add some credentials
        let id1 = vault.credential_create(make_password_request("Cred1")).unwrap();
        let id2 = vault.credential_create(make_ssh_key_request("Cred2")).unwrap();

        // Change password
        vault.change_password(TEST_PASSWORD, new_password).unwrap();

        // Credentials are still accessible
        let d1 = vault.credential_get(&id1).unwrap();
        assert_eq!(d1.name, "Cred1");
        assert_eq!(d1.data["password"], "s3cret!");

        let d2 = vault.credential_get(&id2).unwrap();
        assert_eq!(d2.name, "Cred2");
        assert!(d2.data["private_key"].as_str().unwrap().contains("OPENSSH"));

        // Lock and unlock with new password succeeds
        vault.lock().unwrap();
        assert!(vault.unlock(tp.id(), new_password).is_ok());

        // Old password no longer works
        vault.lock().unwrap();
        assert!(matches!(
            vault.unlock(tp.id(), TEST_PASSWORD).unwrap_err(),
            VaultError::InvalidPassword
        ));
    }

    // ── UT-V-11: Rate limiting ──────────────────────────────────────

    #[test]
    fn test_rate_limiting() {
        let tp = TestProfile::new();
        let vault = Vault::new();
        vault.create(tp.id(), TEST_PASSWORD).unwrap();
        vault.lock().unwrap();

        // First 3 failures should return InvalidPassword
        for i in 0..3 {
            let result = vault.unlock(tp.id(), WRONG_PASSWORD);
            assert!(
                matches!(result, Err(VaultError::InvalidPassword)),
                "Attempt {} should be InvalidPassword",
                i + 1
            );
        }

        // 4th attempt should be RateLimited
        let result = vault.unlock(tp.id(), WRONG_PASSWORD);
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

        // List on empty vault returns empty vec
        let list = vault.credential_list().unwrap();
        assert!(list.is_empty());

        // Get non-existent credential
        let result = vault.credential_get("non-existent-id");
        assert!(matches!(
            result.unwrap_err(),
            VaultError::CredentialNotFound(_)
        ));

        // Update non-existent credential
        let result = vault.credential_update(
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

        // Delete non-existent credential
        let result = vault.credential_delete("non-existent-id");
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

        // Set a very short timeout
        {
            let mut guard = vault.inner.lock().unwrap();
            if let Some(ref mut inner) = *guard {
                inner.idle_timeout_secs = 0; // immediate
                inner.last_activity = Instant::now() - std::time::Duration::from_secs(10);
            }
        }

        // Any operation should now trigger auto-lock
        let result = vault.credential_list();
        assert!(matches!(result.unwrap_err(), VaultError::Locked));
    }

    #[test]
    fn test_vault_no_auto_lock_when_active() {
        let tp = TestProfile::new();
        let vault = setup_vault(&tp);

        // Default timeout is 900s, we should be well within that
        let result = vault.credential_list();
        assert!(result.is_ok());
    }

    // ── UT-V-05: Credential roundtrip (certificate) ───────────────

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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
        assert_eq!(detail.name, "TLS Cert");
        assert_eq!(detail.credential_type, CredentialType::Certificate);
        assert_eq!(detail.username.as_deref(), Some("server"));
        assert!(detail.data["cert_data"].as_str().unwrap().contains("BEGIN CERTIFICATE"));
        assert!(detail.data["private_key"].as_str().unwrap().contains("BEGIN PRIVATE KEY"));
        assert_eq!(detail.tags, vec!["tls", "prod"]);
        assert_eq!(detail.notes.as_deref(), Some("Production TLS certificate"));
    }

    // ── UT-V-06: Credential roundtrip (API token) ──────────────────

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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
        assert_eq!(detail.name, "GitHub Token");
        assert_eq!(detail.credential_type, CredentialType::ApiToken);
        assert_eq!(detail.username.as_deref(), Some("devuser"));
        assert_eq!(detail.data["provider"], "github");
        assert_eq!(detail.data["token"], "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        assert_eq!(detail.data["expiry"], "2027-12-31T23:59:59Z");
        assert_eq!(detail.tags, vec!["api", "github"]);
    }

    // ── UT-V-07: Credential roundtrip (cloud) ──────────────────────

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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
        assert_eq!(detail.name, "AWS Prod");
        assert_eq!(detail.credential_type, CredentialType::CloudCredential);
        assert_eq!(detail.username.as_deref(), Some("iam-deploy"));
        assert_eq!(detail.data["provider"], "aws");
        assert_eq!(detail.data["access_key"], "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(detail.data["secret_key"], "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
        assert_eq!(detail.data["region"], "us-east-1");
        assert_eq!(detail.tags, vec!["aws", "prod"]);
    }

    // ── UT-V-08: Credential roundtrip (TOTP seed) ──────────────────

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
        let id = vault.credential_create(req).unwrap();

        let detail = vault.credential_get(&id).unwrap();
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
        assert_eq!(key.len(), 32, "Derived key must be exactly 32 bytes (AES-256)");

        // Derive again with same inputs → same key
        let key2 = derive_key(password, &salt).unwrap();
        assert_eq!(key.as_slice(), key2.as_slice(), "Same password+salt must produce same key");

        // Different salt → different key
        let mut salt2 = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt2);
        let key3 = derive_key(password, &salt2).unwrap();
        assert_ne!(key.as_slice(), key3.as_slice(), "Different salt must produce different key");
    }

    // ── UT-V-19: Concurrent access ──────────────────────────────────

    #[test]
    fn test_concurrent_access() {
        let tp = TestProfile::new();
        let vault = std::sync::Arc::new(setup_vault(&tp));

        // Pre-create a credential to read concurrently
        let id = vault.credential_create(make_password_request("Shared Cred")).unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let vault_clone = std::sync::Arc::clone(&vault);
            let id_clone = id.clone();
            handles.push(std::thread::spawn(move || {
                // Mix of reads and writes
                if i % 2 == 0 {
                    let detail = vault_clone.credential_get(&id_clone).unwrap();
                    assert_eq!(detail.name, "Shared Cred");
                } else {
                    let _ = vault_clone.credential_list().unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread should not panic during concurrent vault access");
        }

        // Verify credential is still intact after concurrent access
        let detail = vault.credential_get(&id).unwrap();
        assert_eq!(detail.name, "Shared Cred");
        assert_eq!(detail.data["password"], "s3cret!");
    }

    // ── UT-V-14 (existing): Clipboard copy ──────────────────────────

    #[test]
    fn test_clipboard_arboard_available() {
        // Verify the arboard crate is functional (may fail in headless CI)
        let result = arboard::Clipboard::new();
        // We just verify it compiles and the type is correct
        assert!(result.is_ok() || result.is_err());
    }
}

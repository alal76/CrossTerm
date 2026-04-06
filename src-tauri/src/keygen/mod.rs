use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum KeygenError {
    #[error("Key not found: {0}")]
    NotFound(String),
    #[error("Unsupported key type: {0}")]
    UnsupportedKeyType(String),
    #[error("Key generation failed: {0}")]
    Generation(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Key parse error: {0}")]
    Parse(String),
    #[error("SSH connection required for deploy")]
    NoConnection,
}

impl Serialize for KeygenError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub id: String,
    pub filename: String,
    pub key_type: String,
    pub comment: Option<String>,
    pub has_passphrase: bool,
    pub public_key: Option<String>,
    pub fingerprint: Option<String>,
    pub created_at: String,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct KeygenState {
    /// Cached key info from ~/.ssh/
    keys: RwLock<HashMap<String, KeyInfo>>,
}

impl KeygenState {
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn ssh_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
}

fn detect_key_type(filename: &str) -> String {
    if filename.contains("ed25519") {
        "ed25519".to_string()
    } else if filename.contains("ecdsa") {
        "ecdsa".to_string()
    } else if filename.contains("rsa") {
        "rsa".to_string()
    } else if filename.contains("dsa") {
        "dsa".to_string()
    } else {
        "unknown".to_string()
    }
}

fn read_public_key_file(path: &Path) -> Option<String> {
    let pub_path = PathBuf::from(format!("{}.pub", path.display()));
    if pub_path.exists() {
        std::fs::read_to_string(&pub_path).ok()
    } else {
        None
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn keygen_generate(
    state: tauri::State<'_, KeygenState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    key_type: String,
    bits: Option<u32>,
    passphrase: Option<String>,
    comment: String,
) -> Result<KeyInfo, KeygenError> {
    use ssh_key::{Algorithm, LineEnding};
    use ssh_key::private::PrivateKey;

    let algorithm = match key_type.to_lowercase().as_str() {
        "ed25519" => Algorithm::Ed25519,
        "rsa" => {
            let _bits = bits.unwrap_or(4096);
            // ssh-key crate generates RSA keys with default size
            Algorithm::Rsa { hash: None }
        }
        "ecdsa" | "ecdsa-p256" => Algorithm::Ecdsa {
            curve: ssh_key::EcdsaCurve::NistP256,
        },
        other => return Err(KeygenError::UnsupportedKeyType(other.to_string())),
    };

    let private_key = PrivateKey::random(&mut rand::rngs::OsRng, algorithm)
        .map_err(|e| KeygenError::Generation(e.to_string()))?;

    let id = Uuid::new_v4().to_string();
    let short_id = &id[..8];
    let filename = format!("id_{}_{}", key_type.to_lowercase(), short_id);
    let key_path = ssh_dir().join(&filename);
    let pub_path = ssh_dir().join(format!("{}.pub", &filename));

    // Ensure .ssh directory exists with correct permissions
    let ssh_directory = ssh_dir();
    std::fs::create_dir_all(&ssh_directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ssh_directory, std::fs::Permissions::from_mode(0o700))?;
    }

    // Write private key
    let private_pem = if let Some(ref pass) = passphrase {
        private_key
            .encrypt(&mut rand::rngs::OsRng, pass)
            .map_err(|e| KeygenError::Generation(e.to_string()))?
            .to_openssh(LineEnding::LF)
            .map_err(|e| KeygenError::Generation(e.to_string()))?
    } else {
        private_key
            .to_openssh(LineEnding::LF)
            .map_err(|e| KeygenError::Generation(e.to_string()))?
    };
    std::fs::write(&key_path, private_pem.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))?;
    }

    // Write public key
    let public_key = private_key.public_key();
    let mut pub_openssh = public_key
        .to_openssh()
        .map_err(|e| KeygenError::Generation(e.to_string()))?;
    if !comment.is_empty() {
        pub_openssh = format!("{} {}", pub_openssh.trim(), comment);
    }
    std::fs::write(&pub_path, format!("{}\n", pub_openssh))?;

    let fingerprint = public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();

    let info = KeyInfo {
        id: id.clone(),
        filename: filename.clone(),
        key_type: key_type.to_lowercase(),
        comment: Some(comment),
        has_passphrase: passphrase.is_some(),
        public_key: Some(pub_openssh),
        fingerprint: Some(fingerprint),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.keys.write().unwrap().insert(id.clone(), info.clone());

    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::KeygenGenerate,
        &format!("Generated {} key: {}", key_type, filename),
    );

    Ok(info)
}

#[tauri::command]
pub fn keygen_list(
    state: tauri::State<'_, KeygenState>,
) -> Result<Vec<KeyInfo>, KeygenError> {
    let ssh_directory = ssh_dir();
    let mut keys = Vec::new();

    if !ssh_directory.exists() {
        return Ok(keys);
    }

    for entry in std::fs::read_dir(&ssh_directory)? {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Skip public keys, known_hosts, config, authorized_keys, and dotfiles
        if filename.ends_with(".pub")
            || filename == "known_hosts"
            || filename == "known_hosts.old"
            || filename == "config"
            || filename == "authorized_keys"
            || filename.starts_with('.')
        {
            continue;
        }

        // Check if it's a private key by reading first line
        if let Ok(content) = std::fs::read_to_string(&path) {
            let first_line = content.lines().next().unwrap_or("");
            if !first_line.contains("PRIVATE KEY") && !first_line.contains("BEGIN OPENSSH") {
                continue;
            }

            let id = Uuid::new_v4().to_string();
            let public_key = read_public_key_file(&path);
            let has_passphrase = content.contains("ENCRYPTED");

            let info = KeyInfo {
                id: id.clone(),
                filename: filename.clone(),
                key_type: detect_key_type(&filename),
                comment: public_key.as_ref().and_then(|pk| {
                    pk.split_whitespace().nth(2).map(|s| s.to_string())
                }),
                has_passphrase,
                public_key,
                fingerprint: None,
                created_at: entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
                    })
                    .unwrap_or_default(),
            };

            state.keys.write().unwrap().insert(id, info.clone());
            keys.push(info);
        }
    }

    keys.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(keys)
}

#[tauri::command]
pub fn keygen_import(
    state: tauri::State<'_, KeygenState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    path: String,
) -> Result<KeyInfo, KeygenError> {
    let source = PathBuf::from(&path);
    if !source.exists() {
        return Err(KeygenError::NotFound(path.clone()));
    }

    let content = std::fs::read_to_string(&source)?;
    let first_line = content.lines().next().unwrap_or("");
    if !first_line.contains("PRIVATE KEY") && !first_line.contains("BEGIN OPENSSH") {
        return Err(KeygenError::Parse("Not a valid private key file".into()));
    }

    let filename = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Copy to ~/.ssh/ if not already there
    let dest = ssh_dir().join(&filename);
    if !dest.exists() {
        std::fs::create_dir_all(ssh_dir())?;
        std::fs::copy(&source, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
        }
    }

    // Also copy public key if it exists
    let pub_source = PathBuf::from(format!("{}.pub", path));
    if pub_source.exists() {
        let pub_dest = ssh_dir().join(format!("{}.pub", &filename));
        if !pub_dest.exists() {
            std::fs::copy(&pub_source, &pub_dest)?;
        }
    }

    let id = Uuid::new_v4().to_string();
    let public_key = read_public_key_file(&dest);

    let info = KeyInfo {
        id: id.clone(),
        filename: filename.clone(),
        key_type: detect_key_type(&filename),
        comment: public_key.as_ref().and_then(|pk| {
            pk.split_whitespace().nth(2).map(|s| s.to_string())
        }),
        has_passphrase: content.contains("ENCRYPTED"),
        public_key,
        fingerprint: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.keys.write().unwrap().insert(id, info.clone());

    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::KeygenImport,
        &format!("Imported key: {}", filename),
    );

    Ok(info)
}

#[tauri::command]
pub fn keygen_get_public(
    state: tauri::State<'_, KeygenState>,
    key_id: String,
) -> Result<String, KeygenError> {
    let keys = state.keys.read().unwrap();
    let key = keys
        .get(&key_id)
        .ok_or_else(|| KeygenError::NotFound(key_id.clone()))?;

    if let Some(ref pub_key) = key.public_key {
        return Ok(pub_key.clone());
    }

    // Try reading from disk
    let pub_path = ssh_dir().join(format!("{}.pub", key.filename));
    if pub_path.exists() {
        let content = std::fs::read_to_string(&pub_path)?;
        return Ok(content.trim().to_string());
    }

    Err(KeygenError::NotFound(format!(
        "Public key not found for {}",
        key.filename
    )))
}

#[tauri::command]
pub async fn keygen_deploy(
    ssh_state: tauri::State<'_, crate::ssh::SshState>,
    keygen_state: tauri::State<'_, KeygenState>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
    key_id: String,
    connection_id: String,
) -> Result<(), KeygenError> {
    let public_key = {
        let keys = keygen_state.keys.read().unwrap();
        let key = keys
            .get(&key_id)
            .ok_or_else(|| KeygenError::NotFound(key_id.clone()))?;
        key.public_key
            .clone()
            .ok_or_else(|| KeygenError::NotFound("No public key available".into()))?
    };

    let connections = ssh_state.connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or(KeygenError::NoConnection)?
        .clone();

    let conn_locked = conn.lock().await;
    let channel = conn_locked
        .handle
        .channel_open_session()
        .await
        .map_err(|e| KeygenError::Generation(format!("Channel open failed: {}", e)))?;

    // Append public key to remote authorized_keys
    let cmd = format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
        public_key.trim()
    );

    channel
        .exec(true, cmd.as_bytes())
        .await
        .map_err(|e| KeygenError::Generation(format!("Deploy failed: {}", e)))?;

    // Wait for completion
    use russh::ChannelMsg;
    let mut ch = channel;
    loop {
        match ch.wait().await {
            Some(ChannelMsg::ExitStatus { .. })
            | Some(ChannelMsg::Eof)
            | Some(ChannelMsg::Close) => break,
            None => break,
            _ => {}
        }
    }

    let pid = config_state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::KeygenDeploy,
        &format!(
            "Deployed public key {} to connection {}",
            key_id, connection_id
        ),
    );

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keygen_state_new() {
        let state = KeygenState::new();
        let keys = state.keys.read().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_detect_key_type() {
        assert_eq!(detect_key_type("id_ed25519"), "ed25519");
        assert_eq!(detect_key_type("id_rsa"), "rsa");
        assert_eq!(detect_key_type("id_ecdsa"), "ecdsa");
        assert_eq!(detect_key_type("id_dsa"), "dsa");
        assert_eq!(detect_key_type("mykey"), "unknown");
    }

    #[test]
    fn test_ssh_dir_path() {
        let dir = ssh_dir();
        assert!(dir.to_string_lossy().contains(".ssh"));
    }

    #[test]
    fn test_key_info_serialization() {
        let info = KeyInfo {
            id: "key-1".into(),
            filename: "id_ed25519_test".into(),
            key_type: "ed25519".into(),
            comment: Some("test@host".into()),
            has_passphrase: false,
            public_key: Some("ssh-ed25519 AAAA... test@host".into()),
            fingerprint: Some("SHA256:abc123".into()),
            created_at: "2024-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: KeyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "key-1");
        assert_eq!(deserialized.key_type, "ed25519");
        assert_eq!(deserialized.comment, Some("test@host".into()));
    }

    #[test]
    fn test_keygen_error_display() {
        let err = KeygenError::NotFound("key-abc".into());
        assert_eq!(err.to_string(), "Key not found: key-abc");

        let err = KeygenError::UnsupportedKeyType("dsa".into());
        assert_eq!(err.to_string(), "Unsupported key type: dsa");

        let err = KeygenError::NoConnection;
        assert_eq!(err.to_string(), "SSH connection required for deploy");
    }

    #[test]
    fn test_keygen_error_serialize() {
        let err = KeygenError::NoConnection;
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("SSH connection required"));
    }

    #[test]
    fn test_read_public_key_file_nonexistent() {
        let path = PathBuf::from("/tmp/nonexistent_key_abc123");
        let result = read_public_key_file(&path);
        assert!(result.is_none());
    }

    #[test]
    fn test_keygen_generate_ed25519() {
        use ssh_key::{Algorithm, private::PrivateKey};

        let key = PrivateKey::random(&mut rand::rngs::OsRng, Algorithm::Ed25519);
        assert!(key.is_ok(), "Ed25519 key generation should succeed");

        let key = key.unwrap();
        let pub_key = key.public_key();
        let openssh = pub_key.to_openssh();
        assert!(openssh.is_ok(), "Public key should serialize to OpenSSH format");
    }

    #[test]
    fn test_keygen_list_creates_ssh_dir_info() {
        // Test that listing keys from a non-existent dir returns empty
        let state = KeygenState::new();
        // We test the internal logic: if ~/.ssh doesn't exist, it returns empty
        let dir = PathBuf::from("/tmp/nonexistent_ssh_dir_test");
        assert!(!dir.exists());
        // The actual function uses ssh_dir() which is the real ~/.ssh
        // So we just verify the state starts empty
        let keys = state.keys.read().unwrap();
        assert!(keys.is_empty());
    }
}

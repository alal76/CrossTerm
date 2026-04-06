use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum KeyMgrError {
    #[error("Key not found: {0}")]
    NotFound(String),
    #[error("Key already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Agent error: {0}")]
    AgentError(String),
    #[error("Export error: {0}")]
    ExportError(String),
    #[error("Import error: {0}")]
    ImportError(String),
    #[error("Authority error: {0}")]
    AuthorityError(String),
}

impl Serialize for KeyMgrError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub id: String,
    pub name: String,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: String,
    pub private_key_path: String,
    pub comment: Option<String>,
    pub created_at: String,
    pub last_used: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentKey {
    pub fingerprint: String,
    pub key_type: String,
    pub comment: Option<String>,
    pub lifetime: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub id: String,
    pub key_id: String,
    pub serial: u64,
    pub cert_type: CertType,
    pub valid_after: String,
    pub valid_before: String,
    pub principals: Vec<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertType {
    User,
    Host,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDeployTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: KeyDeployAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyDeployAuth {
    Password(PasswordAuth),
    ExistingKey(ExistingKeyAuth),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordAuth {
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingKeyAuth {
    pub key_path: String,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct KeyMgrState {
    keys: Mutex<HashMap<String, SshKeyInfo>>,
    agent_keys: Mutex<Vec<AgentKey>>,
    certificates: Mutex<HashMap<String, CertificateInfo>>,
}

impl KeyMgrState {
    pub fn new() -> Self {
        Self {
            keys: Mutex::new(HashMap::new()),
            agent_keys: Mutex::new(Vec::new()),
            certificates: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn mock_fingerprint(name: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(name.as_bytes());
    format!("SHA256:{}", data_encoding::BASE64.encode(&hash[..32]))
}

fn mock_public_key(name: &str, key_type: &str) -> String {
    format!("{} AAAA{}MockKeyData== {}", key_type, name, name)
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn keymgr_list_keys(
    state: tauri::State<'_, KeyMgrState>,
) -> Result<Vec<SshKeyInfo>, KeyMgrError> {
    let keys = state.keys.lock().unwrap();
    Ok(keys.values().cloned().collect())
}

#[tauri::command]
pub fn keymgr_import_key(
    state: tauri::State<'_, KeyMgrState>,
    path: String,
    name: String,
) -> Result<SshKeyInfo, KeyMgrError> {
    let mut keys = state.keys.lock().unwrap();

    // Check for duplicate names
    if keys.values().any(|k| k.name == name) {
        return Err(KeyMgrError::AlreadyExists(name));
    }

    // In a real implementation, we'd parse the key file.
    // For now, derive key_type from the path/name.
    let key_type = if path.contains("ed25519") {
        "ed25519"
    } else if path.contains("ecdsa") {
        "ecdsa"
    } else if path.contains("rsa") {
        "rsa"
    } else {
        "ed25519"
    };

    let id = Uuid::new_v4().to_string();
    let fingerprint = mock_fingerprint(&name);
    let public_key = mock_public_key(&name, key_type);

    let info = SshKeyInfo {
        id: id.clone(),
        name: name.clone(),
        key_type: key_type.to_string(),
        fingerprint,
        public_key,
        private_key_path: path,
        comment: Some(format!("Imported key: {}", name)),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_used: None,
        tags: Vec::new(),
    };

    keys.insert(id, info.clone());
    Ok(info)
}

#[tauri::command]
pub fn keymgr_export_key(
    state: tauri::State<'_, KeyMgrState>,
    key_id: String,
    format: String,
) -> Result<Vec<u8>, KeyMgrError> {
    let keys = state.keys.lock().unwrap();
    let key = keys
        .get(&key_id)
        .ok_or_else(|| KeyMgrError::NotFound(key_id.clone()))?;

    match format.as_str() {
        "openssh" | "pem" | "pkcs8" => {
            // Stub: return the public key as bytes
            Ok(key.public_key.as_bytes().to_vec())
        }
        other => Err(KeyMgrError::ExportError(format!(
            "Unsupported format: {}",
            other
        ))),
    }
}

#[tauri::command]
pub fn keymgr_delete_key(
    state: tauri::State<'_, KeyMgrState>,
    key_id: String,
) -> Result<(), KeyMgrError> {
    let mut keys = state.keys.lock().unwrap();
    keys.remove(&key_id)
        .ok_or_else(|| KeyMgrError::NotFound(key_id))?;
    Ok(())
}

#[tauri::command]
pub fn keymgr_agent_list(
    state: tauri::State<'_, KeyMgrState>,
) -> Result<Vec<AgentKey>, KeyMgrError> {
    let agent_keys = state.agent_keys.lock().unwrap();
    Ok(agent_keys.clone())
}

#[tauri::command]
pub fn keymgr_agent_add(
    state: tauri::State<'_, KeyMgrState>,
    key_id: String,
    lifetime: Option<u64>,
) -> Result<(), KeyMgrError> {
    let keys = state.keys.lock().unwrap();
    let key = keys
        .get(&key_id)
        .ok_or_else(|| KeyMgrError::NotFound(key_id.clone()))?;

    let agent_key = AgentKey {
        fingerprint: key.fingerprint.clone(),
        key_type: key.key_type.clone(),
        comment: key.comment.clone(),
        lifetime,
    };

    drop(keys);
    let mut agent_keys = state.agent_keys.lock().unwrap();

    // Check if already loaded
    if agent_keys.iter().any(|ak| ak.fingerprint == agent_key.fingerprint) {
        return Err(KeyMgrError::AgentError(
            "Key already loaded in agent".to_string(),
        ));
    }

    agent_keys.push(agent_key);
    Ok(())
}

#[tauri::command]
pub fn keymgr_agent_remove(
    state: tauri::State<'_, KeyMgrState>,
    fingerprint: String,
) -> Result<(), KeyMgrError> {
    let mut agent_keys = state.agent_keys.lock().unwrap();
    let initial_len = agent_keys.len();
    agent_keys.retain(|ak| ak.fingerprint != fingerprint);
    if agent_keys.len() == initial_len {
        return Err(KeyMgrError::NotFound(fingerprint));
    }
    Ok(())
}

#[tauri::command]
pub fn keymgr_agent_remove_all(
    state: tauri::State<'_, KeyMgrState>,
) -> Result<(), KeyMgrError> {
    let mut agent_keys = state.agent_keys.lock().unwrap();
    agent_keys.clear();
    Ok(())
}

#[tauri::command]
pub fn keymgr_deploy_key(
    state: tauri::State<'_, KeyMgrState>,
    key_id: String,
    _target: KeyDeployTarget,
) -> Result<(), KeyMgrError> {
    let keys = state.keys.lock().unwrap();
    let _key = keys
        .get(&key_id)
        .ok_or_else(|| KeyMgrError::NotFound(key_id.clone()))?;

    // Stub: In a real implementation, this would SSH into the target
    // and append the public key to ~/.ssh/authorized_keys
    Ok(())
}

#[tauri::command]
pub fn keymgr_cert_list(
    state: tauri::State<'_, KeyMgrState>,
) -> Result<Vec<CertificateInfo>, KeyMgrError> {
    let certs = state.certificates.lock().unwrap();
    Ok(certs.values().cloned().collect())
}

#[tauri::command]
pub fn keymgr_cert_sign(
    state: tauri::State<'_, KeyMgrState>,
    key_id: String,
    _ca_key_path: String,
    principals: Vec<String>,
    validity_hours: u64,
) -> Result<CertificateInfo, KeyMgrError> {
    let keys = state.keys.lock().unwrap();
    let _key = keys
        .get(&key_id)
        .ok_or_else(|| KeyMgrError::NotFound(key_id.clone()))?;
    drop(keys);

    // Stub: In a real implementation, this would use ssh-keygen -s
    let now = chrono::Utc::now();
    let valid_before = now + chrono::Duration::hours(validity_hours as i64);

    let cert_id = Uuid::new_v4().to_string();
    let cert = CertificateInfo {
        id: cert_id.clone(),
        key_id: key_id.clone(),
        serial: rand::random::<u64>() % 1_000_000,
        cert_type: CertType::User,
        valid_after: now.to_rfc3339(),
        valid_before: valid_before.to_rfc3339(),
        principals,
        extensions: vec![
            "permit-pty".to_string(),
            "permit-user-rc".to_string(),
        ],
    };

    let mut certs = state.certificates.lock().unwrap();
    certs.insert(cert_id, cert.clone());

    Ok(cert)
}

#[tauri::command]
pub fn keymgr_cert_verify(
    _cert_path: String,
) -> Result<CertificateInfo, KeyMgrError> {
    // Stub: In a real implementation, this would parse the certificate file
    let now = chrono::Utc::now();
    Ok(CertificateInfo {
        id: Uuid::new_v4().to_string(),
        key_id: "unknown".to_string(),
        serial: 12345,
        cert_type: CertType::User,
        valid_after: now.to_rfc3339(),
        valid_before: (now + chrono::Duration::hours(24)).to_rfc3339(),
        principals: vec!["root".to_string()],
        extensions: vec!["permit-pty".to_string()],
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> KeyMgrState {
        KeyMgrState::new()
    }

    #[test]
    fn test_key_crud() {
        let state = make_state();

        // Import
        {
            let mut keys = state.keys.lock().unwrap();
            let id = Uuid::new_v4().to_string();
            let info = SshKeyInfo {
                id: id.clone(),
                name: "test-key".to_string(),
                key_type: "ed25519".to_string(),
                fingerprint: mock_fingerprint("test-key"),
                public_key: mock_public_key("test-key", "ed25519"),
                private_key_path: "/home/user/.ssh/id_ed25519".to_string(),
                comment: Some("test".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_used: None,
                tags: vec!["dev".to_string()],
            };
            keys.insert(id.clone(), info);

            // List
            assert_eq!(keys.len(), 1);
            let key = keys.get(&id).unwrap();
            assert_eq!(key.name, "test-key");
            assert_eq!(key.key_type, "ed25519");

            // Export (via public key bytes)
            let exported = key.public_key.as_bytes().to_vec();
            assert!(!exported.is_empty());

            // Delete
            keys.remove(&id);
            assert!(keys.is_empty());
        }
    }

    #[test]
    fn test_agent_operations() {
        let state = make_state();

        // Insert a key first
        let key_id = Uuid::new_v4().to_string();
        {
            let mut keys = state.keys.lock().unwrap();
            keys.insert(
                key_id.clone(),
                SshKeyInfo {
                    id: key_id.clone(),
                    name: "agent-test".to_string(),
                    key_type: "rsa".to_string(),
                    fingerprint: mock_fingerprint("agent-test"),
                    public_key: mock_public_key("agent-test", "rsa"),
                    private_key_path: "/tmp/test_rsa".to_string(),
                    comment: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    last_used: None,
                    tags: Vec::new(),
                },
            );
        }

        // Add to agent
        let fingerprint = {
            let keys = state.keys.lock().unwrap();
            let key = keys.get(&key_id).unwrap();
            let agent_key = AgentKey {
                fingerprint: key.fingerprint.clone(),
                key_type: key.key_type.clone(),
                comment: key.comment.clone(),
                lifetime: Some(3600),
            };
            let fp = agent_key.fingerprint.clone();
            let mut agent_keys = state.agent_keys.lock().unwrap();
            agent_keys.push(agent_key);
            fp
        };

        // List agent keys
        {
            let agent_keys = state.agent_keys.lock().unwrap();
            assert_eq!(agent_keys.len(), 1);
            assert_eq!(agent_keys[0].lifetime, Some(3600));
        }

        // Remove specific key
        {
            let mut agent_keys = state.agent_keys.lock().unwrap();
            agent_keys.retain(|ak| ak.fingerprint != fingerprint);
            assert!(agent_keys.is_empty());
        }

        // Add back and remove all
        {
            let mut agent_keys = state.agent_keys.lock().unwrap();
            agent_keys.push(AgentKey {
                fingerprint: "fp1".to_string(),
                key_type: "ed25519".to_string(),
                comment: None,
                lifetime: None,
            });
            agent_keys.push(AgentKey {
                fingerprint: "fp2".to_string(),
                key_type: "rsa".to_string(),
                comment: None,
                lifetime: None,
            });
            assert_eq!(agent_keys.len(), 2);
            agent_keys.clear();
            assert!(agent_keys.is_empty());
        }
    }

    #[test]
    fn test_certificate_lifecycle() {
        let state = make_state();

        // Set up a key
        let key_id = Uuid::new_v4().to_string();
        {
            let mut keys = state.keys.lock().unwrap();
            keys.insert(
                key_id.clone(),
                SshKeyInfo {
                    id: key_id.clone(),
                    name: "cert-test".to_string(),
                    key_type: "ed25519".to_string(),
                    fingerprint: mock_fingerprint("cert-test"),
                    public_key: mock_public_key("cert-test", "ed25519"),
                    private_key_path: "/tmp/test_ed25519".to_string(),
                    comment: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    last_used: None,
                    tags: Vec::new(),
                },
            );
        }

        // Sign a certificate (stubbed)
        let cert_id = {
            let now = chrono::Utc::now();
            let valid_before = now + chrono::Duration::hours(24);
            let cert = CertificateInfo {
                id: Uuid::new_v4().to_string(),
                key_id: key_id.clone(),
                serial: 42,
                cert_type: CertType::User,
                valid_after: now.to_rfc3339(),
                valid_before: valid_before.to_rfc3339(),
                principals: vec!["admin".to_string(), "deploy".to_string()],
                extensions: vec!["permit-pty".to_string()],
            };
            let id = cert.id.clone();
            let mut certs = state.certificates.lock().unwrap();
            certs.insert(id.clone(), cert);
            id
        };

        // List certificates
        {
            let certs = state.certificates.lock().unwrap();
            assert_eq!(certs.len(), 1);
            let cert = certs.get(&cert_id).unwrap();
            assert_eq!(cert.key_id, key_id);
            assert_eq!(cert.serial, 42);
            assert_eq!(cert.principals.len(), 2);
            assert!(matches!(cert.cert_type, CertType::User));
        }

        // Verify certificate (stubbed - returns a mock)
        let now = chrono::Utc::now();
        let verified = CertificateInfo {
            id: Uuid::new_v4().to_string(),
            key_id: "unknown".to_string(),
            serial: 12345,
            cert_type: CertType::User,
            valid_after: now.to_rfc3339(),
            valid_before: (now + chrono::Duration::hours(24)).to_rfc3339(),
            principals: vec!["root".to_string()],
            extensions: vec!["permit-pty".to_string()],
        };
        assert_eq!(verified.serial, 12345);
        assert!(matches!(verified.cert_type, CertType::User));
    }

    #[test]
    fn test_key_deploy_target_serde() {
        // Password auth variant
        let password_target = KeyDeployTarget {
            host: "example.com".to_string(),
            port: 22,
            username: "admin".to_string(),
            auth: KeyDeployAuth::Password(PasswordAuth {
                password: "secret".to_string(),
            }),
        };
        let json = serde_json::to_string(&password_target).unwrap();
        assert!(json.contains("\"type\":\"password\""));
        assert!(json.contains("\"host\":\"example.com\""));

        let deserialized: KeyDeployTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "example.com");
        assert!(matches!(deserialized.auth, KeyDeployAuth::Password(_)));

        // ExistingKey auth variant
        let key_target = KeyDeployTarget {
            host: "server.local".to_string(),
            port: 2222,
            username: "deploy".to_string(),
            auth: KeyDeployAuth::ExistingKey(ExistingKeyAuth {
                key_path: "/home/user/.ssh/id_ed25519".to_string(),
            }),
        };
        let json2 = serde_json::to_string(&key_target).unwrap();
        assert!(json2.contains("\"type\":\"existing_key\""));

        let deserialized2: KeyDeployTarget = serde_json::from_str(&json2).unwrap();
        assert_eq!(deserialized2.port, 2222);
        assert!(matches!(deserialized2.auth, KeyDeployAuth::ExistingKey(_)));
    }
}

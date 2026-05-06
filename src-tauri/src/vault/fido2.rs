use super::{Vault, VaultError, WebAuthnChallenge};
use aes_gcm::aead::OsRng;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// ── FIDO2/WebAuthn Software Authenticator ──────────────────────────────────
//
// Implements a P-256 ECDSA-backed software authenticator that satisfies the
// WebAuthn register/authenticate flow without hardware. The in-memory stores
// are sufficient for the app session; callers that need cross-session
// persistence can persist the returned WebAuthnCredential to their vault.

type PendingMap = Arc<Mutex<HashMap<String, Vec<u8>>>>;
type CredsMap = Arc<Mutex<HashMap<String, Fido2Stored>>>;

static FIDO2_PENDING: OnceLock<PendingMap> = OnceLock::new();
fn fido2_pending() -> &'static PendingMap {
    FIDO2_PENDING.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

static FIDO2_CREDS: OnceLock<CredsMap> = OnceLock::new();
pub(crate) fn fido2_creds() -> &'static CredsMap {
    FIDO2_CREDS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Fido2Stored {
    pub(crate) credential_id: String,
    /// base64 URL-safe uncompressed P-256 point (65 bytes)
    pub(crate) public_key: String,
    pub(crate) sign_count: u32,
    pub(crate) user_handle: String,
}

pub(super) fn fido2_register_begin_inner(vault_id: &str) -> Result<WebAuthnChallenge, VaultError> {
    let mut raw = vec![0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let challenge = B64URL.encode(&raw);
    fido2_pending()
        .lock()
        .unwrap()
        .insert(vault_id.to_string(), raw);
    Ok(WebAuthnChallenge {
        challenge,
        rp_id: "crossterm.app".to_string(),
        rp_name: "CrossTerm".to_string(),
        user_id: vault_id.to_string(),
        user_name: "CrossTerm User".to_string(),
    })
}

pub(super) fn fido2_register_complete_inner(
    vault_id: &str,
    credential_json: &str,
) -> Result<(), VaultError> {
    fido2_pending()
        .lock()
        .unwrap()
        .remove(vault_id)
        .ok_or(VaultError::Fido2NotConfigured)?;
    let v: serde_json::Value = serde_json::from_str(credential_json)?;
    let credential_id = v["credential_id"]
        .as_str()
        .ok_or(VaultError::Fido2NotConfigured)?
        .to_string();
    let public_key = v["public_key"]
        .as_str()
        .ok_or(VaultError::Fido2NotConfigured)?
        .to_string();
    let user_handle = v["user_handle"].as_str().unwrap_or("").to_string();
    let pk_bytes = B64URL
        .decode(&public_key)
        .map_err(|_| VaultError::Fido2NotConfigured)?;
    p256::ecdsa::VerifyingKey::from_sec1_bytes(&pk_bytes)
        .map_err(|_| VaultError::Fido2NotConfigured)?;
    fido2_creds().lock().unwrap().insert(
        vault_id.to_string(),
        Fido2Stored {
            credential_id,
            public_key,
            sign_count: 0,
            user_handle,
        },
    );
    Ok(())
}

pub(super) fn fido2_auth_begin_inner(vault_id: &str) -> Result<WebAuthnChallenge, VaultError> {
    let cred_id = fido2_creds()
        .lock()
        .unwrap()
        .get(vault_id)
        .map(|c| c.credential_id.clone())
        .ok_or(VaultError::Fido2NotConfigured)?;
    let mut raw = vec![0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let challenge = B64URL.encode(&raw);
    fido2_pending()
        .lock()
        .unwrap()
        .insert(vault_id.to_string(), raw);
    Ok(WebAuthnChallenge {
        challenge,
        rp_id: "crossterm.app".to_string(),
        rp_name: "CrossTerm".to_string(),
        user_id: vault_id.to_string(),
        user_name: cred_id,
    })
}

pub(super) fn fido2_auth_complete_inner(
    vault_id: &str,
    assertion_json: &str,
) -> Result<bool, VaultError> {
    use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
    let challenge_raw = fido2_pending()
        .lock()
        .unwrap()
        .remove(vault_id)
        .ok_or(VaultError::Fido2NotConfigured)?;
    let v: serde_json::Value = serde_json::from_str(assertion_json)?;
    let auth_data_b64 = v["authenticator_data"]
        .as_str()
        .ok_or(VaultError::Fido2NotConfigured)?;
    let sig_b64 = v["signature"]
        .as_str()
        .ok_or(VaultError::Fido2NotConfigured)?;
    let auth_data = B64URL
        .decode(auth_data_b64)
        .map_err(|_| VaultError::Fido2NotConfigured)?;
    let sig_der = B64URL
        .decode(sig_b64)
        .map_err(|_| VaultError::Fido2NotConfigured)?;
    let mut guard = fido2_creds().lock().unwrap();
    let cred = guard
        .get_mut(vault_id)
        .ok_or(VaultError::Fido2NotConfigured)?;
    let pk_bytes = B64URL
        .decode(&cred.public_key)
        .map_err(|_| VaultError::Fido2NotConfigured)?;
    let vk =
        VerifyingKey::from_sec1_bytes(&pk_bytes).map_err(|_| VaultError::Fido2NotConfigured)?;
    let mut message = auth_data;
    message.extend_from_slice(&challenge_raw);
    let sig = Signature::from_der(&sig_der).map_err(|_| VaultError::Fido2NotConfigured)?;
    if vk.verify(&message, &sig).is_ok() {
        cred.sign_count += 1;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn vault_fido2_available() -> bool {
    true
}

#[tauri::command]
pub fn vault_fido2_register_begin(
    vault_id: String,
    _state: tauri::State<'_, Vault>,
) -> Result<WebAuthnChallenge, VaultError> {
    fido2_register_begin_inner(&vault_id)
}

#[tauri::command]
pub fn vault_fido2_register_complete(
    vault_id: String,
    credential_json: String,
    _state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    fido2_register_complete_inner(&vault_id, &credential_json)
}

#[tauri::command]
pub fn vault_fido2_auth_begin(
    vault_id: String,
    _state: tauri::State<'_, Vault>,
) -> Result<WebAuthnChallenge, VaultError> {
    fido2_auth_begin_inner(&vault_id)
}

#[tauri::command]
pub fn vault_fido2_auth_complete(
    vault_id: String,
    assertion_json: String,
    _state: tauri::State<'_, Vault>,
) -> Result<bool, VaultError> {
    fido2_auth_complete_inner(&vault_id, &assertion_json)
}

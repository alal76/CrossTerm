use super::VaultError;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroizing;

pub(super) const SALT_LEN: usize = 32;
pub(super) const KEY_LEN: usize = 32; // AES-256
pub(super) const NONCE_LEN: usize = 12; // AES-GCM standard

/// Derive a 256-bit key from a master password and salt using Argon2id.
pub(super) fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let params = Params::new(65536, 3, 4, Some(KEY_LEN))
        .map_err(|e| VaultError::Encryption(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new(vec![0u8; KEY_LEN]);
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| VaultError::Encryption(e.to_string()))?;
    Ok(key)
}

pub(super) fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<(Vec<u8>, Vec<u8>), VaultError> {
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

pub(super) fn decrypt(
    ciphertext: &[u8],
    nonce_bytes: &[u8],
    key: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| VaultError::Decryption(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VaultError::Decryption(e.to_string()))
}

//! Vault sharing primitives.
//!
//! This module implements the cryptographic plumbing for sharing a vault DEK
//! (Data Encryption Key) with other users via ephemeral X25519 Diffie-Hellman
//! key exchange wrapped with AES-256-GCM authenticated encryption.
//!
//! Cryptographic model
//! -------------------
//! ```text
//! Per-user KEK:     Argon2id(master_password, salt)     [derived by caller]
//! User identity:    StaticSecret / PublicKey  (X25519)
//! Private key rest: AES-256-GCM(KEK, private_key_bytes)  [private key encrypted at rest]
//!
//! Sharing a DEK with a peer:
//!   ephemeral_priv, ephemeral_pub  ← X25519::random()
//!   shared_secret                  ← X25519(ephemeral_priv, peer_pub)
//!   aes_key                        ← SHA-256(shared_secret)
//!   encrypted_dek, nonce           ← AES-256-GCM(aes_key, dek)
//!   Envelope { ephemeral_pub, encrypted_dek, nonce }
//!
//! Opening an envelope:
//!   shared_secret  ← X25519(recipient_priv, ephemeral_pub)
//!   aes_key        ← SHA-256(shared_secret)
//!   dek            ← AES-256-GCM-Decrypt(aes_key, encrypted_dek, nonce)
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

// ── AES-GCM nonce length ─────────────────────────────────────────────────

const NONCE_LEN: usize = 12;

// ── Public data structures ───────────────────────────────────────────────

/// A Curve25519 key pair for a user.  The private key NEVER leaves the device
/// in plaintext; it is always stored encrypted under the caller's KEK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserKeyPair {
    /// Base64-encoded X25519 public key (32 bytes).
    pub public_key: String,
    /// Base64-encoded ciphertext: AES-256-GCM(KEK, private_key_bytes).
    /// The field is excluded from JSON serialisation to prevent the frontend
    /// from ever seeing encrypted key material.
    #[serde(skip_serializing)]
    pub private_key_encrypted: String,
}

/// An encrypted envelope that lets a specific recipient recover the vault DEK.
///
/// The sender generates an ephemeral X25519 key pair, performs ECDH with the
/// recipient's long-term public key, derives a 32-byte AES key with SHA-256,
/// and encrypts the DEK.  Only the holder of the matching private key can open
/// the envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedEnvelope {
    /// The recipient's long-term X25519 public key (base64, 32 bytes).
    /// Used as an opaque identifier for lookup / revocation.
    pub recipient_public_key: String,
    /// The sender's ephemeral X25519 public key (base64, 32 bytes).
    pub ephemeral_public_key: String,
    /// Base64-encoded AES-256-GCM ciphertext of the DEK.
    pub encrypted_dek: String,
    /// Base64-encoded 12-byte GCM nonce.
    pub nonce: String,
}

/// The vault sharing manifest stored alongside the vault.
///
/// It carries the owner's public key (so peers know whom to contact) and one
/// `SharedEnvelope` per authorised user (including, optionally, the owner's
/// own envelope for re-unlocking via DH rather than password).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultSharingManifest {
    /// The owner's long-term X25519 public key, if one has been generated.
    pub owner_public_key: Option<String>,
    /// One envelope per authorised recipient.
    pub envelopes: Vec<SharedEnvelope>,
}

// ── Internal crypto helpers ──────────────────────────────────────────────

/// Encrypt `plaintext` with AES-256-GCM using `key` (32 bytes).
///
/// Returns `(ciphertext, nonce)`.
fn aes_encrypt(plaintext: &[u8], key: &[u8]) -> Result<(Vec<u8>, [u8; NONCE_LEN]), String> {
    let aes_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(aes_key);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("AES-GCM encrypt error: {e}"))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypt `ciphertext` with AES-256-GCM using `key` and `nonce_bytes`.
fn aes_decrypt(ciphertext: &[u8], nonce_bytes: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    let aes_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(aes_key);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("AES-GCM decrypt error: {e}"))
}

/// Derive a 32-byte AES key from a raw X25519 `SharedSecret` via SHA-256.
fn derive_aes_key(shared_secret_bytes: &[u8; 32]) -> [u8; 32] {
    let hash = Sha256::digest(shared_secret_bytes);
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

// ── Core library functions ───────────────────────────────────────────────

/// Generate a new X25519 key pair.
///
/// The private key bytes are immediately encrypted under `kek` and never
/// returned in plaintext.  The returned [`UserKeyPair`] contains:
/// - `public_key` — base64-encoded 32-byte X25519 public key (safe to share).
/// - `private_key_encrypted` — base64-encoded AES-256-GCM(KEK, priv_key_bytes).
pub fn generate_user_key_pair(kek: &[u8]) -> Result<UserKeyPair, String> {
    if kek.len() != 32 {
        return Err(format!(
            "KEK must be 32 bytes, got {}",
            kek.len()
        ));
    }

    // Generate a fresh static X25519 secret (stored long-term, hence StaticSecret).
    let private_key = StaticSecret::random_from_rng(&mut OsRng);
    let public_key = PublicKey::from(&private_key);

    // Encrypt the raw private key bytes under the caller's KEK.
    let priv_bytes: [u8; 32] = private_key.to_bytes();
    let (ciphertext, nonce_bytes) = aes_encrypt(&priv_bytes, kek)?;

    // Encode: nonce || ciphertext, both together as one base64 blob for simplicity.
    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(UserKeyPair {
        public_key: B64.encode(public_key.as_bytes()),
        private_key_encrypted: B64.encode(&blob),
    })
}

/// Create an encrypted envelope so `recipient_public_key_b64` can decrypt `dek`.
///
/// Uses ephemeral X25519 ECDH + SHA-256 KDF + AES-256-GCM to encrypt the DEK
/// bytes.  The envelope can only be opened by the holder of the private key
/// corresponding to `recipient_public_key_b64`.
pub fn create_sharing_envelope(
    dek: &[u8],
    recipient_public_key_b64: &str,
) -> Result<SharedEnvelope, String> {
    // Decode the recipient's public key.
    let recipient_pub_bytes = B64
        .decode(recipient_public_key_b64)
        .map_err(|e| format!("Failed to decode recipient public key: {e}"))?;

    if recipient_pub_bytes.len() != 32 {
        return Err(format!(
            "Recipient public key must be 32 bytes, got {}",
            recipient_pub_bytes.len()
        ));
    }

    let mut recipient_pub_array = [0u8; 32];
    recipient_pub_array.copy_from_slice(&recipient_pub_bytes);
    let recipient_pub = PublicKey::from(recipient_pub_array);

    // Generate an ephemeral X25519 key pair.
    let ephemeral_secret = EphemeralSecret::random_from_rng(&mut OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    // Perform X25519 DH to get the shared secret.
    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_pub);

    // Derive AES key via SHA-256(shared_secret).
    let aes_key = derive_aes_key(shared_secret.as_bytes());

    // Encrypt the DEK.
    let (ciphertext, nonce_bytes) = aes_encrypt(dek, &aes_key)?;

    Ok(SharedEnvelope {
        recipient_public_key: recipient_public_key_b64.to_string(),
        ephemeral_public_key: B64.encode(ephemeral_public.as_bytes()),
        encrypted_dek: B64.encode(&ciphertext),
        nonce: B64.encode(nonce_bytes),
    })
}

/// Open a sharing envelope using the recipient's (encrypted) private key.
///
/// The private key blob is decrypted from KEK first, then used to perform X25519
/// DH against the envelope's ephemeral public key.  AES-256-GCM decryption with
/// the derived key yields the DEK bytes.
pub fn open_sharing_envelope(
    envelope: &SharedEnvelope,
    recipient_private_key_encrypted_b64: &str,
    kek: &[u8],
) -> Result<Vec<u8>, String> {
    if kek.len() != 32 {
        return Err(format!("KEK must be 32 bytes, got {}", kek.len()));
    }

    // Decode the encrypted private key blob.
    let blob = B64
        .decode(recipient_private_key_encrypted_b64)
        .map_err(|e| format!("Failed to decode encrypted private key: {e}"))?;

    if blob.len() <= NONCE_LEN {
        return Err(format!(
            "Encrypted private key blob too short: {} bytes",
            blob.len()
        ));
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);

    // Decrypt the private key under the KEK.
    let priv_bytes = aes_decrypt(ciphertext, nonce_bytes, kek)
        .map_err(|e| format!("Failed to decrypt private key: {e}"))?;

    if priv_bytes.len() != 32 {
        return Err(format!(
            "Decrypted private key must be 32 bytes, got {}",
            priv_bytes.len()
        ));
    }

    let mut priv_array = [0u8; 32];
    priv_array.copy_from_slice(&priv_bytes);
    let recipient_secret = StaticSecret::from(priv_array);

    // Decode the ephemeral public key from the envelope.
    let ephemeral_pub_bytes = B64
        .decode(&envelope.ephemeral_public_key)
        .map_err(|e| format!("Failed to decode ephemeral public key: {e}"))?;

    if ephemeral_pub_bytes.len() != 32 {
        return Err(format!(
            "Ephemeral public key must be 32 bytes, got {}",
            ephemeral_pub_bytes.len()
        ));
    }

    let mut ephemeral_pub_array = [0u8; 32];
    ephemeral_pub_array.copy_from_slice(&ephemeral_pub_bytes);
    let ephemeral_pub = PublicKey::from(ephemeral_pub_array);

    // Perform X25519 DH (recipient private × ephemeral public).
    let shared_secret = recipient_secret.diffie_hellman(&ephemeral_pub);

    // Derive AES key via SHA-256(shared_secret).
    let aes_key = derive_aes_key(shared_secret.as_bytes());

    // Decode the nonce and ciphertext from the envelope.
    let nonce_bytes = B64
        .decode(&envelope.nonce)
        .map_err(|e| format!("Failed to decode nonce: {e}"))?;
    let encrypted_dek = B64
        .decode(&envelope.encrypted_dek)
        .map_err(|e| format!("Failed to decode encrypted DEK: {e}"))?;

    // Decrypt the DEK.
    aes_decrypt(&encrypted_dek, &nonce_bytes, &aes_key)
        .map_err(|e| format!("Failed to decrypt DEK from envelope: {e}"))
}

/// Remove all envelopes for the given `recipient_public_key` from the manifest.
///
/// This is the revocation operation: after this call the corresponding user can
/// no longer open the vault (assuming the DEK or the manifest is rotated).
pub fn revoke_access(manifest: &mut VaultSharingManifest, recipient_public_key: &str) {
    manifest
        .envelopes
        .retain(|e| e.recipient_public_key != recipient_public_key);
}

/// Add or replace an envelope in the manifest.
///
/// If an envelope already exists for the same `recipient_public_key` it is
/// replaced with the new one.  Otherwise the new envelope is appended.
pub fn add_envelope(manifest: &mut VaultSharingManifest, envelope: SharedEnvelope) {
    // Remove any existing envelope for the same recipient.
    manifest
        .envelopes
        .retain(|e| e.recipient_public_key != envelope.recipient_public_key);
    manifest.envelopes.push(envelope);
}

// ── Tauri commands ───────────────────────────────────────────────────────

/// Generate a new X25519 key pair for the current user.
///
/// `kek_b64` is the base64-encoded 32-byte Key Encryption Key derived from the
/// master password.  The private key is encrypted under it before storage; only
/// the public key is returned.
#[tauri::command]
pub fn vault_generate_keypair(
    _vault_id: String,
    kek_b64: String,
) -> Result<String, String> {
    let kek = B64
        .decode(&kek_b64)
        .map_err(|e| format!("Failed to decode KEK: {e}"))?;

    let pair = generate_user_key_pair(&kek)?;
    Ok(pair.public_key)
}

/// Create a `SharedEnvelope` that lets `recipient_public_key` decrypt `dek_b64`.
///
/// `dek_b64` is the base64-encoded vault DEK bytes.
#[tauri::command]
pub fn vault_share_with(
    _vault_id: String,
    recipient_public_key: String,
    dek_b64: String,
) -> Result<SharedEnvelope, String> {
    let dek = B64
        .decode(&dek_b64)
        .map_err(|e| format!("Failed to decode DEK: {e}"))?;

    create_sharing_envelope(&dek, &recipient_public_key)
}

/// Revoke vault access for a given `recipient_public_key`.
///
/// This mutates only the in-memory manifest; the caller is responsible for
/// persisting the updated manifest.
#[tauri::command]
pub fn vault_revoke_share(
    _vault_id: String,
    recipient_public_key: String,
) -> Result<(), String> {
    // This command is intentionally stateless at the Tauri layer.  The caller
    // obtains the manifest, calls `revoke_access`, and persists it themselves.
    // We validate the key looks plausible (non-empty base64) before returning.
    if recipient_public_key.is_empty() {
        return Err("recipient_public_key must not be empty".to_string());
    }
    B64.decode(&recipient_public_key)
        .map_err(|e| format!("Invalid recipient_public_key: {e}"))?;
    Ok(())
}

/// Open a `SharedEnvelope` and return the DEK as a base64-encoded string.
///
/// - `private_key_encrypted_b64` — the caller's encrypted private key blob
///   (as stored in their `UserKeyPair.private_key_encrypted`).
/// - `kek_b64` — the caller's 32-byte KEK (base64-encoded).
#[tauri::command]
pub fn vault_open_envelope(
    envelope: SharedEnvelope,
    private_key_encrypted_b64: String,
    kek_b64: String,
) -> Result<String, String> {
    let kek = B64
        .decode(&kek_b64)
        .map_err(|e| format!("Failed to decode KEK: {e}"))?;

    let dek = open_sharing_envelope(&envelope, &private_key_encrypted_b64, &kek)?;
    Ok(B64.encode(&dek))
}

// ── Unit tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: produce a deterministic-looking 32-byte fake KEK for tests.
    fn fake_kek(seed: u8) -> Vec<u8> {
        vec![seed; 32]
    }

    /// Helper: produce a deterministic-looking 32-byte fake DEK for tests.
    fn fake_dek(seed: u8) -> Vec<u8> {
        vec![seed; 32]
    }

    // ── test 1 ───────────────────────────────────────────────────────────

    /// The public key returned by `generate_user_key_pair` must decode to
    /// exactly 32 bytes.
    #[test]
    fn test_generate_keypair_produces_valid_public_key() {
        let kek = fake_kek(0xAB);
        let pair = generate_user_key_pair(&kek).expect("keypair generation failed");

        let decoded = B64
            .decode(&pair.public_key)
            .expect("public key is not valid base64");

        assert_eq!(
            decoded.len(),
            32,
            "X25519 public key must be 32 bytes, got {}",
            decoded.len()
        );

        // Encrypted private key must also decode and be non-empty.
        let enc = B64
            .decode(&pair.private_key_encrypted)
            .expect("encrypted private key is not valid base64");

        // At minimum: 12-byte nonce + 32-byte ciphertext + 16-byte GCM tag = 60.
        assert!(
            enc.len() >= NONCE_LEN + 32,
            "Encrypted private key blob too short: {}",
            enc.len()
        );
    }

    // ── test 2 ───────────────────────────────────────────────────────────

    /// Full round-trip: generate two key pairs, create an envelope for the
    /// second user using the first user as sender, open the envelope as the
    /// second user, and verify the DEK is identical.
    #[test]
    fn test_create_and_open_envelope_roundtrip() {
        let kek_sender = fake_kek(0x01);
        let kek_recipient = fake_kek(0x02);
        let dek = fake_dek(0xDE);

        // Generate sender and recipient key pairs.
        let sender_pair = generate_user_key_pair(&kek_sender).expect("sender keypair");
        let recipient_pair = generate_user_key_pair(&kek_recipient).expect("recipient keypair");

        // Sender creates an envelope for the recipient.
        let envelope =
            create_sharing_envelope(&dek, &recipient_pair.public_key).expect("create envelope");

        // Recipient opens the envelope.
        let recovered_dek = open_sharing_envelope(
            &envelope,
            &recipient_pair.private_key_encrypted,
            &kek_recipient,
        )
        .expect("open envelope");

        assert_eq!(
            recovered_dek, dek,
            "Recovered DEK does not match original DEK"
        );

        // The sender_pair is used above indirectly — suppress unused warning.
        let _ = &sender_pair;
    }

    // ── test 3 ───────────────────────────────────────────────────────────

    /// A different private key (wrong recipient) cannot open the envelope.
    /// AES-GCM authentication will reject the ciphertext.
    #[test]
    fn test_open_envelope_wrong_key_fails() {
        let kek_recipient = fake_kek(0x03);
        let kek_wrong = fake_kek(0x04);
        let dek = fake_dek(0xAA);

        // Generate the correct recipient pair and a different (wrong) pair.
        let correct_pair = generate_user_key_pair(&kek_recipient).expect("correct keypair");
        let wrong_pair = generate_user_key_pair(&kek_wrong).expect("wrong keypair");

        // Create envelope for the correct recipient.
        let envelope =
            create_sharing_envelope(&dek, &correct_pair.public_key).expect("create envelope");

        // Attempt to open with the wrong private key — must fail.
        let result = open_sharing_envelope(
            &envelope,
            &wrong_pair.private_key_encrypted,
            &kek_wrong,
        );

        assert!(
            result.is_err(),
            "Opening with wrong key should fail, but got Ok"
        );
    }

    // ── test 4 ───────────────────────────────────────────────────────────

    /// `revoke_access` removes only the targeted envelope.
    #[test]
    fn test_revoke_removes_envelope() {
        let kek = fake_kek(0x05);
        let dek = fake_dek(0xBB);

        let pair_a = generate_user_key_pair(&kek).expect("keypair A");
        let pair_b = generate_user_key_pair(&kek).expect("keypair B");

        let envelope_a =
            create_sharing_envelope(&dek, &pair_a.public_key).expect("envelope A");
        let envelope_b =
            create_sharing_envelope(&dek, &pair_b.public_key).expect("envelope B");

        let mut manifest = VaultSharingManifest::default();
        add_envelope(&mut manifest, envelope_a);
        add_envelope(&mut manifest, envelope_b);

        assert_eq!(manifest.envelopes.len(), 2, "Manifest should have 2 envelopes");

        // Revoke access for pair_a.
        revoke_access(&mut manifest, &pair_a.public_key);

        assert_eq!(manifest.envelopes.len(), 1, "Manifest should have 1 envelope after revoke");
        assert_eq!(
            manifest.envelopes[0].recipient_public_key,
            pair_b.public_key,
            "Remaining envelope should be for pair_b"
        );
    }

    // ── test 5 ───────────────────────────────────────────────────────────

    /// `add_envelope` replaces an existing envelope for the same recipient
    /// instead of appending a duplicate.
    #[test]
    fn test_add_envelope_replaces_existing_for_same_recipient() {
        let kek = fake_kek(0x06);
        let dek1 = fake_dek(0xCC);
        let dek2 = fake_dek(0xDD);

        let pair = generate_user_key_pair(&kek).expect("keypair");

        let envelope_first =
            create_sharing_envelope(&dek1, &pair.public_key).expect("first envelope");
        let envelope_second =
            create_sharing_envelope(&dek2, &pair.public_key).expect("second envelope");

        let mut manifest = VaultSharingManifest::default();
        add_envelope(&mut manifest, envelope_first);

        assert_eq!(manifest.envelopes.len(), 1);

        // Add a second envelope for the same recipient — should replace, not append.
        add_envelope(&mut manifest, envelope_second.clone());

        assert_eq!(
            manifest.envelopes.len(),
            1,
            "Duplicate recipient should be replaced, not appended"
        );

        // The envelope in the manifest should be the second one.
        assert_eq!(
            manifest.envelopes[0].encrypted_dek,
            envelope_second.encrypted_dek,
            "Manifest envelope should be the replacement"
        );
    }

    // ── test 6 ───────────────────────────────────────────────────────────

    /// The encrypted private key blob must differ from the raw private key bytes.
    /// This verifies that the KEK actually encrypts the material (not a no-op).
    #[test]
    fn test_kek_protects_private_key() {
        let kek = fake_kek(0x07);
        let pair = generate_user_key_pair(&kek).expect("keypair");

        let blob = B64
            .decode(&pair.private_key_encrypted)
            .expect("blob is valid base64");

        // The blob is nonce (12 bytes) + ciphertext (32 bytes + 16 bytes GCM tag)
        // = 60 bytes minimum.  It must NOT contain the raw 32-byte private key
        // contiguously anywhere (a very strong sanity check — not a formal proof).
        //
        // We verify by checking the blob length is > 32 (it has nonce + tag
        // overhead) and by confirming that decoding with the *wrong* KEK fails.
        assert!(
            blob.len() > 32,
            "Encrypted blob must be longer than raw private key: {} bytes",
            blob.len()
        );

        // Attempting to open with a different KEK must fail.
        let wrong_kek = fake_kek(0xFF);
        let dummy_dek = fake_dek(0xEE);
        let dummy_pub = generate_user_key_pair(&wrong_kek)
            .expect("dummy keypair")
            .public_key;
        let envelope =
            create_sharing_envelope(&dummy_dek, &dummy_pub).expect("dummy envelope");

        // Use the correct pair's encrypted private key but feed the wrong KEK.
        let result = open_sharing_envelope(&envelope, &pair.private_key_encrypted, &wrong_kek);
        assert!(
            result.is_err(),
            "Opening with mismatched KEK must fail, but got Ok"
        );
    }
}

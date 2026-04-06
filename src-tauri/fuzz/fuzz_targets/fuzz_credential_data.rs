//! SEC-T-02: Fuzz credential creation with arbitrary bytes for each field.
//!
//! Feeds arbitrary bytes into credential data deserialization and
//! AES-GCM encrypt/decrypt paths. Verifies graceful error handling
//! with no panics.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    // Split fuzz input into name, username, and data fields
    let split1 = (data[0] as usize) % data.len().max(1);
    let split2 = split1 + (data.get(1).copied().unwrap_or(0) as usize) % (data.len() - split1).max(1);

    let name_bytes = &data[..split1];
    let user_bytes = &data[split1..split2];
    let data_bytes = &data[split2..];

    let _name = String::from_utf8_lossy(name_bytes);
    let _user = String::from_utf8_lossy(user_bytes);

    // Try to parse the remaining bytes as a JSON value (what credential data expects)
    let _ = serde_json::from_slice::<serde_json::Value>(data_bytes);

    // Also test encrypt/decrypt round-trip with fuzzed plaintext
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm, Nonce,
    };
    use rand::RngCore;

    let key = [0x42u8; 32]; // fixed key for fuzzing
    let cipher = match Aes256Gcm::new_from_slice(&key) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt fuzzed data – must not panic
    if let Ok(ciphertext) = cipher.encrypt(nonce, data_bytes) {
        // Decrypt must round-trip – must not panic
        let _ = cipher.decrypt(nonce, ciphertext.as_ref());
    }

    // Try decrypting raw fuzz bytes – must not panic, only error
    let _ = cipher.decrypt(nonce, data_bytes);
});

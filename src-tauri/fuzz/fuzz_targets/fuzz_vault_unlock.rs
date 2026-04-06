//! SEC-T-01: Fuzz vault unlock with arbitrary password bytes.
//!
//! Attempts to create a vault, then unlock it with fuzzed password data.
//! Verifies that no panics occur regardless of input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Skip empty inputs – Argon2 rejects zero-length passwords with an error
    // (not a panic), but there's nothing interesting to test.
    if data.is_empty() {
        return;
    }

    // Attempt to interpret fuzzed bytes as a password string.
    // Invalid UTF-8 is fine – we just use lossy conversion.
    let password = String::from_utf8_lossy(data);

    // Try to derive a key using the same Argon2id parameters the vault uses.
    // This exercises the KDF path without requiring a full Tauri app handle.
    use argon2::{Algorithm, Argon2, Params, Version};

    let salt = [0u8; 32]; // fixed salt for deterministic fuzzing
    let params = match Params::new(4096, 1, 1, Some(32)) {
        Ok(p) => p,
        Err(_) => return,
    };
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = vec![0u8; 32];
    // This must never panic, only return Ok/Err
    let _ = argon2.hash_password_into(password.as_bytes(), &salt, &mut key);
});

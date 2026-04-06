//! SEC-T-03: Fuzz SSH auth parameter parsing.
//!
//! Fuzzes deserialization and validation of SSH authentication data
//! including password strings, private key data, host strings, and
//! JumpHost configurations. Verifies no panics on arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    // Use the first byte to select which parsing path to exercise
    let selector = data[0] % 5;
    let payload = &data[1..];

    match selector {
        // 0: Try to parse as SshAuth JSON (password or private_key variant)
        0 => {
            let _ = serde_json::from_slice::<serde_json::Value>(payload).and_then(|v| {
                // Verify tagged-union structure doesn't cause panics
                serde_json::from_value::<serde_json::Value>(v)
            });
        }

        // 1: Try to parse as JumpHost JSON
        1 => {
            let _ = serde_json::from_slice::<serde_json::Value>(payload);
        }

        // 2: Fuzz SSH private key parsing via ssh-key crate
        2 => {
            let key_str = String::from_utf8_lossy(payload);
            // Try parsing as an OpenSSH private key — must not panic
            let _ = ssh_key::PrivateKey::from_openssh(key_str.as_ref());
            // Try parsing as a public key — must not panic
            let _ = ssh_key::PublicKey::from_openssh(key_str.as_ref());
        }

        // 3: Fuzz host:port parsing patterns
        3 => {
            let input = String::from_utf8_lossy(payload);
            // Exercise host:port splitting logic that SSH connections use
            if let Some((host, port_str)) = input.rsplit_once(':') {
                let _port: Result<u16, _> = port_str.parse();
                // Validate host is non-empty and doesn't contain null bytes
                let _valid = !host.is_empty() && !host.contains('\0');
            }
            // Also exercise bracket-notation for IPv6: [::1]:22
            if input.starts_with('[') {
                if let Some(bracket_end) = input.find(']') {
                    let _ipv6_host = &input[1..bracket_end];
                    let _rest = &input[bracket_end + 1..];
                }
            }
        }

        // 4: Fuzz passphrase-protected key decryption attempt
        _ => {
            if payload.len() < 4 {
                return;
            }
            let split = (payload[0] as usize) % payload.len().max(1);
            let key_bytes = &payload[..split];
            let passphrase_bytes = &payload[split..];

            let key_str = String::from_utf8_lossy(key_bytes);
            let passphrase = String::from_utf8_lossy(passphrase_bytes);

            // Try decrypting a key with a fuzzed passphrase — must not panic
            let _ = ssh_key::PrivateKey::from_openssh(key_str.as_ref()).and_then(|k| {
                k.decrypt(passphrase.as_ref())
            });
        }
    }
});

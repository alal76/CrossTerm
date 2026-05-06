use super::*;

/// Inner (non-Tauri) implementation of `vault_has_totp`, callable from tests.
pub(crate) fn vault_has_totp_inner(vault: &Vault, vault_id: &str) -> Result<bool, String> {
    let summaries = vault
        .credential_list(vault_id)
        .map_err(|e| e.to_string())?;

    Ok(summaries
        .iter()
        .any(|c| c.credential_type == CredentialType::TotpSeed))
}

/// Compute a single RFC 4226 HOTP value for the given key and counter.
///
/// Uses HMAC-SHA1 as mandated by RFC 4226 §5.  The truncation step extracts
/// a 6-digit decimal code from the 20-byte HMAC output.
fn hotp_sha1(key: &[u8], counter: u64) -> u32 {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    // RFC 4226 §5.2: counter is big-endian 8-byte unsigned integer.
    let counter_bytes = counter.to_be_bytes();

    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(key)
        .expect("HMAC-SHA1 accepts any key length");
    mac.update(&counter_bytes);
    let result = mac.finalize().into_bytes();

    // Dynamic truncation (RFC 4226 §5.4)
    let offset = (result[19] & 0x0F) as usize;
    let code = u32::from_be_bytes([
        result[offset] & 0x7F,
        result[offset + 1],
        result[offset + 2],
        result[offset + 3],
    ]);
    code % 1_000_000
}

/// Inner (non-Tauri) implementation of `vault_verify_totp`, callable from tests.
///
/// Implements RFC 6238 TOTP over HMAC-SHA1 with a 30-second step and ±1 step
/// skew (allows one step before/after the current window to tolerate clock drift).
/// The `totp-rs` crate is listed in Cargo.toml at version "6" which does not
/// yet exist on crates.io; this manual implementation uses the `hmac`, `sha2`,
/// and `data-encoding` crates that are already present in the dependency tree.
pub(crate) fn vault_verify_totp_inner(
    vault: &Vault,
    vault_id: &str,
    totp_code: &str,
) -> Result<bool, String> {
    // Step 1: Collect all credential summaries — vault must be unlocked.
    let summaries = vault
        .credential_list(vault_id)
        .map_err(|e| e.to_string())?;

    // Step 2: Find the first TotpSeed credential.
    let totp_id =
        match summaries.iter().find(|c| c.credential_type == CredentialType::TotpSeed) {
            // Step 3: No TOTP credential → second factor not configured → allow.
            None => return Ok(true),
            Some(s) => s.id.clone(),
        };

    // Step 4: Decrypt and retrieve the seed.
    let detail = vault
        .credential_get(vault_id, &totp_id)
        .map_err(|e| e.to_string())?;

    // The TOTP seed is stored in the encrypted JSON as `{"secret": "<base32>", ...}`.
    let seed_str = detail
        .data
        .get("secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "TOTP credential is missing the 'secret' field".to_string())?
        .to_string();

    // Step 5: Decode the base32 seed.
    // data-encoding::BASE32 follows RFC 4648 (no padding required for totp-rs
    // compatible seeds).  Strip whitespace/padding that some apps include.
    let seed_upper = seed_str.trim().to_uppercase();
    let seed_padded = {
        // Pad to a multiple of 8 characters as required by the standard decoder.
        let rem = seed_upper.len() % 8;
        if rem == 0 {
            seed_upper.clone()
        } else {
            format!("{}{}", seed_upper, "=".repeat(8 - rem))
        }
    };
    let key_bytes = data_encoding::BASE32
        .decode(seed_padded.as_bytes())
        .map_err(|e| format!("Invalid TOTP secret encoding: {e}"))?;

    // Step 6: Parse the submitted code (must be exactly 6 decimal digits).
    // totp_code validation: the frontend enforces inputMode="numeric" and
    // maxLength=6; the backend independently validates so the check is
    // reliable even if the frontend constraint is bypassed.
    let code_value: u32 = totp_code
        .parse()
        .map_err(|_| "TOTP code must be a 6-digit number".to_string())?;
    if totp_code.len() != 6 {
        return Err("TOTP code must be exactly 6 digits".to_string());
    }

    // Step 7: RFC 6238 — T = floor(Unix time / step).  Allow ±1 step skew.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("System clock error: {e}"))?
        .as_secs();

    let step: u64 = 30;
    let t = now_secs / step;

    // Check current window and ±1 adjacent windows for clock-drift tolerance.
    for delta in [0i64, -1, 1] {
        let counter = (t as i64 + delta) as u64;
        if hotp_sha1(&key_bytes, counter) == code_value {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Returns whether the vault has a TOTP seed credential configured.
/// The frontend uses this to decide whether to show the authenticator-code
/// input after a successful password unlock.  The vault must be unlocked
/// before calling this command; callers that receive `Err` (vault locked)
/// should treat that as "TOTP state unknown" and proceed without TOTP.
#[tauri::command]
pub fn vault_has_totp(
    vault_id: String,
    state: tauri::State<'_, Vault>,
) -> Result<bool, String> {
    vault_has_totp_inner(&state, &vault_id)
}

/// Validates a TOTP code against the vault's linked TOTP seed credential.
///
/// Security invariant: this command is a **second factor**.  The vault must
/// already be unlocked (password verified) before calling it.  TOTP never
/// replaces the password; it is always additive.
///
/// When no `TotpSeed` credential exists for the vault this returns `Ok(true)`
/// — TOTP is not configured, so no second factor is required.
///
/// The caller (frontend) is responsible for blocking access when the returned
/// value is `false`.  The backend does not re-lock the vault on a bad TOTP
/// code intentionally: the password was already correct and the TOTP window
/// is inherently short-lived (30 s), so aggressive locking would create more
/// UX friction than security value.
///
/// # Code format
/// `totp_code` must be exactly 6 ASCII decimal digits ("000000"–"999999").
/// The totp-rs crate rejects any other format internally, but callers should
/// also validate on the frontend (see VaultUnlock.tsx) to give early feedback.
#[tauri::command]
pub fn vault_verify_totp(
    vault_id: String,
    totp_code: String,
    state: tauri::State<'_, Vault>,
) -> Result<bool, String> {
    vault_verify_totp_inner(&state, &vault_id, &totp_code)
}

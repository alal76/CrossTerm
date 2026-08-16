use super::*;

/// Real native biometric integration (LocalAuthentication.framework on macOS,
/// Windows Hello on Windows) is not implemented yet — see the TODOs in
/// `vault_unlock_biometric` below. This used to unconditionally report `true`
/// on macOS/Windows regardless of that, so the "Use Touch ID"/"Use Windows
/// Hello" button always appeared and then always failed when clicked.
/// Reporting `false` here (matching the FIDO2/YubiKey stub's already-accepted
/// pattern) keeps the button from appearing until real integration lands,
/// rather than showing a button that can never succeed.
#[tauri::command]
pub fn vault_biometric_available() -> Result<bool, VaultError> {
    Ok(false)
}

#[tauri::command]
pub fn vault_unlock_biometric(state: tauri::State<'_, Vault>) -> Result<bool, VaultError> {
    let _guard = state.open_vaults.lock().unwrap();

    // Platform-specific biometric integration stubs:
    // - macOS: LocalAuthentication.framework via objc2 or swift bridge
    // - Windows: Windows Hello via webauthn-authenticator-rs
    // - Linux: polkit or libfido2

    #[cfg(target_os = "macos")]
    {
        // TODO: Integrate with LocalAuthentication.framework
        return Err(VaultError::BiometricUnavailable);
    }

    #[cfg(target_os = "windows")]
    {
        // TODO: Integrate with Windows Hello
        return Err(VaultError::BiometricUnavailable);
    }

    #[cfg(target_os = "linux")]
    {
        return Err(VaultError::BiometricUnavailable);
    }

    #[allow(unreachable_code)]
    Err(VaultError::BiometricUnavailable)
}

#[tauri::command]
pub fn vault_biometric_enroll(
    master_password: String,
    vault_id: String,
    state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    let guard = state.open_vaults.lock().unwrap();
    let inner_ref = guard.get(&vault_id).ok_or(VaultError::Locked)?;
    if inner_ref.encryption_key.is_none() {
        return Err(VaultError::Locked);
    }
    // In a real implementation, this would:
    // 1. Derive the vault key from master_password
    // 2. Store it in Keychain (macOS) / Credential Manager (Windows) with biometric protection
    // 3. Mark the vault as biometric-enabled
    let _ = master_password; // Silence unused warning
    Ok(())
}

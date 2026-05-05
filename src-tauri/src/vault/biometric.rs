use super::*;

#[tauri::command]
pub fn vault_biometric_available() -> Result<bool, VaultError> {
    #[cfg(target_os = "macos")]
    return Ok(true); // Touch ID may be available

    #[cfg(target_os = "windows")]
    return Ok(true); // Windows Hello may be available

    #[cfg(target_os = "linux")]
    return Ok(false); // Generally not available on desktop Linux

    #[allow(unreachable_code)]
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

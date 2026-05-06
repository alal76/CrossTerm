use super::{Vault, VaultError};
use tauri::AppHandle;

#[tauri::command]
pub fn vault_os_store_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows", target_os = "linux"))
}

#[tauri::command]
pub fn vault_os_store_save(
    master_password: String,
    vault_id: String,
    state: tauri::State<'_, Vault>,
) -> Result<(), VaultError> {
    let guard = state.open_vaults.lock().unwrap();
    let inner_ref = guard.get(&vault_id).ok_or(VaultError::Locked)?;
    if inner_ref.encryption_key.is_none() {
        return Err(VaultError::Locked);
    }

    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    entry
        .set_password(&master_password)
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn vault_os_store_retrieve(
    vault_id: String,
    app_handle: AppHandle,
    state: tauri::State<'_, Vault>,
    config_state: tauri::State<'_, crate::config::ConfigState>,
) -> Result<(), VaultError> {
    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    let password = entry
        .get_password()
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;

    let result = state.unlock(&vault_id, &password);
    if result.is_ok() {
        let pid = config_state
            .active_profile_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default();
        crate::audit::append_event(
            &pid,
            crate::audit::AuditEventType::VaultUnlock,
            "Vault unlocked via OS credential store",
        );
        state.start_auto_lock_timer(&vault_id, app_handle);
    }
    result
}

#[tauri::command]
pub fn vault_os_store_delete(vault_id: String) -> Result<(), VaultError> {
    let service = format!("crossterm.vault.{}", vault_id);
    let entry = keyring::Entry::new(&service, "master_password")
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| VaultError::OsStoreError(e.to_string()))?;
    Ok(())
}

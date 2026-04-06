mod audit;
mod config;
mod keygen;
mod sftp;
mod ssh;
mod terminal;
#[cfg(feature = "integration")]
pub mod vault;
#[cfg(not(feature = "integration"))]
mod vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .manage(vault::Vault::new())
        .manage(config::ConfigState::new())
        .manage(terminal::TerminalManager::new())
        .manage(ssh::SshState::new())
        .manage(sftp::SftpState::new())
        .manage(keygen::KeygenState::new())
        .invoke_handler(tauri::generate_handler![
            // Vault
            vault::vault_create,
            vault::vault_unlock,
            vault::vault_lock,
            vault::vault_is_locked,
            vault::vault_change_password,
            vault::vault_check_idle,
            vault::vault_check_orphans,
            vault::vault_clipboard_copy,
            vault::credential_create,
            vault::credential_list,
            vault::credential_get,
            vault::credential_update,
            vault::credential_delete,
            // Config / Profile
            config::profile_list,
            config::profile_create,
            config::profile_get,
            config::profile_update,
            config::profile_delete,
            config::profile_switch,
            config::profile_export,
            config::profile_import,
            config::session_list,
            config::session_create,
            config::session_get,
            config::session_update,
            config::session_delete,
            config::session_search,
            config::session_duplicate,
            config::session_import_ssh_config,
            config::session_bulk_connect,
            config::session_list_by_group,
            config::settings_get,
            config::settings_update,
            config::settings_get_effective,
            config::config_is_portable_mode,
            // Audit
            audit::audit_log_list,
            audit::audit_log_export_csv,
            // Terminal
            terminal::terminal_create,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            terminal::terminal_list,
            terminal::terminal_start_logging,
            terminal::terminal_stop_logging,
            // SSH
            ssh::ssh_connect,
            ssh::ssh_disconnect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_list_connections,
            ssh::ssh_port_forward_add,
            ssh::ssh_port_forward_remove,
            ssh::ssh_exec,
            ssh::ssh_forget_host_key,
            ssh::ssh_generate_key,
            ssh::ssh_list_keys,
            // SFTP
            sftp::sftp_open,
            sftp::sftp_close,
            sftp::sftp_list,
            sftp::sftp_stat,
            sftp::sftp_mkdir,
            sftp::sftp_rmdir,
            sftp::sftp_delete,
            sftp::sftp_rename,
            sftp::sftp_read_file,
            sftp::sftp_write_file,
            sftp::sftp_upload,
            sftp::sftp_download,
            sftp::sftp_scp_upload,
            sftp::sftp_scp_download,
            sftp::sftp_upload_throttled,
            sftp::sftp_download_throttled,
            // Keygen
            keygen::keygen_generate,
            keygen::keygen_list,
            keygen::keygen_import,
            keygen::keygen_get_public,
            keygen::keygen_deploy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

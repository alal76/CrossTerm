mod android;
mod audit;
mod cloud;
mod config;
mod editor;
mod ftp;
mod keygen;
mod keymgr;
mod l10n;
mod macros;
mod network;
mod notifications;
mod plugin_rt;
mod rdp;
mod recording;
mod security;
mod serial;
mod sftp;
mod snippets;
mod ssh;
mod sync;
mod telnet;
mod terminal;
#[cfg(feature = "integration")]
pub mod vault;
#[cfg(not(feature = "integration"))]
mod vault;
mod vnc;
mod window;

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
        .manage(snippets::SnippetState::new())
        .manage(notifications::NotificationState::new())
        .manage(keymgr::KeyMgrState::new())
        .manage(l10n::L10nState::new())
        .manage(security::SecurityState::new())
        .manage(cloud::CloudState::new())
        .manage(ftp::FtpState::new())
        .manage(network::NetworkState::new())
        .manage(rdp::RdpState::new())
        .manage(recording::RecordingState::new())
        .manage(serial::SerialState::new())
        .manage(sync::SyncState::new())
        .manage(telnet::TelnetState::new())
        .manage(vnc::VncState::new())
        .manage(editor::EditorState::new())
        .manage(macros::MacroState::new())
        .manage(plugin_rt::PluginState::new())
        .manage(android::AndroidState::new())
        .invoke_handler(tauri::generate_handler![
            // Vault
            vault::vault_create,
            vault::vault_unlock,
            vault::vault_lock,
            vault::vault_lock_all,
            vault::vault_is_locked,
            vault::vault_list,
            vault::vault_delete,
            vault::vault_share,
            vault::vault_unshare,
            vault::vault_change_password,
            vault::vault_check_idle,
            vault::vault_check_orphans,
            vault::vault_clipboard_copy,
            // Vault: Biometric (BE-VAULT-07)
            vault::vault_biometric_available,
            vault::vault_unlock_biometric,
            vault::vault_biometric_enroll,
            // Vault: OS Credential Store (BE-VAULT-08)
            vault::vault_os_store_available,
            vault::vault_os_store_save,
            vault::vault_os_store_retrieve,
            vault::vault_os_store_delete,
            // Vault: FIDO2/WebAuthn (BE-VAULT-09)
            vault::vault_fido2_available,
            vault::vault_fido2_register_begin,
            vault::vault_fido2_register_complete,
            vault::vault_fido2_auth_begin,
            vault::vault_fido2_auth_complete,
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
            ssh::ssh_discover,
            ssh::ssh_drain_buffer,
            ssh::ssh_disconnect,
            ssh::ssh_auth_respond,
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
            // Snippets
            snippets::snippet_create,
            snippets::snippet_list,
            snippets::snippet_get,
            snippets::snippet_update,
            snippets::snippet_delete,
            snippets::snippet_search,
            // Notifications
            notifications::notification_list,
            notifications::notification_dismiss,
            notifications::notification_clear_all,
            notifications::notification_add,
            // Config extras
            config::shell_integration_install,
            // Window
            window::window_create_for_tab,
            window::window_close,
            window::window_list,
            // Key Manager
            keymgr::keymgr_list_keys,
            keymgr::keymgr_import_key,
            keymgr::keymgr_export_key,
            keymgr::keymgr_delete_key,
            keymgr::keymgr_agent_list,
            keymgr::keymgr_agent_add,
            keymgr::keymgr_agent_remove,
            keymgr::keymgr_agent_remove_all,
            keymgr::keymgr_deploy_key,
            keymgr::keymgr_cert_list,
            keymgr::keymgr_cert_sign,
            keymgr::keymgr_cert_verify,
            // Localisation
            l10n::l10n_list_locales,
            l10n::l10n_get_locale,
            l10n::l10n_set_locale,
            l10n::l10n_get_translations,
            l10n::l10n_set_custom_translation,
            l10n::l10n_export_translations,
            l10n::l10n_import_translations,
            l10n::l10n_get_completeness,
            l10n::l10n_detect_system_locale,
            // Security
            security::security_audit_log,
            security::security_audit_list,
            security::security_audit_search,
            security::security_check_rate_limit,
            security::security_get_config,
            security::security_set_config,
            security::security_cert_pin,
            security::security_cert_verify,
            security::security_cert_list_pins,
            security::security_clear_audit_log,
            // Cloud
            cloud::cloud_detect_clis,
            cloud::cloud_get_asset_tree,
            cloud::aws::cloud_aws_list_profiles,
            cloud::aws::cloud_aws_switch_profile,
            cloud::aws::cloud_aws_sso_login,
            cloud::aws::cloud_aws_list_ec2,
            cloud::aws::cloud_aws_ssm_start,
            cloud::aws::cloud_aws_list_s3_buckets,
            cloud::aws::cloud_aws_list_s3_objects,
            cloud::aws::cloud_aws_cloudwatch_tail,
            cloud::aws::cloud_aws_ecs_exec,
            cloud::aws::cloud_aws_lambda_invoke,
            cloud::aws::cloud_aws_cost_summary,
            cloud::azure::cloud_azure_list_subscriptions,
            cloud::azure::cloud_azure_set_subscription,
            cloud::azure::cloud_azure_login,
            cloud::azure::cloud_azure_list_vms,
            cloud::azure::cloud_azure_bastion_connect,
            cloud::azure::cloud_azure_cloud_shell,
            cloud::azure::cloud_azure_list_storage,
            cloud::azure::cloud_azure_log_analytics_query,
            cloud::gcp::cloud_gcp_list_configs,
            cloud::gcp::cloud_gcp_activate_config,
            cloud::gcp::cloud_gcp_list_instances,
            cloud::gcp::cloud_gcp_iap_tunnel,
            cloud::gcp::cloud_gcp_list_buckets,
            cloud::gcp::cloud_gcp_list_objects,
            cloud::gcp::cloud_gcp_cloud_shell,
            cloud::gcp::cloud_gcp_log_tail,
            // FTP
            ftp::ftp_connect,
            ftp::ftp_disconnect,
            ftp::ftp_list,
            ftp::ftp_upload,
            ftp::ftp_download,
            ftp::ftp_mkdir,
            ftp::ftp_delete,
            ftp::ftp_rename,
            // Network
            network::network_local_subnets,
            network::network_scan_start,
            network::network_scan_results,
            network::network_scan_save_as_sessions,
            network::network_explore_start,
            network::network_wol_send,
            network::network_tunnel_create,
            network::network_tunnel_remove,
            network::network_tunnel_list,
            network::network_tunnel_toggle,
            network::network_fileserver_start,
            network::network_fileserver_stop,
            network::network_fileserver_list,
            network::network_wifi_scan,
            network::network_aircrack_check,
            network::network_aircrack_accept_disclaimer,
            network::network_aircrack_interfaces,
            network::network_aircrack_monitor_start,
            network::network_aircrack_monitor_stop,
            network::network_aircrack_scan_start,
            network::network_aircrack_deauth,
            network::network_aircrack_capture_handshake,
            network::network_aircrack_crack_start,
            network::network_aircrack_audit_log,
            network::network_aircrack_stop_all,
            // RDP
            rdp::rdp_connect,
            rdp::rdp_disconnect,
            rdp::rdp_resize,
            rdp::rdp_send_key,
            rdp::rdp_send_mouse,
            rdp::rdp_clipboard_sync,
            rdp::rdp_screenshot,
            rdp::rdp_list_connections,
            rdp::rdp_configure_redirection,
            rdp::rdp_send_ctrl_alt_del,
            // Recording
            recording::recording_start,
            recording::recording_stop,
            recording::recording_append,
            recording::recording_list,
            recording::recording_get,
            recording::recording_delete,
            recording::recording_playback_start,
            recording::recording_playback_seek,
            recording::recording_playback_set_speed,
            recording::recording_export,
            // Serial
            serial::serial_list_ports,
            serial::serial_connect,
            serial::serial_disconnect,
            serial::serial_write,
            serial::serial_set_baud,
            serial::serial_set_dtr,
            serial::serial_set_rts,
            // Sync
            sync::sync_export,
            sync::sync_import,
            sync::sync_get_status,
            // Telnet
            telnet::telnet_connect,
            telnet::telnet_disconnect,
            telnet::telnet_write,
            telnet::telnet_resize,
            // VNC
            vnc::vnc_connect,
            vnc::vnc_disconnect,
            vnc::vnc_send_key,
            vnc::vnc_send_mouse,
            vnc::vnc_set_encoding,
            vnc::vnc_clipboard_send,
            vnc::vnc_set_view_only,
            vnc::vnc_screenshot,
            vnc::vnc_set_scaling,
            vnc::vnc_list_connections,
            // Editor
            editor::editor_open,
            editor::editor_save,
            editor::editor_close,
            editor::editor_list_open,
            editor::editor_get_content,
            editor::editor_detect_language,
            editor::editor_diff,
            editor::editor_diff_content,
            editor::editor_search,
            editor::editor_replace,
            // Macros
            macros::macro_create,
            macros::macro_update,
            macros::macro_delete,
            macros::macro_list,
            macros::macro_get,
            macros::macro_execute,
            macros::macro_cancel,
            macros::macro_pause,
            macros::macro_resume,
            macros::expect_rule_create,
            macros::expect_rule_delete,
            macros::expect_rule_list,
            macros::expect_rule_toggle,
            // Plugin Runtime
            plugin_rt::plugin_scan,
            plugin_rt::plugin_load,
            plugin_rt::plugin_unload,
            plugin_rt::plugin_enable,
            plugin_rt::plugin_disable,
            plugin_rt::plugin_get_info,
            plugin_rt::plugin_list,
            plugin_rt::plugin_install,
            plugin_rt::plugin_uninstall,
            plugin_rt::plugin_send_event,
            // Plugin API Extensions
            plugin_rt::plugin_register_hook,
            plugin_rt::plugin_unregister_hook,
            plugin_rt::plugin_kv_get,
            plugin_rt::plugin_kv_set,
            plugin_rt::plugin_kv_delete,
            plugin_rt::plugin_http_request,
            plugin_rt::plugin_get_sandbox_config,
            plugin_rt::plugin_set_sandbox_config,
            plugin_rt::plugin_load_wasm,
            // Macro Extensions
            macros::macro_broadcast,
            macros::macro_export,
            macros::macro_import,
            // Cloud Extensions
            cloud::azure::cloud_azure_storage_browse,
            cloud::azure::cloud_azure_aks_get_credentials,
            cloud::azure::cloud_azure_aks_exec,
            cloud::gcp::cloud_gcp_gke_get_credentials,
            cloud::gcp::cloud_gcp_gke_exec,
            // SFTP Extensions
            sftp::sftp_preview,
            sftp::sftp_sync_compare,
            sftp::sftp_sync_execute,
            // RDP Recording
            rdp::rdp_start_recording,
            rdp::rdp_stop_recording,
            // Security Extensions
            security::security_plugin_kv_verify_isolation,
            // Android
            android::android_start_foreground_service,
            android::android_stop_foreground_service,
            android::android_create_notification_channel,
            android::android_is_foreground_active,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

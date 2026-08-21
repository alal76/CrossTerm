use serde::Serialize;
use thiserror::Error;

/// Structured error returned by every Tauri command.
/// Serializes to { "code": "...", "message": "...", "detail": "..." }
/// so the frontend can branch on `error.code` instead of parsing strings.
#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "snake_case", tag = "code")]
#[allow(dead_code)]
pub enum AppError {
    #[error("Authentication failed")]
    AuthFailed { message: String },
    #[error("Host unreachable")]
    HostUnreachable { message: String },
    #[error("Host key changed — possible MITM attack")]
    HostKeyChanged { message: String, fingerprint: String },
    #[error("Connection refused")]
    ConnectionRefused { message: String },
    #[error("Connection timed out")]
    ConnectionTimeout { message: String },
    #[error("Vault is locked")]
    VaultLocked { message: String },
    #[error("Wrong master password")]
    VaultWrongPassword { message: String },
    #[error("Vault not found")]
    VaultNotFound { message: String },
    #[error("Rate limited")]
    RateLimited { message: String, retry_after_secs: u64 },
    #[error("Credential not found")]
    CredentialNotFound { message: String, id: String },
    #[error("Permission denied")]
    PermissionDenied { message: String },
    #[error("Not found")]
    NotFound { message: String },
    #[error("IO error")]
    IoError { message: String },
    #[error("Invalid input")]
    InvalidInput { message: String },
    #[error("Internal error")]
    Internal { message: String },
}

#[allow(dead_code)]
impl AppError {
    pub fn internal(msg: impl Into<String>) -> Self {
        AppError::Internal { message: msg.into() }
    }
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        AppError::InvalidInput { message: msg.into() }
    }
}

/// Conversion from the vault's own error type.
impl From<crate::vault::VaultError> for AppError {
    fn from(e: crate::vault::VaultError) -> Self {
        use crate::vault::VaultError;
        match e {
            VaultError::InvalidPassword => AppError::VaultWrongPassword { message: e.to_string() },
            VaultError::Locked => AppError::VaultLocked { message: e.to_string() },
            VaultError::NotFound => AppError::VaultNotFound { message: e.to_string() },
            VaultError::RateLimited(secs) => AppError::RateLimited { message: e.to_string(), retry_after_secs: secs },
            VaultError::CredentialNotFound(ref id) => AppError::CredentialNotFound { message: e.to_string(), id: id.clone() },
            _ => AppError::Internal { message: e.to_string() },
        }
    }
}

/// Conversion from the SSH module's own error type.
impl From<crate::ssh::SshError> for AppError {
    fn from(e: crate::ssh::SshError) -> Self {
        use crate::ssh::SshError;
        match e {
            SshError::AuthFailed => AppError::AuthFailed { message: e.to_string() },
            SshError::HostKeyChanged(ref host) => AppError::HostKeyChanged {
                message: e.to_string(), fingerprint: host.clone(),
            },
            SshError::ConnectionFailed(msg) => {
                if msg.contains("refused") || msg.contains("Connection refused") {
                    AppError::ConnectionRefused { message: msg }
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    AppError::ConnectionTimeout { message: msg }
                } else {
                    AppError::Internal { message: msg }
                }
            }
            SshError::NotFound(id) => AppError::NotFound { message: format!("Connection not found: {id}") },
            SshError::Io(msg) => AppError::IoError { message: msg },
            _ => AppError::Internal { message: e.to_string() },
        }
    }
}

/// Conversion from the network module's error type.
impl From<crate::network::NetworkError> for AppError {
    fn from(e: crate::network::NetworkError) -> Self {
        use crate::network::NetworkError;
        match e {
            NetworkError::InvalidCidr(s) => AppError::InvalidInput { message: format!("Invalid CIDR: {s}") },
            NetworkError::PortInUse(p) => AppError::InvalidInput { message: format!("Port {p} is already in use") },
            NetworkError::Io(msg) => AppError::IoError { message: msg },
            _ => AppError::Internal { message: e.to_string() },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serializes_with_code() {
        let e = AppError::VaultWrongPassword { message: "bad password".into() };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "vault_wrong_password");
        assert_eq!(json["message"], "bad password");
    }

    #[test]
    fn test_auth_failed_code() {
        let e = AppError::AuthFailed { message: "denied".into() };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "auth_failed");
    }

    #[test]
    fn test_rate_limited_includes_retry_secs() {
        let e = AppError::RateLimited { message: "slow down".into(), retry_after_secs: 30 };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["retry_after_secs"], 30);
    }

    #[test]
    fn test_from_vault_invalid_password() {
        let ve = crate::vault::VaultError::InvalidPassword;
        let ae: AppError = ve.into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "vault_wrong_password");
    }

    #[test]
    fn test_internal_helper() {
        let e = AppError::internal("something broke");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "internal");
    }

    #[test]
    fn test_invalid_input_helper() {
        let e = AppError::invalid_input("bad field");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "invalid_input");
        assert_eq!(json["message"], "bad field");
    }

    #[test]
    fn test_host_key_changed_includes_fingerprint() {
        let e = AppError::HostKeyChanged {
            message: "changed".into(),
            fingerprint: "SHA256:abc".into(),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "host_key_changed");
        assert_eq!(json["fingerprint"], "SHA256:abc");
    }

    #[test]
    fn test_all_simple_variants_serialize_expected_codes() {
        let cases: Vec<(AppError, &str)> = vec![
            (AppError::HostUnreachable { message: "x".into() }, "host_unreachable"),
            (AppError::ConnectionRefused { message: "x".into() }, "connection_refused"),
            (AppError::ConnectionTimeout { message: "x".into() }, "connection_timeout"),
            (AppError::VaultLocked { message: "x".into() }, "vault_locked"),
            (AppError::VaultWrongPassword { message: "x".into() }, "vault_wrong_password"),
            (AppError::VaultNotFound { message: "x".into() }, "vault_not_found"),
            (AppError::PermissionDenied { message: "x".into() }, "permission_denied"),
            (AppError::NotFound { message: "x".into() }, "not_found"),
            (AppError::IoError { message: "x".into() }, "io_error"),
            (AppError::InvalidInput { message: "x".into() }, "invalid_input"),
            (AppError::Internal { message: "x".into() }, "internal"),
        ];
        for (err, expected_code) in cases {
            let json = serde_json::to_value(&err).unwrap();
            assert_eq!(json["code"], expected_code);
        }
    }

    #[test]
    fn test_credential_not_found_includes_id() {
        let e = AppError::CredentialNotFound { message: "x".into(), id: "cred-1".into() };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "credential_not_found");
        assert_eq!(json["id"], "cred-1");
    }

    #[test]
    fn test_from_vault_error_all_mapped_variants() {
        let ae: AppError = crate::vault::VaultError::Locked.into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "vault_locked");

        let ae: AppError = crate::vault::VaultError::NotFound.into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "vault_not_found");

        let ae: AppError = crate::vault::VaultError::RateLimited(42).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "rate_limited");
        assert_eq!(json["retry_after_secs"], 42);

        let ae: AppError = crate::vault::VaultError::CredentialNotFound("cred-1".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "credential_not_found");
        assert_eq!(json["id"], "cred-1");
    }

    #[test]
    fn test_from_vault_error_fallback_to_internal() {
        for ve in [
            crate::vault::VaultError::AlreadyUnlocked,
            crate::vault::VaultError::AlreadyExists,
            crate::vault::VaultError::Encryption("x".into()),
            crate::vault::VaultError::Decryption("x".into()),
        ] {
            let ae: AppError = ve.into();
            assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "internal");
        }
    }

    #[test]
    fn test_from_ssh_error_auth_and_not_found() {
        let ae: AppError = crate::ssh::SshError::AuthFailed.into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "auth_failed");

        let ae: AppError = crate::ssh::SshError::NotFound("conn-1".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["message"], "Connection not found: conn-1");
    }

    #[test]
    fn test_from_ssh_error_host_key_changed() {
        let ae: AppError = crate::ssh::SshError::HostKeyChanged("example.com".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "host_key_changed");
        assert_eq!(json["fingerprint"], "example.com");
    }

    #[test]
    fn test_from_ssh_error_connection_failed_classifies_message() {
        let ae: AppError = crate::ssh::SshError::ConnectionFailed("Connection refused".into()).into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "connection_refused");

        let ae: AppError = crate::ssh::SshError::ConnectionFailed("operation timed out".into()).into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "connection_timeout");

        let ae: AppError = crate::ssh::SshError::ConnectionFailed("something else broke".into()).into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "internal");
    }

    #[test]
    fn test_from_ssh_error_io_and_fallback() {
        let ae: AppError = crate::ssh::SshError::Io("disk full".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "io_error");
        assert_eq!(json["message"], "disk full");

        let ae: AppError = crate::ssh::SshError::Channel("chan broke".into()).into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "internal");
    }

    #[test]
    fn test_from_network_error_variants() {
        let ae: AppError = crate::network::NetworkError::InvalidCidr("10.0.0.0/99".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "invalid_input");
        assert!(json["message"].as_str().unwrap().contains("10.0.0.0/99"));

        let ae: AppError = crate::network::NetworkError::PortInUse(8080).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "invalid_input");
        assert!(json["message"].as_str().unwrap().contains("8080"));

        let ae: AppError = crate::network::NetworkError::Io("read failed".into()).into();
        let json = serde_json::to_value(&ae).unwrap();
        assert_eq!(json["code"], "io_error");
        assert_eq!(json["message"], "read failed");

        let ae: AppError = crate::network::NetworkError::ScanNotFound("scan-1".into()).into();
        assert_eq!(serde_json::to_value(&ae).unwrap()["code"], "internal");
    }
}

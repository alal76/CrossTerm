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
}

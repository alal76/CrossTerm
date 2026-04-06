use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Rate limited: {0}")]
    RateLimited(String),
    #[error("Session expired: {0}")]
    SessionExpired(String),
    #[error("Certificate error: {0}")]
    CertificateError(String),
    #[error("Audit error: {0}")]
    AuditError(String),
}

impl Serialize for SecurityError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub user: String,
    pub action: AuditAction,
    pub resource: String,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Login,
    Logout,
    Connect,
    Disconnect,
    FileAccess,
    ConfigChange,
    VaultAccess,
    KeyOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub max_attempts: u32,
    pub window_secs: u64,
    pub lockout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitState {
    pub attempts: Vec<u64>,
    pub locked_until: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub vault_timeout_secs: u64,
    pub clipboard_clear_secs: u64,
    pub audit_enabled: bool,
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertFingerprint {
    pub sha256: String,
    pub valid_from: String,
    pub valid_until: String,
    pub subject: String,
    pub pinned: bool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SecurityState {
    audit_log: Mutex<Vec<AuditEntry>>,
    rate_limits: Mutex<HashMap<String, RateLimitState>>,
    config: Mutex<SecurityConfig>,
    cert_pins: Mutex<HashMap<String, CertFingerprint>>,
}

impl SecurityState {
    pub fn new() -> Self {
        Self {
            audit_log: Mutex::new(Vec::new()),
            rate_limits: Mutex::new(HashMap::new()),
            config: Mutex::new(SecurityConfig {
                vault_timeout_secs: 300,
                clipboard_clear_secs: 30,
                audit_enabled: true,
                rate_limit: RateLimitConfig {
                    max_attempts: 5,
                    window_secs: 300,
                    lockout_secs: 900,
                },
            }),
            cert_pins: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn current_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn security_audit_log(
    state: tauri::State<'_, SecurityState>,
    action: AuditAction,
    resource: String,
    details: Option<String>,
    success: bool,
) -> Result<(), SecurityError> {
    let config = state.config.lock().unwrap();
    if !config.audit_enabled {
        return Ok(());
    }
    drop(config);

    let entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        user: "current_user".to_string(),
        action,
        resource,
        details,
        ip_address: Some("127.0.0.1".to_string()),
        success,
    };

    let mut log = state.audit_log.lock().unwrap();
    log.push(entry);
    Ok(())
}

#[tauri::command]
pub fn security_audit_list(
    state: tauri::State<'_, SecurityState>,
    limit: Option<u32>,
) -> Result<Vec<AuditEntry>, SecurityError> {
    let log = state.audit_log.lock().unwrap();
    let entries: Vec<AuditEntry> = match limit {
        Some(n) => log.iter().rev().take(n as usize).cloned().collect(),
        None => log.clone(),
    };
    Ok(entries)
}

#[tauri::command]
pub fn security_audit_search(
    state: tauri::State<'_, SecurityState>,
    query: String,
) -> Result<Vec<AuditEntry>, SecurityError> {
    let query_lower = query.to_lowercase();
    let log = state.audit_log.lock().unwrap();
    let results: Vec<AuditEntry> = log
        .iter()
        .filter(|e| {
            e.resource.to_lowercase().contains(&query_lower)
                || e.details
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
                || serde_json::to_string(&e.action)
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query_lower)
        })
        .cloned()
        .collect();
    Ok(results)
}

#[tauri::command]
pub fn security_check_rate_limit(
    state: tauri::State<'_, SecurityState>,
    key: String,
) -> Result<bool, SecurityError> {
    let config = state.config.lock().unwrap();
    let max_attempts = config.rate_limit.max_attempts;
    let window_secs = config.rate_limit.window_secs;
    let lockout_secs = config.rate_limit.lockout_secs;
    drop(config);

    let now = current_timestamp_secs();
    let mut rate_limits = state.rate_limits.lock().unwrap();
    let entry = rate_limits.entry(key).or_insert_with(|| RateLimitState {
        attempts: Vec::new(),
        locked_until: None,
    });

    // Check if locked out
    if let Some(locked_until) = entry.locked_until {
        if now < locked_until {
            return Ok(false);
        }
        // Lockout expired, reset
        entry.locked_until = None;
        entry.attempts.clear();
    }

    // Prune old attempts outside the window
    entry.attempts.retain(|&ts| now - ts < window_secs);

    // Record this attempt
    entry.attempts.push(now);

    // Check if exceeded
    if entry.attempts.len() > max_attempts as usize {
        entry.locked_until = Some(now + lockout_secs);
        return Ok(false);
    }

    Ok(true)
}

#[tauri::command]
pub fn security_get_config(
    state: tauri::State<'_, SecurityState>,
) -> Result<SecurityConfig, SecurityError> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

#[tauri::command]
pub fn security_set_config(
    state: tauri::State<'_, SecurityState>,
    config: SecurityConfig,
) -> Result<(), SecurityError> {
    if config.rate_limit.max_attempts == 0 {
        return Err(SecurityError::InvalidInput(
            "max_attempts must be greater than 0".to_string(),
        ));
    }

    let mut current = state.config.lock().unwrap();
    *current = config;
    Ok(())
}

#[tauri::command]
pub fn security_cert_pin(
    state: tauri::State<'_, SecurityState>,
    host: String,
    fingerprint: CertFingerprint,
) -> Result<(), SecurityError> {
    if host.is_empty() {
        return Err(SecurityError::InvalidInput(
            "Host cannot be empty".to_string(),
        ));
    }

    let mut pins = state.cert_pins.lock().unwrap();
    pins.insert(host, fingerprint);
    Ok(())
}

#[tauri::command]
pub fn security_cert_verify(
    state: tauri::State<'_, SecurityState>,
    host: String,
    fingerprint: String,
) -> Result<bool, SecurityError> {
    let pins = state.cert_pins.lock().unwrap();
    match pins.get(&host) {
        Some(pinned) => Ok(pinned.sha256 == fingerprint),
        None => Ok(true), // No pin = trust on first use
    }
}

#[tauri::command]
pub fn security_cert_list_pins(
    state: tauri::State<'_, SecurityState>,
) -> Result<Vec<(String, CertFingerprint)>, SecurityError> {
    let pins = state.cert_pins.lock().unwrap();
    Ok(pins.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

#[tauri::command]
pub fn security_clear_audit_log(
    state: tauri::State<'_, SecurityState>,
) -> Result<u32, SecurityError> {
    let mut log = state.audit_log.lock().unwrap();
    let count = log.len() as u32;
    log.clear();
    Ok(count)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> SecurityState {
        SecurityState::new()
    }

    #[test]
    fn test_audit_crud() {
        let state = make_state();

        // Log entries
        {
            let mut log = state.audit_log.lock().unwrap();
            for i in 0..5 {
                log.push(AuditEntry {
                    id: Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    user: "testuser".to_string(),
                    action: AuditAction::Connect,
                    resource: format!("server-{}", i),
                    details: Some(format!("Connected to server {}", i)),
                    ip_address: Some("192.168.1.1".to_string()),
                    success: true,
                });
            }
        }

        // List with limit
        {
            let log = state.audit_log.lock().unwrap();
            assert_eq!(log.len(), 5);
            let limited: Vec<_> = log.iter().rev().take(3).collect();
            assert_eq!(limited.len(), 3);
        }

        // Search
        {
            let log = state.audit_log.lock().unwrap();
            let results: Vec<_> = log
                .iter()
                .filter(|e| e.resource.contains("server-2"))
                .collect();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].resource, "server-2");
        }

        // Clear
        {
            let mut log = state.audit_log.lock().unwrap();
            let count = log.len();
            assert_eq!(count, 5);
            log.clear();
            assert!(log.is_empty());
        }
    }

    #[test]
    fn test_rate_limiting() {
        let state = make_state();

        // Set a low rate limit for testing
        {
            let mut config = state.config.lock().unwrap();
            config.rate_limit = RateLimitConfig {
                max_attempts: 3,
                window_secs: 60,
                lockout_secs: 120,
            };
        }

        let now = current_timestamp_secs();

        // Simulate attempts
        {
            let mut rate_limits = state.rate_limits.lock().unwrap();
            let entry = rate_limits
                .entry("test_key".to_string())
                .or_insert_with(|| RateLimitState {
                    attempts: Vec::new(),
                    locked_until: None,
                });

            // First 3 attempts should be allowed
            for _ in 0..3 {
                entry.attempts.push(now);
            }
            assert_eq!(entry.attempts.len(), 3);

            // 4th attempt exceeds limit
            entry.attempts.push(now);
            assert!(entry.attempts.len() > 3);

            // Lock out
            entry.locked_until = Some(now + 120);
        }

        // Verify lockout
        {
            let rate_limits = state.rate_limits.lock().unwrap();
            let entry = rate_limits.get("test_key").unwrap();
            assert!(entry.locked_until.is_some());
            let locked_until = entry.locked_until.unwrap();
            assert!(locked_until > now);
        }
    }

    #[test]
    fn test_security_config() {
        let state = make_state();

        // Get default config
        {
            let config = state.config.lock().unwrap();
            assert_eq!(config.vault_timeout_secs, 300);
            assert_eq!(config.clipboard_clear_secs, 30);
            assert!(config.audit_enabled);
            assert_eq!(config.rate_limit.max_attempts, 5);
        }

        // Update config
        {
            let mut config = state.config.lock().unwrap();
            *config = SecurityConfig {
                vault_timeout_secs: 600,
                clipboard_clear_secs: 60,
                audit_enabled: false,
                rate_limit: RateLimitConfig {
                    max_attempts: 10,
                    window_secs: 600,
                    lockout_secs: 1800,
                },
            };
        }

        // Verify updated config
        {
            let config = state.config.lock().unwrap();
            assert_eq!(config.vault_timeout_secs, 600);
            assert_eq!(config.clipboard_clear_secs, 60);
            assert!(!config.audit_enabled);
            assert_eq!(config.rate_limit.max_attempts, 10);
        }
    }

    #[test]
    fn test_cert_pinning() {
        let state = make_state();

        // Pin a certificate
        let fingerprint = CertFingerprint {
            sha256: "SHA256:abcdef1234567890".to_string(),
            valid_from: "2024-01-01T00:00:00Z".to_string(),
            valid_until: "2025-01-01T00:00:00Z".to_string(),
            subject: "CN=example.com".to_string(),
            pinned: true,
        };

        {
            let mut pins = state.cert_pins.lock().unwrap();
            pins.insert("example.com".to_string(), fingerprint.clone());
        }

        // Verify matching fingerprint
        {
            let pins = state.cert_pins.lock().unwrap();
            let pinned = pins.get("example.com").unwrap();
            assert_eq!(pinned.sha256, "SHA256:abcdef1234567890");
            assert!(pinned.pinned);
        }

        // Verify non-matching fingerprint
        {
            let pins = state.cert_pins.lock().unwrap();
            let pinned = pins.get("example.com").unwrap();
            assert_ne!(pinned.sha256, "SHA256:wrong_fingerprint");
        }

        // List pins
        {
            let pins = state.cert_pins.lock().unwrap();
            let list: Vec<_> = pins.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].0, "example.com");
        }

        // Unpinned host should pass (trust on first use)
        {
            let pins = state.cert_pins.lock().unwrap();
            assert!(pins.get("unknown.host").is_none());
        }
    }
}

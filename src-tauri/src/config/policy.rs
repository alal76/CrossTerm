use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

// ── Host pattern ─────────────────────────────────────────────────────────────

/// Pattern for matching hostnames (glob-style: `*.prod.example.com`).
///
/// Supports a single leading `*` wildcard that matches one or more dot-separated
/// label segments.  Examples:
/// - `*.prod.example.com` matches `web.prod.example.com` and
///   `api.v2.prod.example.com` but NOT `prod.example.com` (the wildcard must
///   cover at least one label).
/// - `example.com` is an exact, case-insensitive match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostPattern(pub String);

impl HostPattern {
    /// Returns `true` if `hostname` matches this pattern.
    ///
    /// # Matching rules
    /// 1. Comparison is case-insensitive.
    /// 2. If the pattern starts with `*.`, the remainder is treated as a
    ///    fixed suffix; the hostname must end with that suffix AND have at
    ///    least one additional label prefix (i.e., `*.x.com` does NOT match
    ///    `x.com`).
    /// 3. Otherwise the entire pattern must equal the entire hostname.
    pub fn matches(&self, hostname: &str) -> bool {
        let pattern = self.0.trim().to_lowercase();
        let host = hostname.trim().to_lowercase();

        if let Some(suffix) = pattern.strip_prefix("*.") {
            // Wildcard branch: hostname must end with ".<suffix>"
            let expected_tail = format!(".{}", suffix);
            if host == suffix {
                // Exact match to the suffix part — the wildcard covers 0 labels,
                // which is NOT allowed.
                return false;
            }
            host.ends_with(&expected_tail)
        } else {
            // Exact match
            host == pattern
        }
    }
}

// ── RecordingPolicy ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingPolicy {
    /// Whether session recording is active at all.
    pub enabled: bool,
    /// Record sessions whose target hostname matches any of these patterns.
    pub require_recording_for: Vec<HostPattern>,
    /// Override the default recording storage path.  `None` uses the app
    /// default (`~/.local/share/CrossTerm/recordings`).
    pub storage_path: Option<String>,
    /// Number of days to keep recordings before deletion.  `0` means keep
    /// forever.
    pub retention_days: u32,
    /// Encrypt recording files at rest.
    pub encrypt_recordings: bool,
    /// Display a compliance banner to the user during a recorded session.
    pub notify_user: bool,
    /// Allow the user to disable recording for their own session.
    pub allow_user_disable: bool,
}

impl Default for RecordingPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            require_recording_for: Vec::new(),
            storage_path: None,
            retention_days: 90,
            encrypt_recordings: false,
            notify_user: true,
            allow_user_disable: false,
        }
    }
}

// ── PolicyConfig ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyConfig {
    pub recording: RecordingPolicy,
    /// Maximum session duration in minutes.  `None` means unlimited.
    pub max_session_duration_minutes: Option<u32>,
    /// Require MFA before opening privileged (e.g. root) sessions.
    pub require_mfa_for_privileged: bool,
    /// Allowed connection protocols.  An empty list means all protocols are
    /// allowed.
    pub allowed_protocols: Vec<String>,
    /// Deny-list of hostnames.  Connections to matching hosts are blocked.
    pub blocked_hosts: Vec<HostPattern>,
    /// Capture every command entered in the audit log.
    pub audit_all_commands: bool,
}

// ── PolicyState ───────────────────────────────────────────────────────────────

pub struct PolicyState {
    config: Arc<RwLock<PolicyConfig>>,
}

impl PolicyState {
    pub fn new() -> Self {
        let config = load_policy().unwrap_or_default();
        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }
}

impl Default for PolicyState {
    fn default() -> Self {
        Self::new()
    }
}

// ── File I/O helpers ─────────────────────────────────────────────────────────

/// Resolve the canonical path for `policy.json`, respecting portable mode.
fn policy_file_path() -> std::path::PathBuf {
    // Portable mode: look for sentinel file next to the binary.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join(".crossterm-portable").exists() {
                let p = dir.join("data").join("policy.json");
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                return p;
            }
        }
    }

    // Standard XDG / platform config dir.
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("crossterm");
    std::fs::create_dir_all(&config_dir).ok();
    config_dir.join("policy.json")
}

// ── Pure helper functions ─────────────────────────────────────────────────────

/// Load policy from `~/.config/crossterm/policy.json` (or portable path).
///
/// Returns [`PolicyConfig::default`] if the file does not yet exist.
pub fn load_policy() -> Result<PolicyConfig, String> {
    let path = policy_file_path();
    if !path.exists() {
        return Ok(PolicyConfig::default());
    }
    let data =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read policy file: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse policy file: {e}"))
}

/// Persist `config` to the policy config file.
pub fn save_policy(config: &PolicyConfig) -> Result<(), String> {
    let path = policy_file_path();
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize policy: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write policy file: {e}"))
}

/// Return `true` when the session to `hostname` must be recorded according to
/// the supplied policy.
///
/// Recording is required when:
/// - `recording.enabled` is `true`, AND
/// - `hostname` matches at least one pattern in `require_recording_for`, OR
///   `require_recording_for` is empty (meaning "record everything").
pub fn requires_recording(policy: &PolicyConfig, hostname: &str) -> bool {
    if !policy.recording.enabled {
        return false;
    }
    if policy.recording.require_recording_for.is_empty() {
        // Enabled with no specific host list → record all sessions.
        return true;
    }
    policy
        .recording
        .require_recording_for
        .iter()
        .any(|p| p.matches(hostname))
}

/// Return `true` when `hostname` is on the deny-list and the connection should
/// be blocked.
pub fn is_blocked(policy: &PolicyConfig, hostname: &str) -> bool {
    policy.blocked_hosts.iter().any(|p| p.matches(hostname))
}

/// Return `true` when `protocol` (e.g. `"ssh"`, `"rdp"`) is permitted.
///
/// An empty `allowed_protocols` list means *all* protocols are allowed.
pub fn is_protocol_allowed(policy: &PolicyConfig, protocol: &str) -> bool {
    if policy.allowed_protocols.is_empty() {
        return true;
    }
    let proto = protocol.to_lowercase();
    policy
        .allowed_protocols
        .iter()
        .any(|p| p.to_lowercase() == proto)
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Return the current policy configuration.
#[tauri::command]
pub fn policy_get(state: tauri::State<'_, PolicyState>) -> Result<PolicyConfig, String> {
    state
        .config
        .read()
        .map(|g| g.clone())
        .map_err(|e| format!("Lock poisoned: {e}"))
}

/// Replace the current policy configuration and persist it to disk.
#[tauri::command]
pub fn policy_update(
    config: PolicyConfig,
    state: tauri::State<'_, PolicyState>,
) -> Result<(), String> {
    save_policy(&config)?;
    let mut guard = state
        .config
        .write()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    *guard = config;
    Ok(())
}

/// Return `true` when the session to `hostname` requires recording under the
/// current policy.
#[tauri::command]
pub fn policy_check_recording_required(
    hostname: String,
    state: tauri::State<'_, PolicyState>,
) -> Result<bool, String> {
    let guard = state
        .config
        .read()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    Ok(requires_recording(&guard, &hostname))
}

/// Return `true` when a connection to `hostname` using `protocol` is permitted
/// by the current policy (i.e., the host is not blocked *and* the protocol is
/// in the allow-list).
#[tauri::command]
pub fn policy_check_connection_allowed(
    hostname: String,
    protocol: String,
    state: tauri::State<'_, PolicyState>,
) -> Result<bool, String> {
    let guard = state
        .config
        .read()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    if is_blocked(&guard, &hostname) {
        return Ok(false);
    }
    Ok(is_protocol_allowed(&guard, &protocol))
}

/// Reset the policy to defaults, persisting the reset to disk.
#[tauri::command]
pub fn policy_reset_to_defaults(state: tauri::State<'_, PolicyState>) -> Result<(), String> {
    let default = PolicyConfig::default();
    save_policy(&default)?;
    let mut guard = state
        .config
        .write()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    *guard = default;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── HostPattern tests ─────────────────────────────────────────────────────

    #[test]
    fn test_host_pattern_exact_match() {
        let p = HostPattern("example.com".into());
        assert!(p.matches("example.com"));
        assert!(p.matches("Example.COM")); // case-insensitive
        assert!(!p.matches("sub.example.com"));
        assert!(!p.matches("notexample.com"));
    }

    #[test]
    fn test_host_pattern_wildcard_star() {
        let p = HostPattern("*.prod.example.com".into());
        // One label prefix
        assert!(p.matches("web.prod.example.com"));
        // Multiple label prefix
        assert!(p.matches("api.v2.prod.example.com"));
        // Case-insensitive
        assert!(p.matches("WEB.PROD.EXAMPLE.COM"));
    }

    #[test]
    fn test_host_pattern_wildcard_no_match() {
        let p = HostPattern("*.prod.example.com".into());
        // Wildcard must cover at least one label — bare suffix must not match.
        assert!(!p.matches("prod.example.com"));
        // Completely unrelated host
        assert!(!p.matches("example.com"));
        // Suffix only, no prefix label
        assert!(!p.matches("example.com"));
    }

    // ── requires_recording tests ──────────────────────────────────────────────

    #[test]
    fn test_requires_recording_true_for_matching_host() {
        let policy = PolicyConfig {
            recording: RecordingPolicy {
                enabled: true,
                require_recording_for: vec![HostPattern("*.prod.example.com".into())],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(requires_recording(&policy, "web.prod.example.com"));
        assert!(!requires_recording(&policy, "web.staging.example.com"));
    }

    #[test]
    fn test_requires_recording_false_when_disabled() {
        let policy = PolicyConfig {
            recording: RecordingPolicy {
                enabled: false,
                require_recording_for: vec![HostPattern("*.prod.example.com".into())],
                ..Default::default()
            },
            ..Default::default()
        };
        // Even though the host matches, recording is disabled globally.
        assert!(!requires_recording(&policy, "web.prod.example.com"));
    }

    #[test]
    fn test_requires_recording_empty_list_records_all() {
        // When enabled with no specific host list, everything should be recorded.
        let policy = PolicyConfig {
            recording: RecordingPolicy {
                enabled: true,
                require_recording_for: vec![],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(requires_recording(&policy, "arbitrary.host.internal"));
    }

    // ── is_blocked tests ──────────────────────────────────────────────────────

    #[test]
    fn test_is_blocked_checks_deny_list() {
        let policy = PolicyConfig {
            blocked_hosts: vec![
                HostPattern("badactor.com".into()),
                HostPattern("*.malicious.net".into()),
            ],
            ..Default::default()
        };
        assert!(is_blocked(&policy, "badactor.com"));
        assert!(is_blocked(&policy, "c2.malicious.net"));
        assert!(!is_blocked(&policy, "goodhost.com"));
        // Bare suffix of wildcard must not match.
        assert!(!is_blocked(&policy, "malicious.net"));
    }

    // ── is_protocol_allowed tests ─────────────────────────────────────────────

    #[test]
    fn test_is_protocol_allowed_empty_means_all() {
        let policy = PolicyConfig {
            allowed_protocols: vec![],
            ..Default::default()
        };
        assert!(is_protocol_allowed(&policy, "ssh"));
        assert!(is_protocol_allowed(&policy, "rdp"));
        assert!(is_protocol_allowed(&policy, "vnc"));
        assert!(is_protocol_allowed(&policy, "telnet"));
    }

    #[test]
    fn test_is_protocol_allowed_restricts_to_list() {
        let policy = PolicyConfig {
            allowed_protocols: vec!["ssh".into(), "sftp".into()],
            ..Default::default()
        };
        assert!(is_protocol_allowed(&policy, "ssh"));
        assert!(is_protocol_allowed(&policy, "SSH")); // case-insensitive
        assert!(is_protocol_allowed(&policy, "sftp"));
        assert!(!is_protocol_allowed(&policy, "rdp"));
        assert!(!is_protocol_allowed(&policy, "telnet"));
        assert!(!is_protocol_allowed(&policy, "vnc"));
    }

    // ── Serialisation round-trip ──────────────────────────────────────────────

    #[test]
    fn test_policy_config_default_serialises() {
        let config = PolicyConfig::default();
        let json = serde_json::to_string(&config).expect("serialise");
        let back: PolicyConfig = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.recording.enabled, config.recording.enabled);
        assert_eq!(back.recording.retention_days, config.recording.retention_days);
        assert_eq!(back.audit_all_commands, config.audit_all_commands);
    }

    #[test]
    fn test_policy_config_roundtrip_with_patterns() {
        let config = PolicyConfig {
            recording: RecordingPolicy {
                enabled: true,
                require_recording_for: vec![
                    HostPattern("*.prod.corp.com".into()),
                    HostPattern("jump.internal".into()),
                ],
                storage_path: Some("/var/log/crossterm/recordings".into()),
                retention_days: 30,
                encrypt_recordings: true,
                notify_user: true,
                allow_user_disable: false,
            },
            max_session_duration_minutes: Some(480),
            require_mfa_for_privileged: true,
            allowed_protocols: vec!["ssh".into(), "sftp".into()],
            blocked_hosts: vec![HostPattern("*.darknet.example".into())],
            audit_all_commands: true,
        };

        let json = serde_json::to_string_pretty(&config).expect("serialise");
        let back: PolicyConfig = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.recording.enabled, true);
        assert_eq!(back.recording.require_recording_for.len(), 2);
        assert_eq!(
            back.recording.require_recording_for[0].0,
            "*.prod.corp.com"
        );
        assert_eq!(back.recording.retention_days, 30);
        assert_eq!(back.max_session_duration_minutes, Some(480));
        assert!(back.require_mfa_for_privileged);
        assert_eq!(back.allowed_protocols, vec!["ssh", "sftp"]);
        assert_eq!(back.blocked_hosts.len(), 1);
        assert!(back.audit_all_commands);
    }
}

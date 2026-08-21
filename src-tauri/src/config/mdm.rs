use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MdmPolicy {
    pub enforce_sso: bool,
    pub disable_local_vault: bool,
    pub allowed_protocols: Option<Vec<String>>,
    pub blocked_hosts: Option<Vec<String>>,
    pub require_recording: bool,
    pub max_session_duration_minutes: Option<u32>,
    pub force_vault_timeout_minutes: Option<u32>,
    pub disable_plugin_installation: bool,
    pub allowed_plugin_ids: Option<Vec<String>>,
    pub audit_endpoint: Option<String>,
    pub support_contact: Option<String>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdmStatus {
    pub managed: bool,
    pub policy_version: u32,
    pub source: String,
    pub last_fetched: Option<String>,
}

static MDM_POLICY: OnceLock<Arc<Mutex<Option<MdmPolicy>>>> = OnceLock::new();
fn get_policy_store() -> Arc<Mutex<Option<MdmPolicy>>> {
    MDM_POLICY
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

#[allow(dead_code)]
pub fn load_mdm_policy_from_file(path: &std::path::Path) -> Result<MdmPolicy, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn config_mdm_load(policy_json: String) -> Result<MdmStatus, String> {
    let policy: MdmPolicy = serde_json::from_str(&policy_json).map_err(|e| e.to_string())?;
    let version = policy.version;
    let store = get_policy_store();
    let mut guard = store.lock().map_err(|e| e.to_string())?;
    *guard = Some(policy);
    Ok(MdmStatus {
        managed: true,
        policy_version: version,
        source: "manual".to_string(),
        last_fetched: None,
    })
}

#[tauri::command]
pub fn config_mdm_get_policy() -> Result<Option<MdmPolicy>, String> {
    let store = get_policy_store();
    let guard = store.lock().map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

#[tauri::command]
pub fn config_mdm_status() -> Result<MdmStatus, String> {
    let store = get_policy_store();
    let guard = store.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(p) => Ok(MdmStatus {
            managed: true,
            policy_version: p.version,
            source: "manual".to_string(),
            last_fetched: None,
        }),
        None => Ok(MdmStatus {
            managed: false,
            policy_version: 0,
            source: "none".to_string(),
            last_fetched: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that touch the global MDM_POLICY store so they can't race.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn fresh_store() {
        // Reset by writing None to the mutex (OnceLock can't be reset, but we can clear the inner value)
        if let Some(store) = MDM_POLICY.get() {
            let mut g = store.lock().unwrap();
            *g = None;
        }
    }

    #[test]
    fn test_mdm_status_unmanaged() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fresh_store();
        let status = config_mdm_status().unwrap();
        assert!(!status.managed);
        assert_eq!(status.source, "none");
    }

    #[test]
    fn test_mdm_load_policy() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fresh_store();
        let json = r#"{"enforce_sso":true,"disable_local_vault":false,"require_recording":false,"disable_plugin_installation":false,"version":3}"#;
        let status = config_mdm_load(json.to_string()).unwrap();
        assert!(status.managed);
        assert_eq!(status.policy_version, 3);
        let policy = config_mdm_get_policy().unwrap().unwrap();
        assert!(policy.enforce_sso);
    }

    #[test]
    fn test_mdm_load_from_file() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let json = r#"{"enforce_sso":false,"disable_local_vault":true,"require_recording":true,"disable_plugin_installation":true,"version":5}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let policy = load_mdm_policy_from_file(tmp.path()).unwrap();
        assert!(policy.disable_local_vault);
        assert_eq!(policy.version, 5);
    }

    #[test]
    fn test_mdm_load_from_file_nonexistent_path_errors() {
        let result = load_mdm_policy_from_file(std::path::Path::new("/nonexistent/mdm-policy.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_mdm_load_from_file_malformed_json_errors() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"{ not valid json").unwrap();
        let result = load_mdm_policy_from_file(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_mdm_load_invalid_json_errors() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = config_mdm_load("not valid json at all".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_mdm_load_policy_with_optional_fields() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fresh_store();
        let json = r#"{
            "enforce_sso": true,
            "disable_local_vault": true,
            "allowed_protocols": ["ssh", "sftp"],
            "blocked_hosts": ["evil.example.com"],
            "require_recording": true,
            "max_session_duration_minutes": 60,
            "force_vault_timeout_minutes": 15,
            "disable_plugin_installation": true,
            "allowed_plugin_ids": ["plugin-a"],
            "audit_endpoint": "https://audit.example.com",
            "support_contact": "it@example.com",
            "version": 7
        }"#;
        let status = config_mdm_load(json.to_string()).unwrap();
        assert_eq!(status.policy_version, 7);
        assert_eq!(status.source, "manual");

        let policy = config_mdm_get_policy().unwrap().unwrap();
        assert_eq!(policy.allowed_protocols, Some(vec!["ssh".to_string(), "sftp".to_string()]));
        assert_eq!(policy.blocked_hosts, Some(vec!["evil.example.com".to_string()]));
        assert_eq!(policy.max_session_duration_minutes, Some(60));
        assert_eq!(policy.force_vault_timeout_minutes, Some(15));
        assert_eq!(policy.allowed_plugin_ids, Some(vec!["plugin-a".to_string()]));
        assert_eq!(policy.audit_endpoint.as_deref(), Some("https://audit.example.com"));
        assert_eq!(policy.support_contact.as_deref(), Some("it@example.com"));

        let status2 = config_mdm_status().unwrap();
        assert!(status2.managed);
        assert_eq!(status2.policy_version, 7);
    }

    #[test]
    fn test_mdm_get_policy_none_when_unset() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fresh_store();
        let policy = config_mdm_get_policy().unwrap();
        assert!(policy.is_none());
    }
}

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

    fn fresh_store() {
        // Reset by writing None to the mutex (OnceLock can't be reset, but we can clear the inner value)
        if let Some(store) = MDM_POLICY.get() {
            let mut g = store.lock().unwrap();
            *g = None;
        }
    }

    #[test]
    fn test_mdm_status_unmanaged() {
        fresh_store();
        let status = config_mdm_status().unwrap();
        assert!(!status.managed);
        assert_eq!(status.source, "none");
    }

    #[test]
    fn test_mdm_load_policy() {
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
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let json = r#"{"enforce_sso":false,"disable_local_vault":true,"require_recording":true,"disable_plugin_installation":true,"version":5}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let policy = load_mdm_policy_from_file(tmp.path()).unwrap();
        assert!(policy.disable_local_vault);
        assert_eq!(policy.version, 5);
    }
}

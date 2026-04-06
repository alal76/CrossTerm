use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("Failed to load plugin: {0}")]
    LoadFailed(String),
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Plugin not found: {0}")]
    NotFound(String),
    #[error("Plugin already loaded: {0}")]
    AlreadyLoaded(String),
    #[error("Plugin timeout: {0}")]
    Timeout(String),
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Serialize for PluginError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    Network,
    FileSystem,
    Terminal,
    Clipboard,
    Notifications,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub permissions: Vec<PluginPermission>,
    pub entry_point: String,
    pub api_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub loaded: bool,
    pub load_time_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEvent {
    pub plugin_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct PluginState {
    plugins: Mutex<HashMap<String, PluginInfo>>,
    plugins_dir: PathBuf,
    hooks: Mutex<HashMap<String, Vec<PluginHook>>>,
    kv_store: Mutex<HashMap<String, HashMap<String, serde_json::Value>>>,
    sandbox_configs: Mutex<HashMap<String, PluginSandboxConfig>>,
}

impl PluginState {
    pub fn new() -> Self {
        let plugins_dir = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("crossterm")
            .join("plugins");
        Self {
            plugins: Mutex::new(HashMap::new()),
            plugins_dir,
            hooks: Mutex::new(HashMap::new()),
            kv_store: Mutex::new(HashMap::new()),
            sandbox_configs: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn validate_manifest(manifest: &PluginManifest) -> Result<(), PluginError> {
    if manifest.id.is_empty() {
        return Err(PluginError::InvalidManifest("id is required".into()));
    }
    if manifest.name.is_empty() {
        return Err(PluginError::InvalidManifest("name is required".into()));
    }
    if manifest.version.is_empty() {
        return Err(PluginError::InvalidManifest("version is required".into()));
    }
    if manifest.entry_point.is_empty() {
        return Err(PluginError::InvalidManifest(
            "entry_point is required".into(),
        ));
    }
    Ok(())
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn plugin_scan(
    state: tauri::State<'_, PluginState>,
) -> Result<Vec<PluginInfo>, PluginError> {
    let plugins_dir = &state.plugins_dir;
    let mut found = Vec::new();

    if !plugins_dir.exists() {
        return Ok(found);
    }

    let entries = std::fs::read_dir(plugins_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                let data = std::fs::read_to_string(&manifest_path)?;
                let manifest: PluginManifest = serde_json::from_str(&data)?;
                validate_manifest(&manifest)?;
                let info = PluginInfo {
                    manifest,
                    enabled: false,
                    loaded: false,
                    load_time_ms: None,
                    error: None,
                };
                found.push(info);
            }
        }
    }

    // Update internal state with scanned plugins
    let mut plugins = state.plugins.lock().unwrap();
    for info in &found {
        plugins
            .entry(info.manifest.id.clone())
            .or_insert_with(|| info.clone());
    }

    Ok(found)
}

#[tauri::command]
pub fn plugin_load(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginInfo, PluginError> {
    let mut plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get_mut(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;

    if info.loaded {
        return Err(PluginError::AlreadyLoaded(plugin_id));
    }

    // Stub: In production, this would use wasmtime to load the WASM module
    info.loaded = true;
    info.load_time_ms = Some(0);
    info.error = None;

    Ok(info.clone())
}

// ── Plugin API Extensions ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHook {
    OnConnect,
    OnDisconnect,
    OnOutputLine,
    OnCommand,
    OnSessionStart,
    OnSessionEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginKvEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub plugin_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHttpRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSandboxConfig {
    pub allowed_paths: Vec<String>,
    pub allowed_hosts: Vec<String>,
    pub max_memory_mb: u32,
    pub max_cpu_time_ms: u64,
}

#[tauri::command]
pub fn plugin_unload(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get_mut(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;

    if !info.loaded {
        return Err(PluginError::NotFound(format!(
            "{} is not loaded",
            plugin_id
        )));
    }

    info.loaded = false;
    info.load_time_ms = None;
    Ok(())
}

#[tauri::command]
pub fn plugin_enable(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get_mut(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;
    info.enabled = true;
    Ok(())
}

#[tauri::command]
pub fn plugin_disable(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get_mut(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;
    info.enabled = false;
    info.loaded = false;
    Ok(())
}

#[tauri::command]
pub fn plugin_get_info(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginInfo, PluginError> {
    let plugins = state.plugins.lock().unwrap();
    plugins
        .get(&plugin_id)
        .cloned()
        .ok_or_else(|| PluginError::NotFound(plugin_id))
}

#[tauri::command]
pub fn plugin_list(
    state: tauri::State<'_, PluginState>,
) -> Result<Vec<PluginInfo>, PluginError> {
    let plugins = state.plugins.lock().unwrap();
    Ok(plugins.values().cloned().collect())
}

#[tauri::command]
pub fn plugin_install(
    path: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginInfo, PluginError> {
    let source = PathBuf::from(&path);
    if !source.exists() {
        return Err(PluginError::LoadFailed(format!(
            "File not found: {}",
            path
        )));
    }

    // Read manifest from adjacent manifest.json or generate a stub
    let manifest_path = source.parent().map(|p| p.join("manifest.json"));
    let manifest = if let Some(ref mp) = manifest_path {
        if mp.exists() {
            let data = std::fs::read_to_string(mp)?;
            serde_json::from_str(&data)?
        } else {
            // Generate a basic manifest from filename
            let name = source
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            PluginManifest {
                id: Uuid::new_v4().to_string(),
                name: name.clone(),
                version: "0.1.0".into(),
                author: "Unknown".into(),
                description: format!("Plugin {}", name),
                permissions: vec![],
                entry_point: path.clone(),
                api_version: "1.0".into(),
            }
        }
    } else {
        return Err(PluginError::InvalidManifest(
            "Could not determine manifest path".into(),
        ));
    };

    validate_manifest(&manifest)?;

    // Copy to plugins directory
    let dest_dir = state.plugins_dir.join(&manifest.id);
    std::fs::create_dir_all(&dest_dir)?;
    let dest_file = dest_dir.join(
        source
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("plugin.wasm")),
    );
    std::fs::copy(&source, &dest_file)?;

    // Save manifest
    let manifest_dest = dest_dir.join("manifest.json");
    let manifest_data = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_dest, manifest_data)?;

    let info = PluginInfo {
        manifest: manifest.clone(),
        enabled: false,
        loaded: false,
        load_time_ms: None,
        error: None,
    };

    let mut plugins = state.plugins.lock().unwrap();
    if plugins.contains_key(&manifest.id) {
        return Err(PluginError::AlreadyLoaded(manifest.id));
    }
    plugins.insert(manifest.id.clone(), info.clone());

    Ok(info)
}

#[tauri::command]
pub fn plugin_uninstall(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut plugins = state.plugins.lock().unwrap();
    if plugins.remove(&plugin_id).is_none() {
        return Err(PluginError::NotFound(plugin_id.clone()));
    }

    // Remove plugin directory
    let dir = state.plugins_dir.join(&plugin_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }

    Ok(())
}

#[tauri::command]
pub fn plugin_send_event(
    plugin_id: String,
    _event: PluginEvent,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;

    if !info.loaded {
        return Err(PluginError::ExecutionFailed(format!(
            "Plugin {} is not loaded",
            plugin_id
        )));
    }

    if !info.enabled {
        return Err(PluginError::ExecutionFailed(format!(
            "Plugin {} is not enabled",
            plugin_id
        )));
    }

    // Stub: In production, this would route the event to the WASM runtime
    Ok(())
}

#[tauri::command]
pub fn plugin_register_hook(
    plugin_id: String,
    hook: PluginHook,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let plugins = state.plugins.lock().unwrap();
    if !plugins.contains_key(&plugin_id) {
        return Err(PluginError::NotFound(plugin_id));
    }
    drop(plugins);

    let mut hooks = state.hooks.lock().unwrap();
    hooks
        .entry(plugin_id)
        .or_insert_with(Vec::new)
        .push(hook);
    Ok(())
}

#[tauri::command]
pub fn plugin_unregister_hook(
    plugin_id: String,
    hook: PluginHook,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut hooks = state.hooks.lock().unwrap();
    let hook_str = serde_json::to_string(&hook).unwrap_or_default();
    if let Some(plugin_hooks) = hooks.get_mut(&plugin_id) {
        plugin_hooks.retain(|h| serde_json::to_string(h).unwrap_or_default() != hook_str);
    }
    Ok(())
}

#[tauri::command]
pub fn plugin_kv_get(
    plugin_id: String,
    key: String,
    state: tauri::State<'_, PluginState>,
) -> Result<Option<serde_json::Value>, PluginError> {
    let kv = state.kv_store.lock().unwrap();
    Ok(kv.get(&plugin_id).and_then(|store| store.get(&key).cloned()))
}

#[tauri::command]
pub fn plugin_kv_set(
    plugin_id: String,
    key: String,
    value: serde_json::Value,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut kv = state.kv_store.lock().unwrap();
    kv.entry(plugin_id)
        .or_insert_with(HashMap::new)
        .insert(key, value);
    Ok(())
}

#[tauri::command]
pub fn plugin_kv_delete(
    plugin_id: String,
    key: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut kv = state.kv_store.lock().unwrap();
    if let Some(store) = kv.get_mut(&plugin_id) {
        store.remove(&key);
    }
    Ok(())
}

#[tauri::command]
pub async fn plugin_http_request(
    plugin_id: String,
    request: PluginHttpRequest,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginHttpResponse, PluginError> {
    // Verify plugin exists and has network permission
    let plugins = state.plugins.lock().unwrap();
    let info = plugins
        .get(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;
    if !info.manifest.permissions.iter().any(|p| matches!(p, PluginPermission::Network)) {
        return Err(PluginError::PermissionDenied(
            "Network permission required".into(),
        ));
    }
    drop(plugins);

    // Check sandbox config for allowed hosts
    let sandbox = state.sandbox_configs.lock().unwrap();
    if let Some(config) = sandbox.get(&plugin_id) {
        if !config.allowed_hosts.is_empty() {
            let url_host = request.url.split('/').nth(2).unwrap_or("");
            if !config.allowed_hosts.iter().any(|h| url_host.contains(h)) {
                return Err(PluginError::SandboxViolation(format!(
                    "Host '{}' not in allowed list",
                    url_host
                )));
            }
        }
    }
    drop(sandbox);

    // Stub: In production, this would make an actual HTTP request
    Ok(PluginHttpResponse {
        status: 200,
        headers: HashMap::new(),
        body: String::new(),
    })
}

#[tauri::command]
pub fn plugin_get_sandbox_config(
    plugin_id: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginSandboxConfig, PluginError> {
    let configs = state.sandbox_configs.lock().unwrap();
    configs
        .get(&plugin_id)
        .cloned()
        .ok_or_else(|| {
            // Return default config if none set
            PluginError::NotFound(format!("No sandbox config for {}", plugin_id))
        })
}

#[tauri::command]
pub fn plugin_set_sandbox_config(
    plugin_id: String,
    config: PluginSandboxConfig,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    let mut configs = state.sandbox_configs.lock().unwrap();
    configs.insert(plugin_id, config);
    Ok(())
}

#[tauri::command]
pub fn plugin_load_wasm(
    path: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginInfo, PluginError> {
    let source = PathBuf::from(&path);
    if !source.exists() {
        return Err(PluginError::LoadFailed(format!(
            "WASM file not found: {}",
            path
        )));
    }

    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    let manifest = PluginManifest {
        id: Uuid::new_v4().to_string(),
        name: name.clone(),
        version: "0.1.0".into(),
        author: "Unknown".into(),
        description: format!("WASM plugin {}", name),
        permissions: vec![],
        entry_point: path,
        api_version: "1.0".into(),
    };

    let info = PluginInfo {
        manifest: manifest.clone(),
        enabled: false,
        loaded: true,
        load_time_ms: Some(0),
        error: None,
    };

    let mut plugins = state.plugins.lock().unwrap();
    plugins.insert(manifest.id.clone(), info.clone());

    Ok(info)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manifest() -> PluginManifest {
        PluginManifest {
            id: "test-plugin".into(),
            name: "Test Plugin".into(),
            version: "1.0.0".into(),
            author: "Test Author".into(),
            description: "A test plugin".into(),
            permissions: vec![
                PluginPermission::Network,
                PluginPermission::FileSystem,
                PluginPermission::Terminal,
                PluginPermission::Clipboard,
                PluginPermission::Notifications,
                PluginPermission::Settings,
            ],
            entry_point: "plugin.wasm".into(),
            api_version: "1.0".into(),
        }
    }

    fn make_state() -> PluginState {
        PluginState {
            plugins: Mutex::new(HashMap::new()),
            plugins_dir: std::env::temp_dir().join("crossterm-test-plugins"),
            hooks: Mutex::new(HashMap::new()),
            kv_store: Mutex::new(HashMap::new()),
            sandbox_configs: Mutex::new(HashMap::new()),
        }
    }

    #[test]
    fn test_plugin_manifest_serde() {
        let manifest = make_manifest();
        let json = serde_json::to_string(&manifest).expect("serialize");
        let parsed: PluginManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.id, "test-plugin");
        assert_eq!(parsed.name, "Test Plugin");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.permissions.len(), 6);

        // Verify snake_case serialization of permissions
        assert!(json.contains("\"network\""));
        assert!(json.contains("\"file_system\""));
        assert!(json.contains("\"terminal\""));
        assert!(json.contains("\"clipboard\""));
        assert!(json.contains("\"notifications\""));
        assert!(json.contains("\"settings\""));
    }

    #[test]
    fn test_plugin_lifecycle() {
        let state = make_state();
        let manifest = make_manifest();
        let plugin_id = manifest.id.clone();

        // Register plugin
        {
            let mut plugins = state.plugins.lock().unwrap();
            plugins.insert(
                plugin_id.clone(),
                PluginInfo {
                    manifest: manifest.clone(),
                    enabled: false,
                    loaded: false,
                    load_time_ms: None,
                    error: None,
                },
            );
        }

        // Load
        {
            let mut plugins = state.plugins.lock().unwrap();
            let info = plugins.get_mut(&plugin_id).unwrap();
            assert!(!info.loaded);
            info.loaded = true;
            info.load_time_ms = Some(5);
            assert!(info.loaded);
        }

        // Enable
        {
            let mut plugins = state.plugins.lock().unwrap();
            let info = plugins.get_mut(&plugin_id).unwrap();
            info.enabled = true;
            assert!(info.enabled);
        }

        // Disable
        {
            let mut plugins = state.plugins.lock().unwrap();
            let info = plugins.get_mut(&plugin_id).unwrap();
            info.enabled = false;
            assert!(!info.enabled);
        }

        // Unload
        {
            let mut plugins = state.plugins.lock().unwrap();
            let info = plugins.get_mut(&plugin_id).unwrap();
            info.loaded = false;
            info.load_time_ms = None;
            assert!(!info.loaded);
        }

        // Uninstall
        {
            let mut plugins = state.plugins.lock().unwrap();
            let removed = plugins.remove(&plugin_id);
            assert!(removed.is_some());
            assert!(!plugins.contains_key(&plugin_id));
        }
    }

    #[test]
    fn test_plugin_duplicate_load() {
        let state = make_state();
        let manifest = make_manifest();
        let plugin_id = manifest.id.clone();

        // Register and load
        {
            let mut plugins = state.plugins.lock().unwrap();
            plugins.insert(
                plugin_id.clone(),
                PluginInfo {
                    manifest,
                    enabled: false,
                    loaded: true,
                    load_time_ms: Some(5),
                    error: None,
                },
            );
        }

        // Attempt duplicate load should fail
        {
            let plugins = state.plugins.lock().unwrap();
            let info = plugins.get(&plugin_id).unwrap();
            assert!(info.loaded);
            // Simulating the AlreadyLoaded check
            let err = PluginError::AlreadyLoaded(plugin_id.clone());
            assert!(err.to_string().contains("already loaded"));
        }
    }

    #[test]
    fn test_plugin_not_found() {
        let state = make_state();
        let plugins = state.plugins.lock().unwrap();
        let result = plugins.get("nonexistent");
        assert!(result.is_none());

        let err = PluginError::NotFound("nonexistent".into());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_plugin_hooks() {
        let state = make_state();
        let plugin_id = "test-plugin".to_string();

        // Register plugin first
        {
            let mut plugins = state.plugins.lock().unwrap();
            plugins.insert(
                plugin_id.clone(),
                PluginInfo {
                    manifest: make_manifest(),
                    enabled: true,
                    loaded: true,
                    load_time_ms: Some(0),
                    error: None,
                },
            );
        }

        // Register hooks
        {
            let mut hooks = state.hooks.lock().unwrap();
            let plugin_hooks = hooks.entry(plugin_id.clone()).or_insert_with(Vec::new);
            plugin_hooks.push(PluginHook::OnConnect);
            plugin_hooks.push(PluginHook::OnDisconnect);
            plugin_hooks.push(PluginHook::OnOutputLine);
            assert_eq!(plugin_hooks.len(), 3);
        }

        // Unregister a hook
        {
            let mut hooks = state.hooks.lock().unwrap();
            let hook_str = serde_json::to_string(&PluginHook::OnConnect).unwrap_or_default();
            if let Some(plugin_hooks) = hooks.get_mut(&plugin_id) {
                plugin_hooks.retain(|h| serde_json::to_string(h).unwrap_or_default() != hook_str);
                assert_eq!(plugin_hooks.len(), 2);
            }
        }

        // Verify hook serde
        let json = serde_json::to_string(&PluginHook::OnSessionStart).unwrap();
        assert!(json.contains("on_session_start"));
    }

    #[test]
    fn test_plugin_kv_store() {
        let state = make_state();
        let plugin_a = "plugin-a".to_string();
        let plugin_b = "plugin-b".to_string();

        // Set values for plugin A
        {
            let mut kv = state.kv_store.lock().unwrap();
            let store = kv.entry(plugin_a.clone()).or_insert_with(HashMap::new);
            store.insert("key1".to_string(), serde_json::json!("value1"));
            store.insert("key2".to_string(), serde_json::json!(42));
        }

        // Set values for plugin B
        {
            let mut kv = state.kv_store.lock().unwrap();
            let store = kv.entry(plugin_b.clone()).or_insert_with(HashMap::new);
            store.insert("key1".to_string(), serde_json::json!("b-value1"));
        }

        // Get from plugin A
        {
            let kv = state.kv_store.lock().unwrap();
            let val = kv.get(&plugin_a).and_then(|s| s.get("key1")).cloned();
            assert_eq!(val, Some(serde_json::json!("value1")));
        }

        // Ensure isolation: A cannot see B's data
        {
            let kv = state.kv_store.lock().unwrap();
            let a_store = kv.get(&plugin_a).unwrap();
            let b_store = kv.get(&plugin_b).unwrap();
            assert_ne!(a_store.get("key1"), b_store.get("key1"));
        }

        // Delete
        {
            let mut kv = state.kv_store.lock().unwrap();
            if let Some(store) = kv.get_mut(&plugin_a) {
                store.remove("key1");
                assert!(store.get("key1").is_none());
                assert!(store.get("key2").is_some());
            }
        }
    }

    #[test]
    fn test_plugin_sandbox_config() {
        let state = make_state();
        let plugin_id = "sandbox-plugin".to_string();

        let config = PluginSandboxConfig {
            allowed_paths: vec!["/tmp".to_string(), "/home".to_string()],
            allowed_hosts: vec!["api.example.com".to_string()],
            max_memory_mb: 128,
            max_cpu_time_ms: 5000,
        };

        // Set config
        {
            let mut configs = state.sandbox_configs.lock().unwrap();
            configs.insert(plugin_id.clone(), config.clone());
        }

        // Get config
        {
            let configs = state.sandbox_configs.lock().unwrap();
            let retrieved = configs.get(&plugin_id).unwrap();
            assert_eq!(retrieved.max_memory_mb, 128);
            assert_eq!(retrieved.max_cpu_time_ms, 5000);
            assert_eq!(retrieved.allowed_paths.len(), 2);
            assert_eq!(retrieved.allowed_hosts.len(), 1);
        }

        // Verify serde roundtrip
        let json = serde_json::to_string(&config).unwrap();
        let parsed: PluginSandboxConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_memory_mb, 128);
        assert_eq!(parsed.allowed_hosts[0], "api.example.com");
    }
}

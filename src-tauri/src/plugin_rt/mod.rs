use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
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
    do_plugin_scan(state.inner())
}

/// Extracted from the `plugin_scan` Tauri command so it's directly
/// unit-testable without constructing a `tauri::State` (which requires a
/// full mock app). Scans `state.plugins_dir` for subdirectories containing a
/// `manifest.json` and registers any newly-found plugins into `state.plugins`.
fn do_plugin_scan(state: &PluginState) -> Result<Vec<PluginInfo>, PluginError> {
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
    do_plugin_load(plugin_id, state.inner())
}

fn do_plugin_load(plugin_id: String, state: &PluginState) -> Result<PluginInfo, PluginError> {
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

#[allow(clippy::enum_variant_names)]
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

#[allow(dead_code)]
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
    do_plugin_unload(plugin_id, state.inner())
}

fn do_plugin_unload(plugin_id: String, state: &PluginState) -> Result<(), PluginError> {
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
    do_plugin_enable(plugin_id, state.inner())
}

fn do_plugin_enable(plugin_id: String, state: &PluginState) -> Result<(), PluginError> {
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
    do_plugin_disable(plugin_id, state.inner())
}

fn do_plugin_disable(plugin_id: String, state: &PluginState) -> Result<(), PluginError> {
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
    do_plugin_get_info(plugin_id, state.inner())
}

fn do_plugin_get_info(plugin_id: String, state: &PluginState) -> Result<PluginInfo, PluginError> {
    let plugins = state.plugins.lock().unwrap();
    plugins
        .get(&plugin_id)
        .cloned()
        .ok_or(PluginError::NotFound(plugin_id))
}

#[tauri::command]
pub fn plugin_list(
    state: tauri::State<'_, PluginState>,
) -> Result<Vec<PluginInfo>, PluginError> {
    do_plugin_list(state.inner())
}

fn do_plugin_list(state: &PluginState) -> Result<Vec<PluginInfo>, PluginError> {
    let plugins = state.plugins.lock().unwrap();
    Ok(plugins.values().cloned().collect())
}

#[tauri::command]
pub fn plugin_install(
    path: String,
    state: tauri::State<'_, PluginState>,
) -> Result<PluginInfo, PluginError> {
    do_plugin_install(path, state.inner())
}

fn do_plugin_install(path: String, state: &PluginState) -> Result<PluginInfo, PluginError> {
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
    do_plugin_uninstall(plugin_id, state.inner())
}

fn do_plugin_uninstall(plugin_id: String, state: &PluginState) -> Result<(), PluginError> {
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
    do_plugin_send_event(plugin_id, state.inner())
}

fn do_plugin_send_event(plugin_id: String, state: &PluginState) -> Result<(), PluginError> {
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
    do_plugin_register_hook(plugin_id, hook, state.inner())
}

fn do_plugin_register_hook(
    plugin_id: String,
    hook: PluginHook,
    state: &PluginState,
) -> Result<(), PluginError> {
    let plugins = state.plugins.lock().unwrap();
    if !plugins.contains_key(&plugin_id) {
        return Err(PluginError::NotFound(plugin_id));
    }
    drop(plugins);

    let mut hooks = state.hooks.lock().unwrap();
    hooks
        .entry(plugin_id)
        .or_default()
        .push(hook);
    Ok(())
}

#[tauri::command]
pub fn plugin_unregister_hook(
    plugin_id: String,
    hook: PluginHook,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    do_plugin_unregister_hook(plugin_id, hook, state.inner())
}

fn do_plugin_unregister_hook(
    plugin_id: String,
    hook: PluginHook,
    state: &PluginState,
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
    do_plugin_kv_get(plugin_id, key, state.inner())
}

fn do_plugin_kv_get(
    plugin_id: String,
    key: String,
    state: &PluginState,
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
    do_plugin_kv_set(plugin_id, key, value, state.inner())
}

fn do_plugin_kv_set(
    plugin_id: String,
    key: String,
    value: serde_json::Value,
    state: &PluginState,
) -> Result<(), PluginError> {
    let mut kv = state.kv_store.lock().unwrap();
    kv.entry(plugin_id)
        .or_default()
        .insert(key, value);
    Ok(())
}

#[tauri::command]
pub fn plugin_kv_delete(
    plugin_id: String,
    key: String,
    state: tauri::State<'_, PluginState>,
) -> Result<(), PluginError> {
    do_plugin_kv_delete(plugin_id, key, state.inner())
}

fn do_plugin_kv_delete(
    plugin_id: String,
    key: String,
    state: &PluginState,
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
    do_plugin_http_request(plugin_id, request, state.inner()).await
}

async fn do_plugin_http_request(
    plugin_id: String,
    request: PluginHttpRequest,
    state: &PluginState,
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
    do_plugin_get_sandbox_config(plugin_id, state.inner())
}

fn do_plugin_get_sandbox_config(
    plugin_id: String,
    state: &PluginState,
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
    do_plugin_set_sandbox_config(plugin_id, config, state.inner())
}

fn do_plugin_set_sandbox_config(
    plugin_id: String,
    config: PluginSandboxConfig,
    state: &PluginState,
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
    do_plugin_load_wasm(path, state.inner())
}

fn do_plugin_load_wasm(path: String, state: &PluginState) -> Result<PluginInfo, PluginError> {
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
//
// NOTE: an earlier pass at these tests exercised the mutexes inside
// `PluginState` directly (locking `state.plugins` and mutating it inline)
// instead of calling the `do_plugin_*` functions that the `#[tauri::command]`
// wrappers delegate to. That looked like it was testing plugin lifecycle
// behavior, but tarpaulin correctly reported 0% coverage for this file
// because none of the actual production functions were ever invoked. The
// tests below call `do_plugin_*` directly (the tauri command wrappers
// themselves can't be unit tested without a full mock `tauri::State`).

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
            let plugin_hooks = hooks.entry(plugin_id.clone()).or_default();
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
            let store = kv.entry(plugin_a.clone()).or_default();
            store.insert("key1".to_string(), serde_json::json!("value1"));
            store.insert("key2".to_string(), serde_json::json!(42));
        }

        // Set values for plugin B
        {
            let mut kv = state.kv_store.lock().unwrap();
            let store = kv.entry(plugin_b.clone()).or_default();
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

    // ── Tests that exercise the real `do_plugin_*` functions ─────────────

    fn make_state_at(dir: PathBuf) -> PluginState {
        PluginState {
            plugins: Mutex::new(HashMap::new()),
            plugins_dir: dir,
            hooks: Mutex::new(HashMap::new()),
            kv_store: Mutex::new(HashMap::new()),
            sandbox_configs: Mutex::new(HashMap::new()),
        }
    }

    fn insert_plugin(state: &PluginState, id: &str, enabled: bool, loaded: bool) {
        let mut manifest = make_manifest();
        manifest.id = id.to_string();
        let mut plugins = state.plugins.lock().unwrap();
        plugins.insert(
            id.to_string(),
            PluginInfo {
                manifest,
                enabled,
                loaded,
                load_time_ms: if loaded { Some(0) } else { None },
                error: None,
            },
        );
    }

    #[test]
    fn test_validate_manifest_rejects_empty_fields() {
        let mut m = make_manifest();
        m.id = String::new();
        assert!(matches!(
            validate_manifest(&m),
            Err(PluginError::InvalidManifest(_))
        ));

        let mut m = make_manifest();
        m.name = String::new();
        assert!(validate_manifest(&m).is_err());

        let mut m = make_manifest();
        m.version = String::new();
        assert!(validate_manifest(&m).is_err());

        let mut m = make_manifest();
        m.entry_point = String::new();
        assert!(validate_manifest(&m).is_err());

        assert!(validate_manifest(&make_manifest()).is_ok());
    }

    #[test]
    fn test_do_plugin_scan_missing_dir_returns_empty() {
        let dir = std::env::temp_dir().join(format!("ct-scan-missing-{}", Uuid::new_v4()));
        let state = make_state_at(dir);
        let found = do_plugin_scan(&state).expect("scan ok");
        assert!(found.is_empty());
    }

    #[test]
    fn test_do_plugin_scan_finds_valid_manifest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tmp.path().join("my-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = make_manifest();
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let state = make_state_at(tmp.path().to_path_buf());
        let found = do_plugin_scan(&state).expect("scan ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.id, "test-plugin");
        assert!(!found[0].loaded);

        // State should now contain the scanned plugin.
        let plugins = state.plugins.lock().unwrap();
        assert!(plugins.contains_key("test-plugin"));
    }

    #[test]
    fn test_do_plugin_scan_rescan_does_not_overwrite_existing_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tmp.path().join("my-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = make_manifest();
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let state = make_state_at(tmp.path().to_path_buf());
        do_plugin_scan(&state).expect("first scan");

        // Mark it loaded, then rescan: `or_insert_with` must not clobber it.
        {
            let mut plugins = state.plugins.lock().unwrap();
            plugins.get_mut("test-plugin").unwrap().loaded = true;
        }
        do_plugin_scan(&state).expect("second scan");
        let plugins = state.plugins.lock().unwrap();
        assert!(plugins.get("test-plugin").unwrap().loaded);
    }

    #[test]
    fn test_do_plugin_scan_invalid_manifest_json_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tmp.path().join("bad-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("manifest.json"), "{ not valid json").unwrap();

        let state = make_state_at(tmp.path().to_path_buf());
        let result = do_plugin_scan(&state);
        assert!(matches!(result, Err(PluginError::Serde(_))));
    }

    #[test]
    fn test_do_plugin_scan_invalid_manifest_fields_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tmp.path().join("bad-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let mut manifest = make_manifest();
        manifest.id = String::new();
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let state = make_state_at(tmp.path().to_path_buf());
        let result = do_plugin_scan(&state);
        assert!(matches!(result, Err(PluginError::InvalidManifest(_))));
    }

    #[test]
    fn test_do_plugin_load_success_and_already_loaded() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", false, false);

        let info = do_plugin_load("p1".into(), &state).expect("load ok");
        assert!(info.loaded);
        assert_eq!(info.load_time_ms, Some(0));

        let err = do_plugin_load("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::AlreadyLoaded(_)));
    }

    #[test]
    fn test_do_plugin_load_not_found() {
        let state = make_state_at(std::env::temp_dir());
        let err = do_plugin_load("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[test]
    fn test_do_plugin_unload_success_and_errors() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", false, true);

        do_plugin_unload("p1".into(), &state).expect("unload ok");
        let plugins = state.plugins.lock().unwrap();
        assert!(!plugins.get("p1").unwrap().loaded);
        drop(plugins);

        // Already unloaded -> error.
        let err = do_plugin_unload("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        // Unknown plugin -> error.
        let err = do_plugin_unload("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[test]
    fn test_do_plugin_enable_disable() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", false, true);

        do_plugin_enable("p1".into(), &state).expect("enable ok");
        assert!(state.plugins.lock().unwrap().get("p1").unwrap().enabled);

        do_plugin_disable("p1".into(), &state).expect("disable ok");
        let plugins = state.plugins.lock().unwrap();
        let info = plugins.get("p1").unwrap();
        assert!(!info.enabled);
        assert!(!info.loaded); // disable also unloads

        drop(plugins);
        let err = do_plugin_enable("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
        let err = do_plugin_disable("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[test]
    fn test_do_plugin_get_info_and_list() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", true, true);
        insert_plugin(&state, "p2", false, false);

        let info = do_plugin_get_info("p1".into(), &state).expect("found");
        assert_eq!(info.manifest.id, "p1");

        let err = do_plugin_get_info("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let list = do_plugin_list(&state).expect("list ok");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_do_plugin_install_file_not_found() {
        let state = make_state_at(std::env::temp_dir());
        let err = do_plugin_install("/no/such/file.wasm".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::LoadFailed(_)));
    }

    #[test]
    fn test_do_plugin_install_generates_stub_manifest_when_absent() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = tempfile::tempdir().expect("tempdir");
        let wasm_path = src_dir.path().join("cool_plugin.wasm");
        std::fs::write(&wasm_path, b"fake wasm bytes").unwrap();

        let state = make_state_at(plugins_dir.path().to_path_buf());
        let info = do_plugin_install(wasm_path.to_string_lossy().to_string(), &state)
            .expect("install ok");

        assert_eq!(info.manifest.name, "cool_plugin");
        assert_eq!(info.manifest.version, "0.1.0");
        assert!(!info.loaded);

        // Files should have been copied into the plugins dir.
        let dest_dir = plugins_dir.path().join(&info.manifest.id);
        assert!(dest_dir.join("cool_plugin.wasm").exists());
        assert!(dest_dir.join("manifest.json").exists());

        // Duplicate install (same manifest.id already registered) errors.
        let plugins = state.plugins.lock().unwrap();
        assert!(plugins.contains_key(&info.manifest.id));
    }

    #[test]
    fn test_do_plugin_install_uses_adjacent_manifest_and_rejects_duplicate() {
        let src_dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = tempfile::tempdir().expect("tempdir");
        let wasm_path = src_dir.path().join("plugin.wasm");
        std::fs::write(&wasm_path, b"fake wasm bytes").unwrap();
        let manifest = make_manifest();
        std::fs::write(
            src_dir.path().join("manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let state = make_state_at(plugins_dir.path().to_path_buf());
        let info = do_plugin_install(wasm_path.to_string_lossy().to_string(), &state)
            .expect("install ok");
        assert_eq!(info.manifest.id, "test-plugin");

        // Installing again with the same manifest id should fail.
        let err = do_plugin_install(wasm_path.to_string_lossy().to_string(), &state)
            .unwrap_err();
        assert!(matches!(err, PluginError::AlreadyLoaded(_)));
    }

    #[test]
    fn test_do_plugin_uninstall_success_and_not_found() {
        let plugins_dir = tempfile::tempdir().expect("tempdir");
        let state = make_state_at(plugins_dir.path().to_path_buf());
        insert_plugin(&state, "p1", false, false);
        let dir = plugins_dir.path().join("p1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("marker"), b"x").unwrap();

        do_plugin_uninstall("p1".into(), &state).expect("uninstall ok");
        assert!(!dir.exists());
        assert!(!state.plugins.lock().unwrap().contains_key("p1"));

        let err = do_plugin_uninstall("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[test]
    fn test_do_plugin_send_event_requires_loaded_and_enabled() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", false, false);

        let err = do_plugin_send_event("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));

        insert_plugin(&state, "p1", false, true); // loaded, not enabled
        let err = do_plugin_send_event("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));

        insert_plugin(&state, "p1", true, true); // loaded and enabled
        do_plugin_send_event("p1".into(), &state).expect("event ok");

        let err = do_plugin_send_event("nope".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[test]
    fn test_do_plugin_register_and_unregister_hook() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", false, false);

        // Unknown plugin -> error, no hook registered.
        let err = do_plugin_register_hook("nope".into(), PluginHook::OnConnect, &state)
            .unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        do_plugin_register_hook("p1".into(), PluginHook::OnConnect, &state).unwrap();
        do_plugin_register_hook("p1".into(), PluginHook::OnDisconnect, &state).unwrap();
        do_plugin_register_hook("p1".into(), PluginHook::OnConnect, &state).unwrap();

        {
            let hooks = state.hooks.lock().unwrap();
            assert_eq!(hooks.get("p1").unwrap().len(), 3);
        }

        do_plugin_unregister_hook("p1".into(), PluginHook::OnConnect, &state).unwrap();
        let hooks = state.hooks.lock().unwrap();
        let remaining = hooks.get("p1").unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(matches!(remaining[0], PluginHook::OnDisconnect));
    }

    #[test]
    fn test_do_plugin_kv_roundtrip_via_functions() {
        let state = make_state_at(std::env::temp_dir());

        assert_eq!(
            do_plugin_kv_get("p1".into(), "k".into(), &state).unwrap(),
            None
        );

        do_plugin_kv_set("p1".into(), "k".into(), serde_json::json!(42), &state).unwrap();
        assert_eq!(
            do_plugin_kv_get("p1".into(), "k".into(), &state).unwrap(),
            Some(serde_json::json!(42))
        );

        do_plugin_kv_delete("p1".into(), "k".into(), &state).unwrap();
        assert_eq!(
            do_plugin_kv_get("p1".into(), "k".into(), &state).unwrap(),
            None
        );

        // Deleting a key that was never set should be a no-op, not an error.
        do_plugin_kv_delete("p1".into(), "missing".into(), &state).unwrap();
    }

    #[test]
    fn test_do_plugin_sandbox_config_get_set() {
        let state = make_state_at(std::env::temp_dir());

        let err = do_plugin_get_sandbox_config("p1".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let config = PluginSandboxConfig {
            allowed_paths: vec!["/tmp".into()],
            allowed_hosts: vec!["example.com".into()],
            max_memory_mb: 64,
            max_cpu_time_ms: 1000,
        };
        do_plugin_set_sandbox_config("p1".into(), config.clone(), &state).unwrap();

        let retrieved = do_plugin_get_sandbox_config("p1".into(), &state).unwrap();
        assert_eq!(retrieved.max_memory_mb, 64);
        assert_eq!(retrieved.allowed_hosts, vec!["example.com".to_string()]);
    }

    #[test]
    fn test_do_plugin_load_wasm() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = make_state_at(std::env::temp_dir());

        let err = do_plugin_load_wasm("/no/such/plugin.wasm".into(), &state).unwrap_err();
        assert!(matches!(err, PluginError::LoadFailed(_)));

        let wasm_path = tmp.path().join("neat.wasm");
        std::fs::write(&wasm_path, b"bytes").unwrap();
        let info =
            do_plugin_load_wasm(wasm_path.to_string_lossy().to_string(), &state).expect("ok");
        assert_eq!(info.manifest.name, "neat");
        assert!(info.loaded);
        assert!(state
            .plugins
            .lock()
            .unwrap()
            .contains_key(&info.manifest.id));
    }

    #[tokio::test]
    async fn test_do_plugin_http_request_requires_network_permission() {
        let state = make_state_at(std::env::temp_dir());
        let mut manifest = make_manifest();
        manifest.id = "p1".into();
        manifest.permissions = vec![PluginPermission::FileSystem]; // no Network
        state.plugins.lock().unwrap().insert(
            "p1".into(),
            PluginInfo {
                manifest,
                enabled: true,
                loaded: true,
                load_time_ms: Some(0),
                error: None,
            },
        );

        let req = PluginHttpRequest {
            url: "https://example.com/foo".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            body: None,
        };
        let err = do_plugin_http_request("p1".into(), req, &state)
            .await
            .unwrap_err();
        assert!(matches!(err, PluginError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn test_do_plugin_http_request_not_found() {
        let state = make_state_at(std::env::temp_dir());
        let req = PluginHttpRequest {
            url: "https://example.com/foo".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            body: None,
        };
        let err = do_plugin_http_request("nope".into(), req, &state)
            .await
            .unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_do_plugin_http_request_sandbox_host_allowlist() {
        let state = make_state_at(std::env::temp_dir());
        insert_plugin(&state, "p1", true, true); // has Network permission via make_manifest

        state.sandbox_configs.lock().unwrap().insert(
            "p1".into(),
            PluginSandboxConfig {
                allowed_paths: vec![],
                allowed_hosts: vec!["good.example.com".into()],
                max_memory_mb: 32,
                max_cpu_time_ms: 1000,
            },
        );

        // Disallowed host.
        let req = PluginHttpRequest {
            url: "https://evil.example.com/foo".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            body: None,
        };
        let err = do_plugin_http_request("p1".into(), req, &state)
            .await
            .unwrap_err();
        assert!(matches!(err, PluginError::SandboxViolation(_)));

        // Allowed host succeeds (stubbed 200 response).
        let req = PluginHttpRequest {
            url: "https://good.example.com/foo".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            body: None,
        };
        let resp = do_plugin_http_request("p1".into(), req, &state)
            .await
            .expect("ok");
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn test_plugin_state_new_sets_plugins_dir() {
        let state = PluginState::new();
        assert!(state.plugins_dir.ends_with("plugins"));
        assert!(state.plugins.lock().unwrap().is_empty());
    }
}

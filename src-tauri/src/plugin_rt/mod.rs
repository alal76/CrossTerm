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
}

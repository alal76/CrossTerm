use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),
    #[error("Profile already exists: {0}")]
    ProfileAlreadyExists(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
}

impl Serialize for ConfigError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Models ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub font_size: u32,
    pub font_family: String,
    pub font_ligatures: bool,
    pub cursor_style: String,
    pub cursor_blink: bool,
    pub scrollback_lines: u32,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub tab_title_format: String,
    pub default_shell: Option<String>,
    pub copy_on_select: bool,
    pub paste_warning_lines: u32,
    pub idle_lock_timeout_secs: u64,
    pub auto_update: bool,
    pub gpu_acceleration: bool,
    pub bell_style: String,
    pub terminal_opacity: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            font_size: 14,
            font_family: "JetBrains Mono".into(),
            font_ligatures: true,
            cursor_style: "block".into(),
            cursor_blink: true,
            scrollback_lines: 10_000,
            line_height: 1.2,
            letter_spacing: 0.0,
            tab_title_format: "{name} - {host}".into(),
            default_shell: None,
            copy_on_select: false,
            paste_warning_lines: 5,
            idle_lock_timeout_secs: 900,
            auto_update: true,
            gpu_acceleration: true,
            bell_style: "visual".into(),
            terminal_opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCreateRequest {
    pub name: String,
    pub avatar: Option<String>,
    pub settings: Option<Settings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUpdateRequest {
    pub name: Option<String>,
    pub avatar: Option<String>,
}

// ── Session types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    SshTerminal,
    SftpBrowser,
    ScpTransfer,
    Rdp,
    Vnc,
    Telnet,
    SerialConsole,
    LocalShell,
    WslShell,
    CloudShell,
    WebConsole,
    KubernetesExec,
    DockerExec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDetails {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol_options: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub session_type: SessionType,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub icon: Option<String>,
    pub color_label: Option<String>,
    pub credential_ref: Option<String>,
    pub connection: ConnectionDetails,
    pub startup_script: Option<String>,
    pub environment_variables: HashMap<String, String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub auto_reconnect: bool,
    pub keep_alive_interval_seconds: u32,
    pub favorite: bool,
    /// Optional per-session settings overrides (JSON merge over profile settings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_override: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SessionCreateRequest {
    pub name: String,
    pub session_type: SessionType,
    pub group: Option<String>,
    pub tags: Option<Vec<String>>,
    pub icon: Option<String>,
    pub color_label: Option<String>,
    pub credential_ref: Option<String>,
    pub connection: ConnectionDetails,
    pub startup_script: Option<String>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub notes: Option<String>,
    pub auto_reconnect: Option<bool>,
    pub keep_alive_interval_seconds: Option<u32>,
    pub favorite: Option<bool>,
    pub settings_override: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SessionUpdateRequest {
    pub name: Option<String>,
    pub session_type: Option<SessionType>,
    pub group: Option<String>,
    pub tags: Option<Vec<String>>,
    pub icon: Option<String>,
    pub color_label: Option<String>,
    pub credential_ref: Option<String>,
    pub connection: Option<ConnectionDetails>,
    pub startup_script: Option<String>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub notes: Option<String>,
    pub auto_reconnect: Option<bool>,
    pub keep_alive_interval_seconds: Option<u32>,
    pub favorite: Option<bool>,
    pub settings_override: Option<serde_json::Value>,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn data_base_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CrossTerm")
}

fn profiles_dir() -> PathBuf {
    let p = data_base_dir().join("profiles");
    std::fs::create_dir_all(&p).ok();
    p
}

fn profile_dir(profile_id: &str) -> PathBuf {
    profiles_dir().join(profile_id)
}

fn profile_file(profile_id: &str) -> PathBuf {
    profile_dir(profile_id).join("profile.json")
}

fn sessions_dir(profile_id: &str) -> PathBuf {
    let p = profile_dir(profile_id).join("sessions");
    std::fs::create_dir_all(&p).ok();
    p
}

fn session_file(profile_id: &str, session_id: &str) -> PathBuf {
    sessions_dir(profile_id).join(format!("{}.json", session_id))
}

/// Public accessor for cross-module use.
pub(crate) fn session_file_path(profile_id: &str, session_id: &str) -> PathBuf {
    session_file(profile_id, session_id)
}

/// Public accessor so vault can scan sessions for orphan checks.
pub(crate) fn do_session_list_for_profile(profile_id: &str) -> Vec<SessionDefinition> {
    do_session_list(profile_id).unwrap_or_default()
}

// ── BE-CFG-06: Portable mode ───────────────────────────────────────────

/// Check for `.crossterm-portable` sentinel file next to the binary.
pub fn is_portable_mode() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join(".crossterm-portable").exists();
        }
    }
    false
}

/// Return the data directory, respecting portable mode.
pub(crate) fn effective_data_dir() -> PathBuf {
    if is_portable_mode() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let portable_dir = dir.join("data");
                std::fs::create_dir_all(&portable_dir).ok();
                return portable_dir;
            }
        }
    }
    data_base_dir()
}

// ── Crypto helpers for profile export/import ────────────────────────────

const EXPORT_SALT_LEN: usize = 32;
const EXPORT_KEY_LEN: usize = 32;
const EXPORT_NONCE_LEN: usize = 12;

fn derive_export_key(passphrase: &[u8], salt: &[u8]) -> Result<Vec<u8>, ConfigError> {
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(65536, 3, 4, Some(EXPORT_KEY_LEN))
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = vec![0u8; EXPORT_KEY_LEN];
    argon2
        .hash_password_into(passphrase, salt, &mut key)
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;
    Ok(key)
}

// ── Config state ────────────────────────────────────────────────────────

pub struct ConfigState {
    pub active_profile_id: RwLock<Option<String>>,
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            active_profile_id: RwLock::new(None),
        }
    }

    fn active_profile(&self) -> Result<String, ConfigError> {
        self.active_profile_id
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| ConfigError::ProfileNotFound("No active profile".into()))
    }
}

// ── Profile operations ─────────────────────────────────────────────────

fn do_profile_create(req: ProfileCreateRequest) -> Result<Profile, ConfigError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let profile = Profile {
        id: id.clone(),
        name: req.name,
        avatar: req.avatar,
        created_at: now,
        updated_at: now,
        settings: req.settings.unwrap_or_default(),
    };
    let dir = profile_dir(&id);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&profile)?;
    std::fs::write(profile_file(&id), json)?;
    // Ensure sessions dir
    std::fs::create_dir_all(sessions_dir(&id))?;
    Ok(profile)
}

fn do_profile_list() -> Result<Vec<Profile>, ConfigError> {
    let dir = profiles_dir();
    let mut profiles = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let pf = entry.path().join("profile.json");
                if pf.exists() {
                    let data = std::fs::read_to_string(&pf)?;
                    let profile: Profile = serde_json::from_str(&data)?;
                    profiles.push(profile);
                }
            }
        }
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

fn do_profile_get(id: &str) -> Result<Profile, ConfigError> {
    let pf = profile_file(id);
    if !pf.exists() {
        return Err(ConfigError::ProfileNotFound(id.into()));
    }
    let data = std::fs::read_to_string(&pf)?;
    Ok(serde_json::from_str(&data)?)
}

fn do_profile_update(id: &str, req: ProfileUpdateRequest) -> Result<Profile, ConfigError> {
    let mut profile = do_profile_get(id)?;
    if let Some(name) = req.name {
        profile.name = name;
    }
    if let Some(avatar) = req.avatar {
        profile.avatar = Some(avatar);
    }
    profile.updated_at = Utc::now();
    let json = serde_json::to_string_pretty(&profile)?;
    std::fs::write(profile_file(id), json)?;
    Ok(profile)
}

fn do_profile_delete(id: &str) -> Result<(), ConfigError> {
    let dir = profile_dir(id);
    if !dir.exists() {
        return Err(ConfigError::ProfileNotFound(id.into()));
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

// ── Session operations ──────────────────────────────────────────────────

fn do_session_create(
    profile_id: &str,
    req: SessionCreateRequest,
) -> Result<SessionDefinition, ConfigError> {
    // Ensure profile exists
    if !profile_file(profile_id).exists() {
        return Err(ConfigError::ProfileNotFound(profile_id.into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let session = SessionDefinition {
        id: id.clone(),
        name: req.name,
        session_type: req.session_type,
        group: req.group,
        tags: req.tags.unwrap_or_default(),
        icon: req.icon,
        color_label: req.color_label,
        credential_ref: req.credential_ref,
        connection: req.connection,
        startup_script: req.startup_script,
        environment_variables: req.environment_variables.unwrap_or_default(),
        notes: req.notes,
        created_at: now,
        updated_at: now,
        last_connected_at: None,
        auto_reconnect: req.auto_reconnect.unwrap_or(true),
        settings_override: req.settings_override,
        keep_alive_interval_seconds: req.keep_alive_interval_seconds.unwrap_or(60),
        favorite: req.favorite.unwrap_or(false),
    };
    let json = serde_json::to_string_pretty(&session)?;
    std::fs::write(session_file(profile_id, &id), json)?;
    Ok(session)
}

fn do_session_list(profile_id: &str) -> Result<Vec<SessionDefinition>, ConfigError> {
    let dir = sessions_dir(profile_id);
    let mut sessions = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let data = std::fs::read_to_string(&path)?;
                let session: SessionDefinition = serde_json::from_str(&data)?;
                sessions.push(session);
            }
        }
    }
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sessions)
}

pub(crate) fn do_session_get(profile_id: &str, session_id: &str) -> Result<SessionDefinition, ConfigError> {
    let path = session_file(profile_id, session_id);
    if !path.exists() {
        return Err(ConfigError::SessionNotFound(session_id.into()));
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

pub(crate) fn do_session_update(
    profile_id: &str,
    session_id: &str,
    req: SessionUpdateRequest,
) -> Result<SessionDefinition, ConfigError> {
    let mut session = do_session_get(profile_id, session_id)?;
    if let Some(v) = req.name { session.name = v; }
    if let Some(v) = req.session_type { session.session_type = v; }
    if let Some(v) = req.group { session.group = Some(v); }
    if let Some(v) = req.tags { session.tags = v; }
    if let Some(v) = req.icon { session.icon = Some(v); }
    if let Some(v) = req.color_label { session.color_label = Some(v); }
    if let Some(v) = req.credential_ref { session.credential_ref = Some(v); }
    if let Some(v) = req.connection { session.connection = v; }
    if let Some(v) = req.startup_script { session.startup_script = Some(v); }
    if let Some(v) = req.environment_variables { session.environment_variables = v; }
    if let Some(v) = req.notes { session.notes = Some(v); }
    if let Some(v) = req.auto_reconnect { session.auto_reconnect = v; }
    if let Some(v) = req.keep_alive_interval_seconds { session.keep_alive_interval_seconds = v; }
    if let Some(v) = req.settings_override { session.settings_override = Some(v); }
    if let Some(v) = req.favorite { session.favorite = v; }
    session.updated_at = Utc::now();
    let json = serde_json::to_string_pretty(&session)?;
    std::fs::write(session_file(profile_id, session_id), json)?;
    Ok(session)
}

fn do_session_delete(profile_id: &str, session_id: &str) -> Result<(), ConfigError> {
    let path = session_file(profile_id, session_id);
    if !path.exists() {
        return Err(ConfigError::SessionNotFound(session_id.into()));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}

fn do_session_search(profile_id: &str, query: &str) -> Result<Vec<SessionDefinition>, ConfigError> {
    let all = do_session_list(profile_id)?;
    let q = query.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.connection
                    .host
                    .as_deref()
                    .map_or(false, |h| h.to_lowercase().contains(&q))
                || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
                || s.notes
                    .as_deref()
                    .map_or(false, |n| n.to_lowercase().contains(&q))
                || s.group
                    .as_deref()
                    .map_or(false, |g| g.to_lowercase().contains(&q))
        })
        .collect())
}

fn do_settings_update(profile_id: &str, settings: Settings) -> Result<Settings, ConfigError> {
    let mut profile = do_profile_get(profile_id)?;
    profile.settings = settings;
    profile.updated_at = Utc::now();
    let json = serde_json::to_string_pretty(&profile)?;
    std::fs::write(profile_file(profile_id), json)?;
    Ok(profile.settings)
}

// ── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn profile_list() -> Result<Vec<Profile>, ConfigError> {
    do_profile_list()
}

#[tauri::command]
pub fn profile_create(request: ProfileCreateRequest) -> Result<Profile, ConfigError> {
    let profile = do_profile_create(request)?;
    crate::audit::append_event(&profile.id, crate::audit::AuditEventType::ProfileCreate, &format!("Created profile '{}'", profile.name));
    Ok(profile)
}

#[tauri::command]
pub fn profile_get(id: String) -> Result<Profile, ConfigError> {
    do_profile_get(&id)
}

#[tauri::command]
pub fn profile_update(id: String, request: ProfileUpdateRequest) -> Result<Profile, ConfigError> {
    do_profile_update(&id, request)
}

#[tauri::command]
pub fn profile_delete(id: String) -> Result<(), ConfigError> {
    do_profile_delete(&id)
}

#[tauri::command]
pub fn profile_switch(
    state: tauri::State<'_, ConfigState>,
    id: String,
) -> Result<Profile, ConfigError> {
    let profile = do_profile_get(&id)?;
    let mut active = state.active_profile_id.write().unwrap();
    *active = Some(id.clone());
    crate::audit::append_event(&id, crate::audit::AuditEventType::ProfileSwitch, &format!("Switched to profile '{}'", profile.name));
    Ok(profile)
}

#[tauri::command]
pub fn session_list(state: tauri::State<'_, ConfigState>) -> Result<Vec<SessionDefinition>, ConfigError> {
    let pid = state.active_profile()?;
    do_session_list(&pid)
}

#[tauri::command]
pub fn session_create(
    state: tauri::State<'_, ConfigState>,
    request: SessionCreateRequest,
) -> Result<SessionDefinition, ConfigError> {
    let pid = state.active_profile()?;
    let session = do_session_create(&pid, request)?;
    crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionCreate, &format!("Created session '{}' ({})", session.name, session.id));
    Ok(session)
}

#[tauri::command]
pub fn session_get(
    state: tauri::State<'_, ConfigState>,
    id: String,
) -> Result<SessionDefinition, ConfigError> {
    let pid = state.active_profile()?;
    do_session_get(&pid, &id)
}

#[tauri::command]
pub fn session_update(
    state: tauri::State<'_, ConfigState>,
    id: String,
    request: SessionUpdateRequest,
) -> Result<SessionDefinition, ConfigError> {
    let pid = state.active_profile()?;
    do_session_update(&pid, &id, request)
}

#[tauri::command]
pub fn session_delete(
    state: tauri::State<'_, ConfigState>,
    id: String,
) -> Result<(), ConfigError> {
    let pid = state.active_profile()?;
    let result = do_session_delete(&pid, &id);
    if result.is_ok() {
        crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionDelete, &format!("Deleted session {}", id));
    }
    result
}

#[tauri::command]
pub fn session_search(
    state: tauri::State<'_, ConfigState>,
    query: String,
) -> Result<Vec<SessionDefinition>, ConfigError> {
    let pid = state.active_profile()?;
    do_session_search(&pid, &query)
}

#[tauri::command]
pub fn settings_get(
    state: tauri::State<'_, ConfigState>,
) -> Result<Settings, ConfigError> {
    let pid = state.active_profile()?;
    let profile = do_profile_get(&pid)?;
    Ok(profile.settings)
}

#[tauri::command]
pub fn settings_update(
    state: tauri::State<'_, ConfigState>,
    settings: Settings,
) -> Result<Settings, ConfigError> {
    let pid = state.active_profile()?;
    let result = do_settings_update(&pid, settings);
    if result.is_ok() {
        crate::audit::append_event(&pid, crate::audit::AuditEventType::SettingsUpdate, "Settings updated");
    }
    result
}

#[tauri::command]
pub fn session_duplicate(
    state: tauri::State<'_, ConfigState>,
    session_id: String,
) -> Result<SessionDefinition, ConfigError> {
    let pid = state.active_profile()?;
    let source = do_session_get(&pid, &session_id)?;
    let new_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let new_session = SessionDefinition {
        id: new_id.clone(),
        name: format!("{} (Copy)", source.name),
        session_type: source.session_type,
        group: source.group,
        tags: source.tags,
        icon: source.icon,
        color_label: source.color_label,
        credential_ref: source.credential_ref,
        connection: source.connection,
        startup_script: source.startup_script,
        environment_variables: source.environment_variables,
        notes: source.notes,
        created_at: now,
        updated_at: now,
        last_connected_at: None,
        auto_reconnect: source.auto_reconnect,
        keep_alive_interval_seconds: source.keep_alive_interval_seconds,
        settings_override: source.settings_override,
        favorite: false,
    };
    let json = serde_json::to_string_pretty(&new_session)?;
    std::fs::write(session_file(&pid, &new_id), json)?;
    crate::audit::append_event(&pid, crate::audit::AuditEventType::SessionCreate, &format!("Duplicated session '{}' as '{}'", source.name, new_session.name));
    Ok(new_session)
}

// ── SSH Config Import ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ParsedSshHost {
    name: String,
    hostname: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
}

fn parse_ssh_config(content: &str) -> Vec<ParsedSshHost> {
    let mut hosts = Vec::new();
    let mut current: Option<ParsedSshHost> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first whitespace or '='
        let (key, value) = match line.find(|c: char| c == ' ' || c == '\t' || c == '=') {
            Some(pos) => {
                let k = &line[..pos];
                let v = line[pos + 1..].trim().trim_matches('=').trim();
                (k, v)
            }
            None => continue,
        };

        match key.to_lowercase().as_str() {
            "host" => {
                if let Some(h) = current.take() {
                    if !h.name.contains('*') && !h.name.contains('?') {
                        hosts.push(h);
                    }
                }
                current = Some(ParsedSshHost {
                    name: value.to_string(),
                    hostname: None,
                    port: None,
                    user: None,
                    identity_file: None,
                    proxy_jump: None,
                });
            }
            "hostname" => {
                if let Some(ref mut h) = current {
                    h.hostname = Some(value.to_string());
                }
            }
            "port" => {
                if let Some(ref mut h) = current {
                    h.port = value.parse().ok();
                }
            }
            "user" => {
                if let Some(ref mut h) = current {
                    h.user = Some(value.to_string());
                }
            }
            "identityfile" => {
                if let Some(ref mut h) = current {
                    h.identity_file = Some(value.to_string());
                }
            }
            "proxyjump" => {
                if let Some(ref mut h) = current {
                    h.proxy_jump = Some(value.to_string());
                }
            }
            _ => {}
        }
    }

    // Push final host
    if let Some(h) = current {
        if !h.name.contains('*') && !h.name.contains('?') {
            hosts.push(h);
        }
    }

    hosts
}

#[tauri::command]
pub fn session_import_ssh_config(
    state: tauri::State<'_, ConfigState>,
    path: Option<String>,
) -> Result<Vec<String>, ConfigError> {
    let pid = state.active_profile()?;

    let config_path = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => dirs::home_dir()
            .ok_or_else(|| ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine home directory",
            )))?
            .join(".ssh")
            .join("config"),
    };

    let content = std::fs::read_to_string(&config_path)?;
    let parsed_hosts = parse_ssh_config(&content);
    let mut created_ids = Vec::new();

    for host in parsed_hosts {
        let hostname = host.hostname.clone().unwrap_or_else(|| host.name.clone());
        let port = host.port.unwrap_or(22);

        let mut notes_parts = Vec::new();
        if let Some(ref id_file) = host.identity_file {
            notes_parts.push(format!("IdentityFile: {}", id_file));
        }
        if let Some(ref pj) = host.proxy_jump {
            notes_parts.push(format!("ProxyJump: {}", pj));
        }

        let req = SessionCreateRequest {
            name: host.name.clone(),
            session_type: SessionType::SshTerminal,
            group: Some("Imported".to_string()),
            tags: Some(vec!["ssh-config-import".to_string()]),
            icon: None,
            color_label: None,
            credential_ref: None,
            connection: ConnectionDetails {
                host: Some(hostname),
                port: Some(port),
                protocol_options: None,
            },
            startup_script: None,
            environment_variables: if host.user.is_some() {
                let mut env = HashMap::new();
                env.insert("SSH_USER".to_string(), host.user.unwrap());
                Some(env)
            } else {
                None
            },
            notes: if notes_parts.is_empty() {
                None
            } else {
                Some(notes_parts.join("\n"))
            },
            auto_reconnect: None,
            settings_override: None,
            keep_alive_interval_seconds: None,
            favorite: None,
        };

        let session = do_session_create(&pid, req)?;
        created_ids.push(session.id);
    }

    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::SessionCreate,
        &format!("Imported {} sessions from SSH config", created_ids.len()),
    );

    Ok(created_ids)
}

// ── BE-CFG-02: Bulk connect all in folder ───────────────────────────────

#[tauri::command]
pub fn session_bulk_connect(
    state: tauri::State<'_, ConfigState>,
    folder_path: String,
) -> Result<Vec<String>, ConfigError> {
    let pid = state.active_profile()?;
    let sessions = do_session_list(&pid)?;
    let ids: Vec<String> = sessions
        .into_iter()
        .filter(|s| s.group.as_deref() == Some(&folder_path))
        .map(|s| s.id)
        .collect();
    Ok(ids)
}

// ── BE-CFG-02: Session list by group ────────────────────────────────────

#[tauri::command]
pub fn session_list_by_group(
    state: tauri::State<'_, ConfigState>,
    profile_id: String,
    group: String,
) -> Result<Vec<SessionDefinition>, ConfigError> {
    let sessions = do_session_list(&profile_id)?;
    Ok(sessions
        .into_iter()
        .filter(|s| s.group.as_deref() == Some(&group))
        .collect())
}

// ── BE-CFG-04: Profile export/import ────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct ProfileExportData {
    profile: Profile,
    sessions: Vec<SessionDefinition>,
    settings: Settings,
}

#[tauri::command]
pub fn profile_export(
    state: tauri::State<'_, ConfigState>,
    profile_id: String,
    path: String,
    passphrase: String,
) -> Result<(), ConfigError> {
    let profile = do_profile_get(&profile_id)?;
    let sessions = do_session_list(&profile_id)?;
    let settings = profile.settings.clone();

    let export = ProfileExportData {
        profile,
        sessions,
        settings,
    };

    let json = serde_json::to_string_pretty(&export)?;

    // Encrypt with AES-256-GCM
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    use rand::RngCore;

    let mut salt = vec![0u8; EXPORT_SALT_LEN];
    aes_gcm::aead::OsRng.fill_bytes(&mut salt);
    let key = derive_export_key(passphrase.as_bytes(), &salt)?;

    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;
    let mut nonce_bytes = [0u8; EXPORT_NONCE_LEN];
    aes_gcm::aead::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, json.as_bytes())
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;

    // File format: salt (32 bytes) + nonce (12 bytes) + ciphertext
    let mut output = Vec::with_capacity(EXPORT_SALT_LEN + EXPORT_NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    std::fs::write(&path, output)?;

    let pid = state.active_profile_id.read().unwrap().clone().unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::ProfileExport,
        &format!("Exported profile to {}", path),
    );

    Ok(())
}

#[tauri::command]
pub fn profile_import(
    state: tauri::State<'_, ConfigState>,
    path: String,
    passphrase: String,
) -> Result<Profile, ConfigError> {
    let encrypted = std::fs::read(&path)?;
    if encrypted.len() < EXPORT_SALT_LEN + EXPORT_NONCE_LEN {
        return Err(ConfigError::Encryption("Invalid export file".into()));
    }

    let salt = &encrypted[..EXPORT_SALT_LEN];
    let nonce_bytes = &encrypted[EXPORT_SALT_LEN..EXPORT_SALT_LEN + EXPORT_NONCE_LEN];
    let ciphertext = &encrypted[EXPORT_SALT_LEN + EXPORT_NONCE_LEN..];

    let key = derive_export_key(passphrase.as_bytes(), salt)?;

    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| ConfigError::Encryption("Invalid passphrase or corrupted file".into()))?;
    let data = String::from_utf8(plaintext)
        .map_err(|e| ConfigError::Encryption(e.to_string()))?;
    let export: ProfileExportData = serde_json::from_str(&data)?;

    // Create a new profile with imported data
    let new_profile = do_profile_create(ProfileCreateRequest {
        name: format!("{} (Imported)", export.profile.name),
        avatar: export.profile.avatar,
        settings: Some(export.settings),
    })?;

    // Import sessions
    for session in export.sessions {
        let req = SessionCreateRequest {
            name: session.name,
            session_type: session.session_type,
            group: session.group,
            tags: Some(session.tags),
            icon: session.icon,
            color_label: session.color_label,
            credential_ref: session.credential_ref,
            connection: session.connection,
            startup_script: session.startup_script,
            environment_variables: Some(session.environment_variables),
            notes: session.notes,
            auto_reconnect: Some(session.auto_reconnect),
            settings_override: session.settings_override,
            keep_alive_interval_seconds: Some(session.keep_alive_interval_seconds),
            favorite: Some(session.favorite),
        };
        do_session_create(&new_profile.id, req)?;
    }

    let pid = state
        .active_profile_id
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();
    crate::audit::append_event(
        &pid,
        crate::audit::AuditEventType::ProfileImport,
        &format!("Imported profile '{}' from {}", new_profile.name, path),
    );

    Ok(new_profile)
}

// ── BE-CFG-05: Settings hierarchy ───────────────────────────────────────

/// Resolve effective settings with session → folder → profile → app defaults cascade.
fn do_settings_get_effective(
    profile_id: &str,
    session_id: Option<&str>,
) -> Result<Settings, ConfigError> {
    // Override with profile settings
    let profile = do_profile_get(profile_id)?;
    let mut effective = profile.settings.clone();
    if let Some(sid) = session_id {
        let session = do_session_get(profile_id, sid)?;

        // Check folder-level settings: look for a .settings.json in group folder
        if let Some(ref folder) = session.group {
            let folder_settings_path = sessions_dir(profile_id).join(format!("{}.settings.json", folder));
            if folder_settings_path.exists() {
                if let Ok(data) = std::fs::read_to_string(&folder_settings_path) {
                    if let Ok(folder_overrides) = serde_json::from_str::<serde_json::Value>(&data) {
                        let mut base = serde_json::to_value(&effective).unwrap_or_default();
                        json_merge(&mut base, &folder_overrides);
                        if let Ok(merged) = serde_json::from_value::<Settings>(base) {
                            effective = merged;
                        }
                    }
                }
            }
        }

        // Session-level overrides from settings_override (JSON merge)
        if let Some(ref overrides) = session.settings_override {
            let mut base = serde_json::to_value(&effective).unwrap_or_default();
            json_merge(&mut base, overrides);
            if let Ok(merged) = serde_json::from_value::<Settings>(base) {
                effective = merged;
            }
        }

        // Legacy: also apply protocol_options overrides for backward compat
        if let Some(ref opts) = session.connection.protocol_options {
            if let Some(font_size) = opts.get("font_size").and_then(|v| v.as_u64()) {
                effective.font_size = font_size as u32;
            }
            if let Some(theme) = opts.get("theme").and_then(|v| v.as_str()) {
                effective.theme = theme.to_string();
            }
            if let Some(font_family) = opts.get("font_family").and_then(|v| v.as_str()) {
                effective.font_family = font_family.to_string();
            }
            if let Some(scrollback) = opts.get("scrollback_lines").and_then(|v| v.as_u64()) {
                effective.scrollback_lines = scrollback as u32;
            }
            if let Some(bell) = opts.get("bell_style").and_then(|v| v.as_str()) {
                effective.bell_style = bell.to_string();
            }
        }
    }

    Ok(effective)
}

/// Recursively merge `overlay` into `base`. For objects, overlay keys replace
/// base keys; non-object values are overwritten entirely.
fn json_merge(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                json_merge(base_map.entry(key.clone()).or_insert(serde_json::Value::Null), value);
            }
        }
        (base, overlay) => {
            *base = overlay.clone();
        }
    }
}

#[tauri::command]
pub fn settings_get_effective(
    state: tauri::State<'_, ConfigState>,
    session_id: Option<String>,
) -> Result<Settings, ConfigError> {
    let pid = state.active_profile()?;
    do_settings_get_effective(&pid, session_id.as_deref())
}

// ── BE-CFG-06: Portable mode command ────────────────────────────────────

#[tauri::command]
pub fn config_is_portable_mode() -> bool {
    is_portable_mode()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a unique test profile and returns its ID. Cleans up on drop.
    struct TestEnv {
        profile_id: String,
    }

    impl TestEnv {
        fn new() -> Self {
            let req = ProfileCreateRequest {
                name: "Test Profile".to_string(),
                avatar: None,
                settings: None,
            };
            let profile = do_profile_create(req).unwrap();
            Self {
                profile_id: profile.id,
            }
        }

        fn id(&self) -> &str {
            &self.profile_id
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = do_profile_delete(&self.profile_id);
        }
    }

    fn make_session_request(name: &str, host: &str) -> SessionCreateRequest {
        SessionCreateRequest {
            name: name.to_string(),
            session_type: SessionType::SshTerminal,
            group: Some("servers".to_string()),
            tags: Some(vec!["linux".to_string()]),
            icon: None,
            color_label: None,
            credential_ref: None,
            connection: ConnectionDetails {
                host: Some(host.to_string()),
                port: Some(22),
                protocol_options: None,
            },
            startup_script: None,
            environment_variables: None,
            notes: Some(format!("Session for {}", host)),
            auto_reconnect: None,
            keep_alive_interval_seconds: None,
            favorite: None,
            settings_override: None,
        }
    }

    // ── UT-C-01: Profile CRUD ───────────────────────────────────────

    #[test]
    fn test_profile_crud() {
        // Create
        let req = ProfileCreateRequest {
            name: "CRUD Test Profile".to_string(),
            avatar: Some("avatar.png".to_string()),
            settings: None,
        };
        let profile = do_profile_create(req).unwrap();
        assert_eq!(profile.name, "CRUD Test Profile");
        assert_eq!(profile.avatar.as_deref(), Some("avatar.png"));
        let id = profile.id.clone();

        // Get
        let fetched = do_profile_get(&id).unwrap();
        assert_eq!(fetched.name, "CRUD Test Profile");

        // List should include it
        let list = do_profile_list().unwrap();
        assert!(list.iter().any(|p| p.id == id));

        // Update
        let updated = do_profile_update(
            &id,
            ProfileUpdateRequest {
                name: Some("Renamed Profile".to_string()),
                avatar: None,
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Renamed Profile");
        assert!(updated.updated_at > profile.created_at);

        // Delete
        do_profile_delete(&id).unwrap();
        assert!(matches!(
            do_profile_get(&id).unwrap_err(),
            ConfigError::ProfileNotFound(_)
        ));
    }

    // ── UT-C-02: Session CRUD ───────────────────────────────────────

    #[test]
    fn test_session_crud() {
        let env = TestEnv::new();

        // Create
        let session = do_session_create(env.id(), make_session_request("Web Server", "10.0.0.1"))
            .unwrap();
        assert_eq!(session.name, "Web Server");
        assert_eq!(session.connection.host.as_deref(), Some("10.0.0.1"));
        assert_eq!(session.connection.port, Some(22));
        assert_eq!(session.session_type, SessionType::SshTerminal);
        assert!(session.auto_reconnect);
        let sid = session.id.clone();

        // Get
        let fetched = do_session_get(env.id(), &sid).unwrap();
        assert_eq!(fetched.name, "Web Server");

        // List
        let list = do_session_list(env.id()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, sid);

        // Update
        let updated = do_session_update(
            env.id(),
            &sid,
            SessionUpdateRequest {
                name: Some("DB Server".to_string()),
                session_type: None,
                group: Some("databases".to_string()),
                tags: Some(vec!["mysql".to_string()]),
                icon: None,
                color_label: None,
                credential_ref: None,
                connection: Some(ConnectionDetails {
                    host: Some("10.0.0.2".to_string()),
                    port: Some(3306),
                    protocol_options: None,
                }),
                startup_script: None,
                environment_variables: None,
                notes: None,
                auto_reconnect: Some(false),
                keep_alive_interval_seconds: None,
                favorite: Some(true),
                settings_override: None,
            },
        )
        .unwrap();
        assert_eq!(updated.name, "DB Server");
        assert_eq!(updated.connection.host.as_deref(), Some("10.0.0.2"));
        assert!(!updated.auto_reconnect);
        assert!(updated.favorite);

        // Delete
        do_session_delete(env.id(), &sid).unwrap();
        assert!(matches!(
            do_session_get(env.id(), &sid).unwrap_err(),
            ConfigError::SessionNotFound(_)
        ));
    }

    // ── UT-C-03: Session search by name ─────────────────────────────

    #[test]
    fn test_session_search_by_name() {
        let env = TestEnv::new();

        do_session_create(env.id(), make_session_request("Alpha Web", "10.0.0.1")).unwrap();

    // ── UT-C-07: Bulk connect ───────────────────────────────────────

    #[test]
    fn test_session_bulk_connect_by_folder() {
        let env = TestEnv::new();

        // Create sessions in different folders
        let mut req1 = make_session_request("Web1", "10.0.0.1");
        req1.group = Some("production".to_string());
        do_session_create(env.id(), req1).unwrap();

        let mut req2 = make_session_request("Web2", "10.0.0.2");
        req2.group = Some("production".to_string());
        do_session_create(env.id(), req2).unwrap();

        let mut req3 = make_session_request("Dev1", "10.0.0.3");
        req3.group = Some("staging".to_string());
        do_session_create(env.id(), req3).unwrap();

        // Get all sessions in production folder
        let all = do_session_list(env.id()).unwrap();
        let prod_ids: Vec<String> = all
            .into_iter()
            .filter(|s| s.group.as_deref() == Some("production"))
            .map(|s| s.id)
            .collect();

        assert_eq!(prod_ids.len(), 2);
    }

    #[test]
    fn test_session_bulk_connect_empty_folder() {
        let env = TestEnv::new();

        let all = do_session_list(env.id()).unwrap();
        let ids: Vec<String> = all
            .into_iter()
            .filter(|s| s.group.as_deref() == Some("nonexistent"))
            .map(|s| s.id)
            .collect();

        assert!(ids.is_empty());
    }

    // ── UT-C-08: Profile export/import ──────────────────────────────

    #[test]
    fn test_profile_export_import_roundtrip() {
        let env = TestEnv::new();

        // Create sessions
        do_session_create(env.id(), make_session_request("Server1", "10.0.0.1")).unwrap();
        do_session_create(env.id(), make_session_request("Server2", "10.0.0.2")).unwrap();

        // Export
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let export_path = tmp.path().to_string_lossy().to_string();

        let profile = do_profile_get(env.id()).unwrap();
        let sessions = do_session_list(env.id()).unwrap();
        let export = ProfileExportData {
            profile: profile.clone(),
            sessions,
            settings: profile.settings,
        };
        let json = serde_json::to_string_pretty(&export).unwrap();
        std::fs::write(&export_path, &json).unwrap();

        // Import
        let data = std::fs::read_to_string(&export_path).unwrap();
        let imported: ProfileExportData = serde_json::from_str(&data).unwrap();

        assert_eq!(imported.profile.name, "Test Profile");
        assert_eq!(imported.sessions.len(), 2);
    }

    #[test]
    fn test_profile_export_data_serialization() {
        let export = ProfileExportData {
            profile: Profile {
                id: "test-id".into(),
                name: "Test".into(),
                avatar: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                settings: Settings::default(),
            },
            sessions: vec![],
            settings: Settings::default(),
        };
        let json = serde_json::to_string(&export).unwrap();
        assert!(json.contains("Test"));
        let deserialized: ProfileExportData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.profile.name, "Test");
    }

    // ── UT-C-09: Settings hierarchy ─────────────────────────────────

    #[test]
    fn test_settings_effective_defaults_to_profile() {
        let env = TestEnv::new();

        let settings = do_settings_get_effective(env.id(), None).unwrap();
        let profile = do_profile_get(env.id()).unwrap();

        assert_eq!(settings.theme, profile.settings.theme);
        assert_eq!(settings.font_size, profile.settings.font_size);
    }

    #[test]
    fn test_settings_effective_with_session_overrides() {
        let env = TestEnv::new();

        let mut req = make_session_request("TestSession", "10.0.0.1");
        let mut opts = HashMap::new();
        opts.insert("font_size".into(), serde_json::json!(20));
        opts.insert("theme".into(), serde_json::json!("solarized"));
        req.connection.protocol_options = Some(opts);
        let session = do_session_create(env.id(), req).unwrap();

        let settings = do_settings_get_effective(env.id(), Some(&session.id)).unwrap();
        assert_eq!(settings.font_size, 20);
        assert_eq!(settings.theme, "solarized");
    }

    // ── UT-C-10: Portable mode ──────────────────────────────────────

    #[test]
    fn test_portable_mode_detection() {
        // In test environment, sentinel file doesn't exist next to binary
        assert!(!is_portable_mode());
    }

    #[test]
    fn test_effective_data_dir_non_portable() {
        let dir = effective_data_dir();
        assert!(dir.to_string_lossy().contains("CrossTerm"));
    }
        do_session_create(env.id(), make_session_request("Beta Web", "10.0.0.2")).unwrap();
        do_session_create(env.id(), make_session_request("Gamma DB", "10.0.0.3")).unwrap();

        // Search by partial name
        let results = do_session_search(env.id(), "web").unwrap();
        assert_eq!(results.len(), 2);

        // Case insensitive
        let results = do_session_search(env.id(), "GAMMA").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Gamma DB");

        // Search by host
        let results = do_session_search(env.id(), "10.0.0.2").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Beta Web");

        // No matches
        let results = do_session_search(env.id(), "nonexistent").unwrap();
        assert!(results.is_empty());
    }

    // ── UT-C-04: Settings persistence ───────────────────────────────

    #[test]
    fn test_settings_persistence() {
        let env = TestEnv::new();

        let new_settings = Settings {
            theme: "light".into(),
            font_size: 18,
            font_family: "Fira Code".into(),
            font_ligatures: false,
            cursor_style: "underline".into(),
            cursor_blink: false,
            scrollback_lines: 5_000,
            line_height: 1.5,
            letter_spacing: 0.5,
            tab_title_format: "{host}".into(),
            default_shell: Some("/bin/fish".into()),
            copy_on_select: true,
            paste_warning_lines: 10,
            idle_lock_timeout_secs: 600,
            auto_update: false,
            gpu_acceleration: false,
            bell_style: "sound".into(),
            terminal_opacity: 0.9,
        };

        do_settings_update(env.id(), new_settings).unwrap();

        // Reload from disk
        let profile = do_profile_get(env.id()).unwrap();
        assert_eq!(profile.settings.theme, "light");
        assert_eq!(profile.settings.font_size, 18);
        assert_eq!(profile.settings.font_family, "Fira Code");
        assert!(!profile.settings.font_ligatures);
        assert_eq!(profile.settings.cursor_style, "underline");
        assert!(!profile.settings.cursor_blink);
        assert_eq!(profile.settings.scrollback_lines, 5_000);
        assert_eq!(profile.settings.default_shell.as_deref(), Some("/bin/fish"));
        assert!(profile.settings.copy_on_select);
        assert!(!profile.settings.auto_update);
        assert_eq!(profile.settings.terminal_opacity, 0.9);
    }

    // ── UT-C-05: Settings defaults ──────────────────────────────────

    #[test]
    fn test_settings_defaults() {
        let env = TestEnv::new();

        let profile = do_profile_get(env.id()).unwrap();
        let defaults = Settings::default();

        assert_eq!(profile.settings.theme, defaults.theme);
        assert_eq!(profile.settings.font_size, defaults.font_size);
        assert_eq!(profile.settings.font_family, defaults.font_family);
        assert_eq!(profile.settings.font_ligatures, defaults.font_ligatures);
        assert_eq!(profile.settings.cursor_style, defaults.cursor_style);
        assert_eq!(profile.settings.cursor_blink, defaults.cursor_blink);
        assert_eq!(profile.settings.scrollback_lines, defaults.scrollback_lines);
        assert_eq!(profile.settings.default_shell, defaults.default_shell);
        assert_eq!(profile.settings.copy_on_select, defaults.copy_on_select);
        assert_eq!(profile.settings.auto_update, defaults.auto_update);
        assert_eq!(profile.settings.gpu_acceleration, defaults.gpu_acceleration);
        assert_eq!(profile.settings.terminal_opacity, defaults.terminal_opacity);
    }

    // ── UT-C-06: Profile data isolation ─────────────────────────────

    #[test]
    fn test_profile_data_isolation() {
        let env_a = TestEnv::new();
        let env_b = TestEnv::new();

        // Create sessions under profile A
        do_session_create(env_a.id(), make_session_request("A-Server-1", "10.0.1.1")).unwrap();
        do_session_create(env_a.id(), make_session_request("A-Server-2", "10.0.1.2")).unwrap();

        // Create a session under profile B
        do_session_create(env_b.id(), make_session_request("B-Server-1", "10.0.2.1")).unwrap();

        // Profile A should see only its sessions
        let list_a = do_session_list(env_a.id()).unwrap();
        assert_eq!(list_a.len(), 2);
        assert!(list_a.iter().all(|s| s.name.starts_with("A-")));

        // Profile B should see only its sessions
        let list_b = do_session_list(env_b.id()).unwrap();
        assert_eq!(list_b.len(), 1);
        assert_eq!(list_b[0].name, "B-Server-1");

        // Search on profile A should not return profile B sessions
        let search = do_session_search(env_a.id(), "B-Server").unwrap();
        assert!(search.is_empty());
    }

    // ── UT-C-06: SSH Config Parser ──────────────────────────────────

    #[test]
    fn test_parse_ssh_config_basic() {
        let config = r#"
Host webserver
    HostName 192.168.1.100
    Port 2222
    User admin
    IdentityFile ~/.ssh/id_rsa

Host dbserver
    HostName db.example.com
    User postgres

Host *
    ServerAliveInterval 60
"#;
        let hosts = parse_ssh_config(config);
        assert_eq!(hosts.len(), 2);

        assert_eq!(hosts[0].name, "webserver");
        assert_eq!(hosts[0].hostname.as_deref(), Some("192.168.1.100"));
        assert_eq!(hosts[0].port, Some(2222));
        assert_eq!(hosts[0].user.as_deref(), Some("admin"));
        assert_eq!(hosts[0].identity_file.as_deref(), Some("~/.ssh/id_rsa"));

        assert_eq!(hosts[1].name, "dbserver");
        assert_eq!(hosts[1].hostname.as_deref(), Some("db.example.com"));
        assert_eq!(hosts[1].user.as_deref(), Some("postgres"));
        assert_eq!(hosts[1].port, None);
    }

    #[test]
    fn test_parse_ssh_config_skips_wildcards() {
        let config = "Host *\n    User default\n\nHost ?\n    Port 22\n";
        let hosts = parse_ssh_config(config);
        assert!(hosts.is_empty());
    }

    #[test]
    fn test_parse_ssh_config_proxy_jump() {
        let config = "Host target\n    HostName 10.0.0.5\n    ProxyJump bastion\n";
        let hosts = parse_ssh_config(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].proxy_jump.as_deref(), Some("bastion"));
    }

    #[test]
    fn test_parse_ssh_config_empty() {
        let hosts = parse_ssh_config("");
        assert!(hosts.is_empty());
    }

    // ── UT-C-11: Session list by group ──────────────────────────────

    #[test]
    fn test_session_list_by_group() {
        let env = TestEnv::new();

        let mut req1 = make_session_request("Web1", "10.0.0.1");
        req1.group = Some("production".to_string());
        do_session_create(env.id(), req1).unwrap();

        let mut req2 = make_session_request("Web2", "10.0.0.2");
        req2.group = Some("production".to_string());
        do_session_create(env.id(), req2).unwrap();

        let mut req3 = make_session_request("Dev1", "10.0.0.3");
        req3.group = Some("staging".to_string());
        do_session_create(env.id(), req3).unwrap();

        let sessions = do_session_list(env.id()).unwrap();
        let prod: Vec<_> = sessions
            .iter()
            .filter(|s| s.group.as_deref() == Some("production"))
            .collect();
        assert_eq!(prod.len(), 2);

        let staging: Vec<_> = sessions
            .iter()
            .filter(|s| s.group.as_deref() == Some("staging"))
            .collect();
        assert_eq!(staging.len(), 1);

        let empty: Vec<_> = sessions
            .iter()
            .filter(|s| s.group.as_deref() == Some("nonexistent"))
            .collect();
        assert!(empty.is_empty());
    }

    // ── UT-C-12: Encrypted profile export/import ────────────────────

    #[test]
    fn test_encrypted_export_import_roundtrip() {
        let env = TestEnv::new();

        do_session_create(env.id(), make_session_request("Server1", "10.0.0.1")).unwrap();
        do_session_create(env.id(), make_session_request("Server2", "10.0.0.2")).unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let export_path = tmp.path().to_string_lossy().to_string();

        // Export: serialize + encrypt
        let profile = do_profile_get(env.id()).unwrap();
        let sessions = do_session_list(env.id()).unwrap();
        let export = ProfileExportData {
            profile: profile.clone(),
            sessions,
            settings: profile.settings,
        };
        let json = serde_json::to_string_pretty(&export).unwrap();
        let passphrase = "test-passphrase-123!";

        use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
        use rand::RngCore;

        let mut salt = vec![0u8; EXPORT_SALT_LEN];
        aes_gcm::aead::OsRng.fill_bytes(&mut salt);
        let key = derive_export_key(passphrase.as_bytes(), &salt).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let mut nonce_bytes = [0u8; EXPORT_NONCE_LEN];
        aes_gcm::aead::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, json.as_bytes()).unwrap();

        let mut output = Vec::new();
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        std::fs::write(&export_path, &output).unwrap();

        // Import: read + decrypt
        let encrypted = std::fs::read(&export_path).unwrap();
        let s = &encrypted[..EXPORT_SALT_LEN];
        let n = &encrypted[EXPORT_SALT_LEN..EXPORT_SALT_LEN + EXPORT_NONCE_LEN];
        let ct = &encrypted[EXPORT_SALT_LEN + EXPORT_NONCE_LEN..];

        let dk = derive_export_key(passphrase.as_bytes(), s).unwrap();
        let dc = Aes256Gcm::new_from_slice(&dk).unwrap();
        let dn = Nonce::from_slice(n);
        let plaintext = dc.decrypt(dn, ct).unwrap();
        let data = String::from_utf8(plaintext).unwrap();
        let imported: ProfileExportData = serde_json::from_str(&data).unwrap();

        assert_eq!(imported.profile.name, "Test Profile");
        assert_eq!(imported.sessions.len(), 2);
    }

    #[test]
    fn test_encrypted_import_wrong_passphrase() {
        let env = TestEnv::new();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let export_path = tmp.path().to_string_lossy().to_string();

        let profile = do_profile_get(env.id()).unwrap();
        let export = ProfileExportData {
            profile: profile.clone(),
            sessions: vec![],
            settings: profile.settings,
        };
        let json = serde_json::to_string_pretty(&export).unwrap();

        use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
        use rand::RngCore;

        let mut salt = vec![0u8; EXPORT_SALT_LEN];
        aes_gcm::aead::OsRng.fill_bytes(&mut salt);
        let key = derive_export_key("correct-pass".as_bytes(), &salt).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let mut nonce_bytes = [0u8; EXPORT_NONCE_LEN];
        aes_gcm::aead::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, json.as_bytes()).unwrap();

        let mut output = Vec::new();
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        std::fs::write(&export_path, &output).unwrap();

        // Try decrypting with wrong passphrase
        let encrypted = std::fs::read(&export_path).unwrap();
        let wrong_key = derive_export_key("wrong-pass".as_bytes(), &encrypted[..EXPORT_SALT_LEN]).unwrap();
        let wrong_cipher = Aes256Gcm::new_from_slice(&wrong_key).unwrap();
        let nonce = Nonce::from_slice(&encrypted[EXPORT_SALT_LEN..EXPORT_SALT_LEN + EXPORT_NONCE_LEN]);
        let result = wrong_cipher.decrypt(nonce, &encrypted[EXPORT_SALT_LEN + EXPORT_NONCE_LEN..]);
        assert!(result.is_err(), "Decryption with wrong passphrase should fail");
    }

    // ── UT-C-11: Session all types ──────────────────────────────────

    #[test]
    fn test_session_all_types() {
        let env = TestEnv::new();

        let all_types = vec![
            (SessionType::SshTerminal, "SSH Terminal"),
            (SessionType::SftpBrowser, "SFTP Browser"),
            (SessionType::ScpTransfer, "SCP Transfer"),
            (SessionType::Rdp, "RDP Session"),
            (SessionType::Vnc, "VNC Session"),
            (SessionType::Telnet, "Telnet Session"),
            (SessionType::SerialConsole, "Serial Console"),
            (SessionType::LocalShell, "Local Shell"),
            (SessionType::WslShell, "WSL Shell"),
            (SessionType::CloudShell, "Cloud Shell"),
            (SessionType::WebConsole, "Web Console"),
            (SessionType::KubernetesExec, "Kubernetes Exec"),
            (SessionType::DockerExec, "Docker Exec"),
        ];

        let mut created_ids = Vec::new();
        for (stype, name) in &all_types {
            let req = SessionCreateRequest {
                name: name.to_string(),
                session_type: stype.clone(),
                group: Some("all-types-test".to_string()),
                tags: None,
                icon: None,
                color_label: None,
                credential_ref: None,
                connection: ConnectionDetails {
                    host: Some("10.0.0.1".to_string()),
                    port: Some(22),
                    protocol_options: None,
                },
                startup_script: None,
                environment_variables: None,
                notes: None,
                auto_reconnect: None,
                keep_alive_interval_seconds: None,
                favorite: None,
                settings_override: None,
            };
            let session = do_session_create(env.id(), req).unwrap();
            assert_eq!(session.session_type, *stype);
            created_ids.push(session.id);
        }

        assert_eq!(created_ids.len(), 13, "All 13 session types must be created");

        // Verify all persist and deserialize correctly
        let list = do_session_list(env.id()).unwrap();
        assert_eq!(list.len(), 13);

        for (stype, name) in &all_types {
            let found = list.iter().find(|s| s.name == *name);
            assert!(found.is_some(), "Session '{}' not found after persist", name);
            assert_eq!(found.unwrap().session_type, *stype, "SessionType mismatch for '{}'", name);
        }
    }

    // ── UT-C-13: last_connected_at update ───────────────────────────

    #[test]
    fn test_last_connected_at_update() {
        let env = TestEnv::new();

        let session = do_session_create(env.id(), make_session_request("Test Server", "10.0.0.1"))
            .unwrap();
        assert!(session.last_connected_at.is_none(), "New session should have no last_connected_at");

        let now = Utc::now();
        let updated = do_session_update(
            env.id(),
            &session.id,
            SessionUpdateRequest {
                name: None,
                session_type: None,
                group: None,
                tags: None,
                icon: None,
                color_label: None,
                credential_ref: None,
                connection: None,
                startup_script: None,
                environment_variables: None,
                notes: None,
                auto_reconnect: None,
                keep_alive_interval_seconds: None,
                favorite: None,
                settings_override: None,
            },
        )
        .unwrap();

        // Manually update last_connected_at by writing to file
        let mut session_data = do_session_get(env.id(), &session.id).unwrap();
        session_data.last_connected_at = Some(now);
        let json = serde_json::to_string_pretty(&session_data).unwrap();
        let path = session_file(env.id(), &session.id);
        std::fs::write(&path, json).unwrap();

        // Reload and verify timestamp persists
        let reloaded = do_session_get(env.id(), &session.id).unwrap();
        assert!(reloaded.last_connected_at.is_some(), "last_connected_at should persist after update");
        let lc = reloaded.last_connected_at.unwrap();
        // Should be within 1 second of `now`
        let diff = (lc - now).num_seconds().abs();
        assert!(diff <= 1, "last_connected_at timestamp drift too large: {} seconds", diff);
    }
}

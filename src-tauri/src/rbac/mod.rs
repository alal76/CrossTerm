use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // Vault permissions
    VaultRead,
    VaultWrite,
    VaultShare,
    VaultDelete,
    // Session permissions
    SessionConnect,
    SessionCreate,
    SessionDelete,
    // Admin permissions
    ManageUsers,
    ManageRoles,
    ViewAuditLog,
    ExportAuditLog,
    // Network
    NetworkScan,
    PortForward,
    // Recording
    ViewRecordings,
    ForceRecording,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,    // all permissions
    PowerUser, // connect, create sessions, port forward, view recordings
    ReadOnly, // connect only (no write, no admin)
    Auditor,  // view audit log + export only
    Custom(Vec<Permission>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: Role,
    pub public_key: Option<String>, // X25519 public key for vault sharing
    pub added_at: String,           // ISO 8601
    pub last_active: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamConfig {
    pub members: Vec<TeamMember>,
    pub require_mfa: bool,
    pub session_timeout_minutes: u32,
    pub allowed_ips: Vec<String>,
}

// ── State ────────────────────────────────────────────────────────────────────

pub struct RbacState {
    team_config: Arc<RwLock<Option<TeamConfig>>>,
}

impl RbacState {
    pub fn new() -> Self {
        Self {
            team_config: Arc::new(RwLock::new(None)),
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn config_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("crossterm").join("team_config.json"))
}

fn load_config_from_disk() -> Result<TeamConfig, String> {
    let path = config_path().ok_or_else(|| "Cannot determine data directory".to_string())?;
    if !path.exists() {
        return Ok(TeamConfig::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_config_to_disk(config: &TeamConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "Cannot determine data directory".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())
}

fn ensure_loaded(state: &RbacState) -> Result<(), String> {
    let needs_load = {
        let guard = state.team_config.read().map_err(|e| e.to_string())?;
        guard.is_none()
    };
    if needs_load {
        let config = load_config_from_disk()?;
        let mut guard = state.team_config.write().map_err(|e| e.to_string())?;
        *guard = Some(config);
    }
    Ok(())
}

// ── Business logic ────────────────────────────────────────────────────────────

/// Returns all permissions granted to a role.
pub fn role_permissions(role: &Role) -> Vec<Permission> {
    use Permission::*;
    match role {
        Role::Admin => vec![
            VaultRead, VaultWrite, VaultShare, VaultDelete,
            SessionConnect, SessionCreate, SessionDelete,
            ManageUsers, ManageRoles, ViewAuditLog, ExportAuditLog,
            NetworkScan, PortForward,
            ViewRecordings, ForceRecording,
        ],
        Role::PowerUser => vec![
            VaultRead,
            SessionConnect, SessionCreate,
            PortForward,
            ViewRecordings,
        ],
        Role::ReadOnly => vec![
            SessionConnect,
        ],
        Role::Auditor => vec![
            ViewAuditLog, ExportAuditLog,
        ],
        Role::Custom(perms) => perms.clone(),
    }
}

/// Returns true if the member holds the given permission.
pub fn has_permission(member: &TeamMember, perm: &Permission) -> bool {
    role_permissions(&member.role).contains(perm)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn rbac_list_members(state: tauri::State<RbacState>) -> Result<Vec<TeamMember>, String> {
    ensure_loaded(&state)?;
    let guard = state.team_config.read().map_err(|e| e.to_string())?;
    Ok(guard
        .as_ref()
        .map(|c| c.members.clone())
        .unwrap_or_default())
}

#[tauri::command]
pub fn rbac_add_member(
    state: tauri::State<RbacState>,
    display_name: String,
    email: Option<String>,
    role: Role,
) -> Result<TeamMember, String> {
    ensure_loaded(&state)?;
    let member = TeamMember {
        id: Uuid::new_v4().to_string(),
        display_name,
        email,
        role,
        public_key: None,
        added_at: chrono::Utc::now().to_rfc3339(),
        last_active: None,
    };
    {
        let mut guard = state.team_config.write().map_err(|e| e.to_string())?;
        let config = guard.get_or_insert_with(TeamConfig::default);
        config.members.push(member.clone());
        save_config_to_disk(config)?;
    }
    Ok(member)
}

#[tauri::command]
pub fn rbac_update_member_role(
    state: tauri::State<RbacState>,
    member_id: String,
    role: Role,
) -> Result<TeamMember, String> {
    ensure_loaded(&state)?;
    let mut guard = state.team_config.write().map_err(|e| e.to_string())?;
    let config = guard.as_mut().ok_or("Team config not loaded")?;
    let member = config
        .members
        .iter_mut()
        .find(|m| m.id == member_id)
        .ok_or_else(|| format!("Member not found: {member_id}"))?;
    member.role = role;
    let updated = member.clone();
    save_config_to_disk(config)?;
    Ok(updated)
}

#[tauri::command]
pub fn rbac_remove_member(
    state: tauri::State<RbacState>,
    member_id: String,
) -> Result<(), String> {
    ensure_loaded(&state)?;
    let mut guard = state.team_config.write().map_err(|e| e.to_string())?;
    let config = guard.as_mut().ok_or("Team config not loaded")?;
    let before = config.members.len();
    config.members.retain(|m| m.id != member_id);
    if config.members.len() == before {
        return Err(format!("Member not found: {member_id}"));
    }
    save_config_to_disk(config)?;
    Ok(())
}

#[tauri::command]
pub fn rbac_get_team_config(state: tauri::State<RbacState>) -> Result<TeamConfig, String> {
    ensure_loaded(&state)?;
    let guard = state.team_config.read().map_err(|e| e.to_string())?;
    Ok(guard.as_ref().cloned().unwrap_or_default())
}

#[tauri::command]
pub fn rbac_update_team_config(
    state: tauri::State<RbacState>,
    config: TeamConfig,
) -> Result<(), String> {
    save_config_to_disk(&config)?;
    let mut guard = state.team_config.write().map_err(|e| e.to_string())?;
    *guard = Some(config);
    Ok(())
}

#[tauri::command]
pub fn rbac_check_permission(
    state: tauri::State<RbacState>,
    member_id: String,
    permission: Permission,
) -> Result<bool, String> {
    ensure_loaded(&state)?;
    let guard = state.team_config.read().map_err(|e| e.to_string())?;
    let member = guard
        .as_ref()
        .and_then(|c| c.members.iter().find(|m| m.id == member_id))
        .ok_or_else(|| format!("Member not found: {member_id}"))?;
    Ok(has_permission(member, &permission))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_member(role: Role) -> TeamMember {
        TeamMember {
            id: "test-id".to_string(),
            display_name: "Test User".to_string(),
            email: None,
            role,
            public_key: None,
            added_at: "2026-01-01T00:00:00Z".to_string(),
            last_active: None,
        }
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let perms = role_permissions(&Role::Admin);
        assert!(perms.contains(&Permission::ManageUsers));
        assert!(perms.contains(&Permission::VaultDelete));
        assert!(perms.contains(&Permission::ExportAuditLog));
        assert!(perms.contains(&Permission::ForceRecording));
        assert!(perms.contains(&Permission::PortForward));
        assert!(perms.contains(&Permission::SessionDelete));
    }

    #[test]
    fn test_readonly_cannot_manage_users() {
        let perms = role_permissions(&Role::ReadOnly);
        assert!(!perms.contains(&Permission::ManageUsers));
        assert!(!perms.contains(&Permission::VaultWrite));
        assert!(!perms.contains(&Permission::SessionCreate));
        assert!(perms.contains(&Permission::SessionConnect));
    }

    #[test]
    fn test_auditor_can_export_audit_log() {
        let perms = role_permissions(&Role::Auditor);
        assert!(perms.contains(&Permission::ViewAuditLog));
        assert!(perms.contains(&Permission::ExportAuditLog));
        // Auditor should not have session or vault permissions
        assert!(!perms.contains(&Permission::SessionConnect));
        assert!(!perms.contains(&Permission::VaultRead));
    }

    #[test]
    fn test_custom_role_has_exact_permissions() {
        let custom_perms = vec![Permission::VaultRead, Permission::SessionConnect];
        let perms = role_permissions(&Role::Custom(custom_perms.clone()));
        assert_eq!(perms, custom_perms);
        assert!(!perms.contains(&Permission::VaultWrite));
        assert!(!perms.contains(&Permission::ManageUsers));
    }

    #[test]
    fn test_has_permission_checks_role_correctly() {
        let admin = make_member(Role::Admin);
        let readonly = make_member(Role::ReadOnly);
        let auditor = make_member(Role::Auditor);

        assert!(has_permission(&admin, &Permission::ManageUsers));
        assert!(!has_permission(&readonly, &Permission::ManageUsers));
        assert!(!has_permission(&auditor, &Permission::SessionConnect));
        assert!(has_permission(&auditor, &Permission::ExportAuditLog));
        assert!(has_permission(&readonly, &Permission::SessionConnect));
    }

    #[test]
    fn test_team_config_default_is_empty() {
        let config = TeamConfig::default();
        assert!(config.members.is_empty());
        assert!(!config.require_mfa);
        assert_eq!(config.session_timeout_minutes, 0);
        assert!(config.allowed_ips.is_empty());
    }

    #[test]
    fn test_power_user_permissions() {
        let perms = role_permissions(&Role::PowerUser);
        assert!(perms.contains(&Permission::SessionConnect));
        assert!(perms.contains(&Permission::SessionCreate));
        assert!(perms.contains(&Permission::PortForward));
        assert!(perms.contains(&Permission::ViewRecordings));
        assert!(!perms.contains(&Permission::ManageUsers));
        assert!(!perms.contains(&Permission::VaultWrite));
    }

    #[test]
    fn test_custom_member_permission_check() {
        let member = make_member(Role::Custom(vec![
            Permission::NetworkScan,
            Permission::VaultRead,
        ]));
        assert!(has_permission(&member, &Permission::NetworkScan));
        assert!(has_permission(&member, &Permission::VaultRead));
        assert!(!has_permission(&member, &Permission::VaultWrite));
        assert!(!has_permission(&member, &Permission::ManageUsers));
    }

    #[test]
    fn test_permission_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&Permission::ManageUsers).unwrap(),
            "\"manage_users\""
        );
        assert_eq!(
            serde_json::to_string(&Permission::ViewAuditLog).unwrap(),
            "\"view_audit_log\""
        );
        let de: Permission = serde_json::from_str("\"vault_share\"").unwrap();
        assert_eq!(de, Permission::VaultShare);
    }

    #[test]
    fn test_permission_hash_eq_dedup() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Permission::VaultRead);
        set.insert(Permission::VaultRead);
        set.insert(Permission::VaultWrite);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_role_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&Role::PowerUser).unwrap(),
            "\"power_user\""
        );
        assert_eq!(
            serde_json::to_string(&Role::ReadOnly).unwrap(),
            "\"read_only\""
        );

        let custom = Role::Custom(vec![Permission::SessionConnect]);
        let json = serde_json::to_string(&custom).unwrap();
        let de: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(de, custom);
    }

    #[test]
    fn test_team_member_serde_roundtrip() {
        let member = TeamMember {
            id: "id-1".to_string(),
            display_name: "Alice".to_string(),
            email: Some("alice@example.com".to_string()),
            role: Role::Admin,
            public_key: Some("pubkey-bytes".to_string()),
            added_at: "2026-01-01T00:00:00Z".to_string(),
            last_active: Some("2026-01-02T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&member).unwrap();
        let de: TeamMember = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, member.id);
        assert_eq!(de.display_name, member.display_name);
        assert_eq!(de.email, member.email);
        assert_eq!(de.role, member.role);
        assert_eq!(de.public_key, member.public_key);
    }

    #[test]
    fn test_team_config_serde_roundtrip_with_members() {
        let config = TeamConfig {
            members: vec![make_member(Role::PowerUser)],
            require_mfa: true,
            session_timeout_minutes: 30,
            allowed_ips: vec!["10.0.0.0/8".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let de: TeamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(de.members.len(), 1);
        assert!(de.require_mfa);
        assert_eq!(de.session_timeout_minutes, 30);
        assert_eq!(de.allowed_ips, vec!["10.0.0.0/8".to_string()]);
    }

    #[test]
    fn test_rbac_state_new_starts_unloaded() {
        let state = RbacState::new();
        // Access the private field directly (test module is a descendant of
        // the defining module, so this is allowed) to confirm the state
        // starts empty and hasn't eagerly touched disk.
        let guard = state.team_config.read().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn test_config_path_shape() {
        // Doesn't touch disk — just verifies the constructed path, so the
        // real user data directory is never written to by this test.
        let path = config_path().expect("data dir should resolve on any supported OS");
        assert!(path.to_string_lossy().contains("crossterm"));
        assert_eq!(path.file_name().unwrap(), "team_config.json");
    }
}

// ── LDAP / AD group sync ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapConfig {
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password_vault_ref: String,
    pub base_dn: String,
    pub group_filter: String,
    pub user_attr: String,
    pub group_attr: String,
    pub sync_interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LdapGroupMapping {
    pub ldap_group_dn: String,
    pub crossterm_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapSyncResult {
    pub synced_at: String,
    pub users_added: usize,
    pub users_updated: usize,
    pub users_removed: usize,
    pub errors: Vec<String>,
}

static LDAP_CONFIG: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Option<LdapConfig>>>> =
    std::sync::OnceLock::new();
fn get_ldap_config() -> std::sync::Arc<std::sync::Mutex<Option<LdapConfig>>> {
    LDAP_CONFIG
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
        .clone()
}

#[tauri::command]
pub fn rbac_ldap_configure(config: LdapConfig) -> Result<(), String> {
    let store = get_ldap_config();
    let mut guard = store.lock().map_err(|e| e.to_string())?;
    *guard = Some(config);
    Ok(())
}

#[tauri::command]
pub fn rbac_ldap_test_connection() -> Result<String, String> {
    Ok("ldap_connection_test_requires_live_server".to_string())
}

#[tauri::command]
pub fn rbac_ldap_sync() -> Result<LdapSyncResult, String> {
    Ok(LdapSyncResult {
        synced_at: "2026-01-01T00:00:00Z".to_string(),
        users_added: 0,
        users_updated: 0,
        users_removed: 0,
        errors: vec!["LDAP sync requires live AD/LDAP server".to_string()],
    })
}

#[cfg(test)]
mod ldap_tests {
    use super::*;

    #[test]
    fn test_ldap_configure() {
        let cfg = LdapConfig {
            server_url: "ldaps://ad.example.com:636".to_string(),
            bind_dn: "CN=svc,DC=corp,DC=com".to_string(),
            bind_password_vault_ref: "vault-cred-1".to_string(),
            base_dn: "DC=corp,DC=com".to_string(),
            group_filter: "(objectClass=group)".to_string(),
            user_attr: "sAMAccountName".to_string(),
            group_attr: "cn".to_string(),
            sync_interval_minutes: 60,
        };
        rbac_ldap_configure(cfg).unwrap();
        let store = get_ldap_config();
        let guard = store.lock().unwrap();
        assert_eq!(
            guard.as_ref().unwrap().server_url,
            "ldaps://ad.example.com:636"
        );
    }

    #[test]
    fn test_ldap_sync_returns_stub_error() {
        let result = rbac_ldap_sync().unwrap();
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("requires live"));
    }

    #[test]
    fn test_ldap_test_connection_stub() {
        let result = rbac_ldap_test_connection().unwrap();
        assert_eq!(result, "ldap_connection_test_requires_live_server");
    }

    #[test]
    fn test_ldap_group_mapping_serde_roundtrip() {
        let mapping = LdapGroupMapping {
            ldap_group_dn: "CN=Admins,DC=corp,DC=com".to_string(),
            crossterm_role: "admin".to_string(),
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let de: LdapGroupMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(de.ldap_group_dn, mapping.ldap_group_dn);
        assert_eq!(de.crossterm_role, mapping.crossterm_role);
    }

    #[test]
    fn test_ldap_sync_result_serde_roundtrip() {
        let result = LdapSyncResult {
            synced_at: "2026-01-01T00:00:00Z".to_string(),
            users_added: 3,
            users_updated: 1,
            users_removed: 0,
            errors: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let de: LdapSyncResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de.users_added, 3);
        assert_eq!(de.users_updated, 1);
        assert!(de.errors.is_empty());
    }
}

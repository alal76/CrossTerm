use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Types ─────────────────────────────────────────────────────────────────────

/// A portable `.ctbundle` export containing sessions, group metadata, and a
/// SHA-256 integrity checksum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtBundle {
    pub version: u32,
    pub created_at: String,
    /// Array of Session JSON objects.
    pub sessions: Vec<serde_json::Value>,
    /// Array of group metadata JSON objects.
    pub groups: Vec<serde_json::Value>,
    /// Hex-encoded SHA-256 of `sessions_json + groups_json`.
    pub checksum: String,
}

// ── Core logic ────────────────────────────────────────────────────────────────

fn compute_checksum(
    sessions: &Vec<serde_json::Value>,
    groups: &Vec<serde_json::Value>,
) -> Result<String, String> {
    let sessions_json =
        serde_json::to_string(sessions).map_err(|e| format!("serialize sessions: {e}"))?;
    let groups_json =
        serde_json::to_string(groups).map_err(|e| format!("serialize groups: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(sessions_json.as_bytes());
    hasher.update(groups_json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Build a new [`CtBundle`] from raw session and group data.
///
/// The `checksum` field is automatically computed as the hex-encoded SHA-256 of
/// `serde_json::to_string(&sessions) + serde_json::to_string(&groups)`.
pub fn create_bundle(
    sessions: Vec<serde_json::Value>,
    groups: Vec<serde_json::Value>,
) -> Result<CtBundle, String> {
    let checksum = compute_checksum(&sessions, &groups)?;
    Ok(CtBundle {
        version: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        sessions,
        groups,
        checksum,
    })
}

/// Serialize a [`CtBundle`] to a pretty-printed JSON string.
pub fn serialize_bundle(bundle: &CtBundle) -> Result<String, String> {
    serde_json::to_string_pretty(bundle).map_err(|e| format!("serialize bundle: {e}"))
}

/// Deserialize a [`CtBundle`] from a JSON string.
pub fn deserialize_bundle(json: &str) -> Result<CtBundle, String> {
    serde_json::from_str(json).map_err(|e| format!("deserialize bundle: {e}"))
}

/// Returns `true` if the bundle's stored checksum matches a freshly-computed
/// checksum over its `sessions` and `groups` fields.
pub fn verify_bundle_checksum(bundle: &CtBundle) -> bool {
    match compute_checksum(&bundle.sessions, &bundle.groups) {
        Ok(expected) => expected == bundle.checksum,
        Err(_) => false,
    }
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

/// Export sessions and groups as a serialized `.ctbundle` JSON string.
#[tauri::command]
pub fn session_bundle_export(
    sessions: Vec<serde_json::Value>,
    groups: Vec<serde_json::Value>,
) -> Result<String, String> {
    let bundle = create_bundle(sessions, groups)?;
    serialize_bundle(&bundle)
}

/// Import a `.ctbundle` JSON string, verify its checksum, and return the bundle.
///
/// Returns an error if the checksum does not match (i.e. the bundle was tampered
/// with or is corrupt).
#[tauri::command]
pub fn session_bundle_import(bundle_json: String) -> Result<CtBundle, String> {
    let bundle = deserialize_bundle(&bundle_json)?;
    if !verify_bundle_checksum(&bundle) {
        return Err("bundle checksum mismatch: file may be corrupt or tampered".to_string());
    }
    Ok(bundle)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sessions() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({"name": "prod", "host": "prod.example.com", "port": 22}),
            serde_json::json!({"name": "dev",  "host": "dev.example.com",  "port": 2222}),
        ]
    }

    fn sample_groups() -> Vec<serde_json::Value> {
        vec![serde_json::json!({"id": "grp-1", "label": "Production"})]
    }

    #[test]
    fn test_bundle_create_and_verify() {
        let bundle = create_bundle(sample_sessions(), sample_groups())
            .expect("create_bundle must succeed");
        assert!(
            verify_bundle_checksum(&bundle),
            "freshly created bundle must pass checksum verification"
        );
        assert!(!bundle.checksum.is_empty(), "checksum must not be empty");
        assert_eq!(bundle.version, 1);
    }

    #[test]
    fn test_bundle_checksum_tamper() {
        let mut bundle = create_bundle(sample_sessions(), sample_groups())
            .expect("create_bundle must succeed");
        // Tamper: inject an extra session after the fact
        bundle
            .sessions
            .push(serde_json::json!({"name": "evil", "host": "evil.example.com"}));
        assert!(
            !verify_bundle_checksum(&bundle),
            "tampered bundle must fail checksum verification"
        );
    }

    #[test]
    fn test_bundle_round_trip() {
        let sessions = sample_sessions();
        let session_count = sessions.len();

        let bundle = create_bundle(sessions, sample_groups())
            .expect("create_bundle must succeed");
        let json = serialize_bundle(&bundle).expect("serialize must succeed");
        let restored = deserialize_bundle(&json).expect("deserialize must succeed");

        assert_eq!(
            restored.sessions.len(),
            session_count,
            "session count must survive round-trip"
        );
        assert!(
            verify_bundle_checksum(&restored),
            "round-tripped bundle must pass checksum verification"
        );
    }

    #[test]
    fn test_bundle_empty() {
        let bundle =
            create_bundle(vec![], vec![]).expect("create_bundle with empty inputs must succeed");
        assert!(
            verify_bundle_checksum(&bundle),
            "empty bundle must pass checksum verification"
        );
        assert!(
            !bundle.checksum.is_empty(),
            "empty bundle must still have a non-empty checksum"
        );
        assert_eq!(bundle.sessions.len(), 0);
        assert_eq!(bundle.groups.len(), 0);
    }

    #[test]
    fn test_deserialize_bundle_invalid_json_errors() {
        let result = deserialize_bundle("{ not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("deserialize bundle"));
    }

    #[test]
    fn test_deserialize_bundle_missing_fields_errors() {
        // Well-formed JSON, but missing required fields (sessions/groups/checksum).
        let result = deserialize_bundle(r#"{"version": 1}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_bundle_import_rejects_tampered_checksum() {
        let bundle = create_bundle(sample_sessions(), sample_groups()).unwrap();
        let json = serialize_bundle(&bundle).unwrap();
        // Corrupt the JSON by editing a session's host value directly, leaving
        // the (now stale) checksum field untouched — simulates on-disk tampering.
        let corrupted = json.replace("prod.example.com", "evil.example.com");
        let result = session_bundle_import(corrupted);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("checksum mismatch"));
    }

    #[test]
    fn test_session_bundle_import_accepts_valid_bundle() {
        let exported = session_bundle_export(sample_sessions(), sample_groups()).unwrap();
        let imported = session_bundle_import(exported).expect("valid bundle should import");
        assert_eq!(imported.sessions.len(), 2);
        assert_eq!(imported.groups.len(), 1);
    }

    #[test]
    fn test_session_bundle_import_invalid_json_errors() {
        let result = session_bundle_import("not json at all".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_bundle_checksum_detects_group_tamper() {
        let mut bundle = create_bundle(sample_sessions(), sample_groups()).unwrap();
        bundle.groups.push(serde_json::json!({"id": "grp-evil"}));
        assert!(!verify_bundle_checksum(&bundle));
    }

    #[test]
    fn test_compute_checksum_deterministic() {
        // Same inputs must always hash to the same checksum.
        let a = create_bundle(sample_sessions(), sample_groups()).unwrap();
        let b = create_bundle(sample_sessions(), sample_groups()).unwrap();
        assert_eq!(a.checksum, b.checksum, "checksum must be deterministic for identical content");
    }
}

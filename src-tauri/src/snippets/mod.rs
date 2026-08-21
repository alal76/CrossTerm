use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum SnippetError {
    #[error("Snippet not found: {0}")]
    NotFound(String),
    #[error("Snippet already exists: {0}")]
    AlreadyExists(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Serialize for SnippetError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub command: String,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn default_snippets_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    base.join("crossterm").join("snippets.json")
}

fn load_snippets(path: &PathBuf) -> HashMap<String, Snippet> {
    match std::fs::read_to_string(path) {
        Ok(data) => {
            let list: Vec<Snippet> = serde_json::from_str(&data).unwrap_or_default();
            list.into_iter().map(|s| (s.id.clone(), s)).collect()
        }
        Err(_) => HashMap::new(),
    }
}

fn save_snippets(path: &PathBuf, snippets: &HashMap<String, Snippet>) -> Result<(), SnippetError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let list: Vec<&Snippet> = snippets.values().collect();
    let data = serde_json::to_string_pretty(&list)?;
    std::fs::write(path, data)?;
    Ok(())
}

// ── State ───────────────────────────────────────────────────────────────

pub struct SnippetState {
    snippets: Mutex<HashMap<String, Snippet>>,
    path: PathBuf,
}

impl SnippetState {
    pub fn new() -> Self {
        let path = default_snippets_path();
        let snippets = load_snippets(&path);
        Self {
            snippets: Mutex::new(snippets),
            path,
        }
    }

    #[cfg(test)]
    pub fn new_with_path(path: PathBuf) -> Self {
        let snippets = load_snippets(&path);
        Self {
            snippets: Mutex::new(snippets),
            path,
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn snippet_create(
    name: String,
    command: String,
    tags: Vec<String>,
    state: tauri::State<'_, SnippetState>,
) -> Result<Snippet, SnippetError> {
    let now = chrono::Utc::now().to_rfc3339();
    let snippet = Snippet {
        id: Uuid::new_v4().to_string(),
        name,
        command,
        tags,
        created_at: now.clone(),
        updated_at: now,
    };
    let mut map = state.snippets.lock().unwrap();
    map.insert(snippet.id.clone(), snippet.clone());
    save_snippets(&state.path, &map)?;
    Ok(snippet)
}

#[tauri::command]
pub fn snippet_list(
    state: tauri::State<'_, SnippetState>,
) -> Result<Vec<Snippet>, SnippetError> {
    let map = state.snippets.lock().unwrap();
    let mut list: Vec<Snippet> = map.values().cloned().collect();
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(list)
}

#[tauri::command]
pub fn snippet_get(
    id: String,
    state: tauri::State<'_, SnippetState>,
) -> Result<Snippet, SnippetError> {
    let map = state.snippets.lock().unwrap();
    map.get(&id).cloned().ok_or(SnippetError::NotFound(id))
}

#[tauri::command]
pub fn snippet_update(
    id: String,
    name: Option<String>,
    command: Option<String>,
    tags: Option<Vec<String>>,
    state: tauri::State<'_, SnippetState>,
) -> Result<Snippet, SnippetError> {
    let mut map = state.snippets.lock().unwrap();
    let snippet = map.get_mut(&id).ok_or_else(|| SnippetError::NotFound(id.clone()))?;
    if let Some(n) = name {
        snippet.name = n;
    }
    if let Some(c) = command {
        snippet.command = c;
    }
    if let Some(t) = tags {
        snippet.tags = t;
    }
    snippet.updated_at = chrono::Utc::now().to_rfc3339();
    let updated = snippet.clone();
    save_snippets(&state.path, &map)?;
    Ok(updated)
}

#[tauri::command]
pub fn snippet_delete(
    id: String,
    state: tauri::State<'_, SnippetState>,
) -> Result<(), SnippetError> {
    let mut map = state.snippets.lock().unwrap();
    if map.remove(&id).is_none() {
        return Err(SnippetError::NotFound(id));
    }
    save_snippets(&state.path, &map)?;
    Ok(())
}

#[tauri::command]
pub fn snippet_search(
    query: String,
    state: tauri::State<'_, SnippetState>,
) -> Result<Vec<Snippet>, SnippetError> {
    let map = state.snippets.lock().unwrap();
    let q = query.to_lowercase();
    let mut results: Vec<Snippet> = map
        .values()
        .filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.command.to_lowercase().contains(&q)
                || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .cloned()
        .collect();
    results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(results)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_state() -> SnippetState {
        let tmp = NamedTempFile::new().unwrap();
        SnippetState::new_with_path(tmp.path().to_path_buf())
    }

    fn create_snippet(state: &SnippetState, name: &str, command: &str, tags: Vec<String>) -> Snippet {
        let mut map = state.snippets.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let snippet = Snippet {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            command: command.to_string(),
            tags,
            created_at: now.clone(),
            updated_at: now,
        };
        map.insert(snippet.id.clone(), snippet.clone());
        save_snippets(&state.path, &map).unwrap();
        snippet
    }

    #[test]
    fn test_snippet_crud() {
        let state = test_state();

        // Create
        let snippet = create_snippet(&state, "List files", "ls -la", vec!["linux".into(), "files".into()]);
        assert_eq!(snippet.name, "List files");
        assert_eq!(snippet.command, "ls -la");
        assert_eq!(snippet.tags.len(), 2);

        // Get
        let map = state.snippets.lock().unwrap();
        let fetched = map.get(&snippet.id).unwrap();
        assert_eq!(fetched.name, "List files");
        drop(map);

        // Update
        {
            let mut map = state.snippets.lock().unwrap();
            let s = map.get_mut(&snippet.id).unwrap();
            s.name = "List all files".to_string();
            s.updated_at = chrono::Utc::now().to_rfc3339();
            save_snippets(&state.path, &map).unwrap();
        }
        let map = state.snippets.lock().unwrap();
        assert_eq!(map.get(&snippet.id).unwrap().name, "List all files");
        drop(map);

        // Delete
        {
            let mut map = state.snippets.lock().unwrap();
            map.remove(&snippet.id);
            save_snippets(&state.path, &map).unwrap();
        }
        let map = state.snippets.lock().unwrap();
        assert!(map.get(&snippet.id).is_none());
    }

    #[test]
    fn test_snippet_search() {
        let state = test_state();

        create_snippet(&state, "Docker logs", "docker logs -f {{container}}", vec!["docker".into()]);
        create_snippet(&state, "K8s pods", "kubectl get pods", vec!["k8s".into()]);
        create_snippet(&state, "Docker ps", "docker ps -a", vec!["docker".into()]);

        let map = state.snippets.lock().unwrap();
        let q = "docker".to_lowercase();
        let results: Vec<&Snippet> = map
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q)
                    || s.command.to_lowercase().contains(&q)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect();
        assert_eq!(results.len(), 2);

        let q2 = "kubectl".to_lowercase();
        let results2: Vec<&Snippet> = map
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q2)
                    || s.command.to_lowercase().contains(&q2)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q2))
            })
            .collect();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_snippet_persistence() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Create state, add a snippet, drop it
        {
            let state = SnippetState::new_with_path(path.clone());
            create_snippet(&state, "Persist test", "echo hello", vec!["test".into()]);
        }

        // Reload from disk
        let state2 = SnippetState::new_with_path(path);
        let map = state2.snippets.lock().unwrap();
        assert_eq!(map.len(), 1);
        let snippet = map.values().next().unwrap();
        assert_eq!(snippet.name, "Persist test");
        assert_eq!(snippet.command, "echo hello");
    }

    // ── default_snippets_path ──────────────────────────────────────────

    #[test]
    fn test_default_snippets_path_shape() {
        let path = default_snippets_path();
        assert!(path.ends_with("crossterm/snippets.json"), "got: {path:?}");
    }

    // ── load_snippets edge cases ───────────────────────────────────────

    #[test]
    fn test_load_snippets_nonexistent_path_returns_empty() {
        let path = PathBuf::from("/nonexistent/path/snippets.json");
        let map = load_snippets(&path);
        assert!(map.is_empty());
    }

    #[test]
    fn test_load_snippets_malformed_json_returns_empty() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "not valid json").unwrap();
        let map = load_snippets(&tmp.path().to_path_buf());
        assert!(map.is_empty(), "malformed JSON must fall back to an empty map, not panic");
    }

    #[test]
    fn test_load_snippets_valid_array_keyed_by_id() {
        let tmp = NamedTempFile::new().unwrap();
        let json = r#"[{"id":"s1","name":"Test","command":"ls","tags":[],"createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z"}]"#;
        std::fs::write(tmp.path(), json).unwrap();
        let map = load_snippets(&tmp.path().to_path_buf());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("s1").unwrap().name, "Test");
    }

    // ── save_snippets ─────────────────────────────────────────────────

    #[test]
    fn test_save_snippets_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested_path = dir.path().join("nested").join("subdir").join("snippets.json");
        let mut map = HashMap::new();
        let snippet = Snippet {
            id: "abc".into(),
            name: "n".into(),
            command: "echo hi".into(),
            tags: vec![],
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        map.insert(snippet.id.clone(), snippet);
        save_snippets(&nested_path, &map).expect("should create parent dirs and save");
        assert!(nested_path.exists());

        let reloaded = load_snippets(&nested_path);
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded.get("abc").unwrap().command, "echo hi");
    }

    #[test]
    fn test_save_snippets_empty_map_writes_empty_array() {
        let tmp = NamedTempFile::new().unwrap();
        let map: HashMap<String, Snippet> = HashMap::new();
        save_snippets(&tmp.path().to_path_buf(), &map).unwrap();
        let contents = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: Vec<Snippet> = serde_json::from_str(&contents).unwrap();
        assert!(parsed.is_empty());
    }

    // ── SnippetError Display ────────────────────────────────────────────

    #[test]
    fn test_snippet_error_display_messages() {
        assert_eq!(
            SnippetError::NotFound("id-1".into()).to_string(),
            "Snippet not found: id-1"
        );
        assert_eq!(
            SnippetError::AlreadyExists("id-2".into()).to_string(),
            "Snippet already exists: id-2"
        );
    }

    // ── Search: case-insensitivity and tag matching ─────────────────────

    #[test]
    fn test_snippet_search_matches_by_tag_case_insensitive() {
        let state = test_state();
        create_snippet(&state, "Restart nginx", "systemctl restart nginx", vec!["Linux".into(), "Ops".into()]);

        let map = state.snippets.lock().unwrap();
        let q = "linux".to_lowercase();
        let results: Vec<&Snippet> = map
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q)
                    || s.command.to_lowercase().contains(&q)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect();
        assert_eq!(results.len(), 1, "tag match must be case-insensitive");
    }

    #[test]
    fn test_snippet_search_no_match_returns_empty() {
        let state = test_state();
        create_snippet(&state, "List files", "ls -la", vec![]);

        let map = state.snippets.lock().unwrap();
        let q = "nonexistent-query".to_lowercase();
        let results: Vec<&Snippet> = map
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&q)
                    || s.command.to_lowercase().contains(&q)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect();
        assert!(results.is_empty());
    }

    // ── Update: partial fields only touch what's provided ────────────────

    #[test]
    fn test_snippet_partial_update_preserves_untouched_fields() {
        let state = test_state();
        let snippet = create_snippet(&state, "Original", "echo original", vec!["a".into()]);

        {
            let mut map = state.snippets.lock().unwrap();
            let s = map.get_mut(&snippet.id).unwrap();
            // Only update the command, leave name/tags untouched.
            s.command = "echo updated".to_string();
            save_snippets(&state.path, &map).unwrap();
        }

        let map = state.snippets.lock().unwrap();
        let updated = map.get(&snippet.id).unwrap();
        assert_eq!(updated.name, "Original", "name must be unchanged");
        assert_eq!(updated.command, "echo updated");
        assert_eq!(updated.tags, vec!["a".to_string()], "tags must be unchanged");
    }

    // ── Delete: nonexistent id is a no-op at the map level ────────────────

    #[test]
    fn test_snippet_delete_nonexistent_id_returns_none() {
        let state = test_state();
        let mut map = state.snippets.lock().unwrap();
        let removed = map.remove("does-not-exist");
        assert!(removed.is_none());
    }
}

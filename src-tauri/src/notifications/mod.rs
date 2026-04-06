use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Notification not found: {0}")]
    NotFound(String),
}

impl Serialize for NotificationError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NotificationEntry {
    pub id: String,
    pub timestamp: String,
    pub severity: String,
    pub message: String,
    pub session_id: Option<String>,
    pub category: String,
    pub dismissed: bool,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct NotificationState {
    entries: Mutex<Vec<NotificationEntry>>,
}

impl NotificationState {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn notification_list(
    state: tauri::State<'_, NotificationState>,
) -> Result<Vec<NotificationEntry>, NotificationError> {
    let entries = state.entries.lock().unwrap();
    let mut result = entries.clone();
    result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(result)
}

#[tauri::command]
pub fn notification_dismiss(
    id: String,
    state: tauri::State<'_, NotificationState>,
) -> Result<(), NotificationError> {
    let mut entries = state.entries.lock().unwrap();
    let entry = entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| NotificationError::NotFound(id))?;
    entry.dismissed = true;
    Ok(())
}

#[tauri::command]
pub fn notification_clear_all(
    state: tauri::State<'_, NotificationState>,
) -> Result<(), NotificationError> {
    let mut entries = state.entries.lock().unwrap();
    entries.clear();
    Ok(())
}

#[tauri::command]
pub fn notification_add(
    severity: String,
    message: String,
    session_id: Option<String>,
    category: String,
    state: tauri::State<'_, NotificationState>,
) -> Result<NotificationEntry, NotificationError> {
    let entry = NotificationEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        severity,
        message,
        session_id,
        category,
        dismissed: false,
    };
    let mut entries = state.entries.lock().unwrap();
    entries.push(entry.clone());
    Ok(entry)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> NotificationState {
        NotificationState::new()
    }

    #[test]
    fn test_notification_add_and_list() {
        let state = make_state();

        // Add two notifications
        {
            let mut entries = state.entries.lock().unwrap();
            entries.push(NotificationEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                severity: "info".to_string(),
                message: "First notification".to_string(),
                session_id: None,
                category: "system".to_string(),
                dismissed: false,
            });
            entries.push(NotificationEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: "2026-01-02T00:00:00Z".to_string(),
                severity: "error".to_string(),
                message: "Second notification".to_string(),
                session_id: Some("sess-1".to_string()),
                category: "connection".to_string(),
                dismissed: false,
            });
        }

        let entries = state.entries.lock().unwrap();
        assert_eq!(entries.len(), 2);

        // Newest should sort first when listed
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        assert_eq!(sorted[0].message, "Second notification");
        assert_eq!(sorted[1].message, "First notification");
    }

    #[test]
    fn test_notification_dismiss_and_clear() {
        let state = make_state();

        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();

        {
            let mut entries = state.entries.lock().unwrap();
            entries.push(NotificationEntry {
                id: id1.clone(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                severity: "warn".to_string(),
                message: "Warning one".to_string(),
                session_id: None,
                category: "system".to_string(),
                dismissed: false,
            });
            entries.push(NotificationEntry {
                id: id2.clone(),
                timestamp: "2026-01-02T00:00:00Z".to_string(),
                severity: "info".to_string(),
                message: "Info two".to_string(),
                session_id: None,
                category: "system".to_string(),
                dismissed: false,
            });
        }

        // Dismiss first entry
        {
            let mut entries = state.entries.lock().unwrap();
            let entry = entries.iter_mut().find(|e| e.id == id1).unwrap();
            entry.dismissed = true;
        }

        {
            let entries = state.entries.lock().unwrap();
            let dismissed = entries.iter().find(|e| e.id == id1).unwrap();
            assert!(dismissed.dismissed);
            let not_dismissed = entries.iter().find(|e| e.id == id2).unwrap();
            assert!(!not_dismissed.dismissed);
        }

        // Clear all
        {
            let mut entries = state.entries.lock().unwrap();
            entries.clear();
        }

        let entries = state.entries.lock().unwrap();
        assert!(entries.is_empty());
    }
}

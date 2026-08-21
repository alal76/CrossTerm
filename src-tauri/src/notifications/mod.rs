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
    do_notification_list(state.inner())
}

fn do_notification_list(state: &NotificationState) -> Result<Vec<NotificationEntry>, NotificationError> {
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
    do_notification_dismiss(id, state.inner())
}

fn do_notification_dismiss(id: String, state: &NotificationState) -> Result<(), NotificationError> {
    let mut entries = state.entries.lock().unwrap();
    let entry = entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or(NotificationError::NotFound(id))?;
    entry.dismissed = true;
    Ok(())
}

#[tauri::command]
pub fn notification_clear_all(
    state: tauri::State<'_, NotificationState>,
) -> Result<(), NotificationError> {
    do_notification_clear_all(state.inner())
}

fn do_notification_clear_all(state: &NotificationState) -> Result<(), NotificationError> {
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
    do_notification_add(severity, message, session_id, category, state.inner())
}

fn do_notification_add(
    severity: String,
    message: String,
    session_id: Option<String>,
    category: String,
    state: &NotificationState,
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

    // ── Tests exercising the real `do_notification_*` functions ──────────

    #[test]
    fn test_do_notification_add_and_list_sorted_newest_first() {
        let state = make_state();
        let first = do_notification_add(
            "info".into(),
            "First".into(),
            None,
            "system".into(),
            &state,
        )
        .expect("add ok");
        assert!(!first.id.is_empty());
        assert!(!first.dismissed);

        // Force a distinguishable, later timestamp for the second entry
        // rather than relying on real clock granularity.
        {
            let mut entries = state.entries.lock().unwrap();
            entries[0].timestamp = "2020-01-01T00:00:00Z".into();
        }
        let second = do_notification_add(
            "error".into(),
            "Second".into(),
            Some("sess-1".into()),
            "connection".into(),
            &state,
        )
        .expect("add ok");
        {
            let mut entries = state.entries.lock().unwrap();
            let e = entries.iter_mut().find(|e| e.id == second.id).unwrap();
            e.timestamp = "2020-01-02T00:00:00Z".into();
        }

        let listed = do_notification_list(&state).expect("list ok");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].message, "Second");
        assert_eq!(listed[1].message, "First");
    }

    #[test]
    fn test_do_notification_dismiss_success_and_not_found() {
        let state = make_state();
        let entry = do_notification_add(
            "warn".into(),
            "Warn".into(),
            None,
            "system".into(),
            &state,
        )
        .unwrap();

        do_notification_dismiss(entry.id.clone(), &state).expect("dismiss ok");
        let listed = do_notification_list(&state).unwrap();
        assert!(listed[0].dismissed);

        let err = do_notification_dismiss("nonexistent".into(), &state).unwrap_err();
        assert!(matches!(err, NotificationError::NotFound(_)));
    }

    #[test]
    fn test_do_notification_clear_all() {
        let state = make_state();
        do_notification_add("info".into(), "a".into(), None, "system".into(), &state).unwrap();
        do_notification_add("info".into(), "b".into(), None, "system".into(), &state).unwrap();
        assert_eq!(do_notification_list(&state).unwrap().len(), 2);

        do_notification_clear_all(&state).expect("clear ok");
        assert!(do_notification_list(&state).unwrap().is_empty());
    }

    #[test]
    fn test_notification_error_display_and_serialize() {
        let err = NotificationError::NotFound("abc".into());
        assert_eq!(err.to_string(), "Notification not found: abc");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Notification not found: abc\"");

        let io_err: NotificationError =
            std::io::Error::new(std::io::ErrorKind::Other, "disk full").into();
        assert!(matches!(io_err, NotificationError::Io(_)));
    }

    #[test]
    fn test_notification_entry_serde_roundtrip() {
        let entry = NotificationEntry {
            id: "n1".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            severity: "info".into(),
            message: "hello".into(),
            session_id: Some("s1".into()),
            category: "system".into(),
            dismissed: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: NotificationEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "n1");
        assert_eq!(parsed.session_id.as_deref(), Some("s1"));
        assert!(!parsed.dismissed);
    }
}

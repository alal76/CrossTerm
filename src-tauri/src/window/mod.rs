use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Error ──

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum WindowError {
    #[error("Window not found: {0}")]
    NotFound(String),
    #[error("Tauri error: {0}")]
    Tauri(String),
}

impl Serialize for WindowError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

// ── Types ──

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedTab {
    pub window_label: String,
    pub tab_id: String,
    pub session_id: String,
}

// ── Helpers ──

/// Builds the window label used for a detached tab window. Extracted so it's
/// unit-testable without a full `tauri::AppHandle`.
fn tab_window_label(tab_id: &str) -> String {
    format!("tab-{}", tab_id)
}

// ── Commands ──

#[tauri::command]
pub async fn window_create_for_tab(
    app: tauri::AppHandle,
    tab_id: String,
    _session_id: String,
    title: String,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<String, WindowError> {
    use tauri::WebviewWindowBuilder;

    let label = tab_window_label(&tab_id);
    let mut builder = WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(&title)
    .inner_size(900.0, 600.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true);

    // Mobile has no windowing concept of decorations (title bar/borders) -
    // WebviewWindowBuilder doesn't expose this method there at all.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.decorations(true);
    }

    if let (Some(x), Some(y)) = (x, y) {
        builder = builder.position(x, y);
    }

    builder
        .build()
        .map_err(|e| WindowError::Tauri(e.to_string()))?;

    Ok(label)
}

#[tauri::command]
pub async fn window_close(
    app: tauri::AppHandle,
    window_label: String,
) -> Result<(), WindowError> {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window(&window_label) {
        window
            .close()
            .map_err(|e| WindowError::Tauri(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn window_list(app: tauri::AppHandle) -> Result<Vec<String>, WindowError> {
    use tauri::Manager;

    let labels: Vec<String> = app.webview_windows().keys().cloned().collect();
    Ok(labels)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detached_tab_serialize() {
        let tab = DetachedTab {
            window_label: "tab-123".to_string(),
            tab_id: "123".to_string(),
            session_id: "sess-456".to_string(),
        };
        let json = serde_json::to_string(&tab).unwrap();
        assert!(json.contains("tab-123"));
        assert!(json.contains("sess-456"));
    }

    #[test]
    fn test_window_error_display() {
        let err = WindowError::NotFound("test".to_string());
        assert_eq!(err.to_string(), "Window not found: test");
    }

    #[test]
    fn test_window_error_tauri_variant_display() {
        let err = WindowError::Tauri("boom".to_string());
        assert_eq!(err.to_string(), "Tauri error: boom");
    }

    #[test]
    fn test_window_error_serializes_as_string() {
        let err = WindowError::NotFound("abc".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Window not found: abc\"");
    }

    #[test]
    fn test_tab_window_label_format() {
        assert_eq!(tab_window_label("123"), "tab-123");
        assert_eq!(tab_window_label(""), "tab-");
        assert_eq!(tab_window_label("abc-def"), "tab-abc-def");
    }

    #[test]
    fn test_detached_tab_serde_roundtrip() {
        let tab = DetachedTab {
            window_label: "tab-1".into(),
            tab_id: "1".into(),
            session_id: "s1".into(),
        };
        let json = serde_json::to_string(&tab).unwrap();
        let parsed: DetachedTab = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.window_label, "tab-1");
        assert_eq!(parsed.tab_id, "1");
        assert_eq!(parsed.session_id, "s1");
    }
}

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

    let label = format!("tab-{}", tab_id);
    let mut builder = WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(&title)
    .inner_size(900.0, 600.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true);

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
}

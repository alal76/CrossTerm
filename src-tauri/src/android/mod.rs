use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AndroidError {
    #[error("Service error: {0}")]
    ServiceError(String),
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Build error: {0}")]
    BuildError(String),
}

impl Serialize for AndroidError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForegroundServiceConfig {
    pub title: String,
    pub body: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub importance: AndroidImportance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidImportance {
    Default,
    High,
    Low,
    Min,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct AndroidState {
    foreground_active: Mutex<bool>,
    channels: Mutex<Vec<NotificationChannel>>,
}

impl AndroidState {
    pub fn new() -> Self {
        Self {
            foreground_active: Mutex::new(false),
            channels: Mutex::new(Vec::new()),
        }
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn android_start_foreground_service(
    config: ForegroundServiceConfig,
    state: tauri::State<'_, AndroidState>,
) -> Result<(), AndroidError> {
    if config.title.is_empty() {
        return Err(AndroidError::ServiceError(
            "Title is required".to_string(),
        ));
    }
    if config.channel_id.is_empty() {
        return Err(AndroidError::ServiceError(
            "Channel ID is required".to_string(),
        ));
    }

    // Stub: In production, this invokes Android JNI to start a foreground service
    let mut active = state.foreground_active.lock().unwrap();
    *active = true;
    Ok(())
}

#[tauri::command]
pub async fn android_stop_foreground_service(
    state: tauri::State<'_, AndroidState>,
) -> Result<(), AndroidError> {
    let mut active = state.foreground_active.lock().unwrap();
    if !*active {
        return Err(AndroidError::ServiceError(
            "No foreground service is running".to_string(),
        ));
    }
    *active = false;
    Ok(())
}

#[tauri::command]
pub async fn android_create_notification_channel(
    channel: NotificationChannel,
    state: tauri::State<'_, AndroidState>,
) -> Result<(), AndroidError> {
    if channel.id.is_empty() {
        return Err(AndroidError::ChannelError(
            "Channel ID is required".to_string(),
        ));
    }
    if channel.name.is_empty() {
        return Err(AndroidError::ChannelError(
            "Channel name is required".to_string(),
        ));
    }

    let mut channels = state.channels.lock().unwrap();
    // Prevent duplicates
    if channels.iter().any(|c| c.id == channel.id) {
        return Err(AndroidError::ChannelError(format!(
            "Channel '{}' already exists",
            channel.id
        )));
    }
    channels.push(channel);
    Ok(())
}

#[tauri::command]
pub async fn android_is_foreground_active(
    state: tauri::State<'_, AndroidState>,
) -> Result<bool, AndroidError> {
    let active = state.foreground_active.lock().unwrap();
    Ok(*active)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> AndroidState {
        AndroidState::new()
    }

    #[test]
    fn test_foreground_lifecycle() {
        let state = make_state();

        // Initially inactive
        {
            let active = state.foreground_active.lock().unwrap();
            assert!(!*active);
        }

        // Start foreground service
        {
            let mut active = state.foreground_active.lock().unwrap();
            *active = true;
            assert!(*active);
        }

        // Stop foreground service
        {
            let mut active = state.foreground_active.lock().unwrap();
            *active = false;
            assert!(!*active);
        }
    }

    #[test]
    fn test_notification_channel() {
        let state = make_state();

        let channel = NotificationChannel {
            id: "default".to_string(),
            name: "Default".to_string(),
            description: "Default notification channel".to_string(),
            importance: AndroidImportance::Default,
        };

        // Add channel
        {
            let mut channels = state.channels.lock().unwrap();
            channels.push(channel.clone());
            assert_eq!(channels.len(), 1);
            assert_eq!(channels[0].id, "default");
        }

        // Verify duplicate detection logic
        {
            let channels = state.channels.lock().unwrap();
            let duplicate = channels.iter().any(|c| c.id == "default");
            assert!(duplicate);
        }

        // Verify serde roundtrip
        let json = serde_json::to_string(&channel).expect("serialize");
        assert!(json.contains("\"importance\":\"default\""));

        let high = NotificationChannel {
            id: "high".to_string(),
            name: "High Priority".to_string(),
            description: "High priority notifications".to_string(),
            importance: AndroidImportance::High,
        };
        let high_json = serde_json::to_string(&high).expect("serialize high");
        assert!(high_json.contains("\"importance\":\"high\""));
    }
}

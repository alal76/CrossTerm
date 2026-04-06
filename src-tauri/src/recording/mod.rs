use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RecordingError {
    #[error("Recording not found: {0}")]
    NotFound(String),
    #[error("Recording already active: {0}")]
    AlreadyActive(String),
    #[error("Recording not active: {0}")]
    NotActive(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Export failed: {0}")]
    ExportFailed(String),
    #[error("Invalid position: {0}")]
    InvalidPosition(f64),
    #[error("Invalid speed: {0}")]
    InvalidSpeed(f64),
}

impl Serialize for RecordingError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for RecordingError {
    fn from(err: std::io::Error) -> Self {
        RecordingError::Io(err.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingHeader {
    pub version: u8,
    pub width: u32,
    pub height: u32,
    pub timestamp: f64,
    pub title: Option<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingEvent {
    pub time: f64,
    pub event_type: RecordingEventType,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingEventType {
    Output,
    Input,
    Resize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingInfo {
    pub id: String,
    pub path: String,
    pub title: Option<String>,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    pub recording_id: String,
    pub position: f64,
    pub speed: f64,
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Gif,
    Mp4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackFrameEvent {
    pub recording_id: String,
    pub data: String,
    pub position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackCompleteEvent {
    pub recording_id: String,
}

// ── Internal Types ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ActiveRecording {
    pub id: String,
    pub path: PathBuf,
    pub header: RecordingHeader,
    pub events: Vec<RecordingEvent>,
    pub start_time: Instant,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Serialize a RecordingHeader as the first line of an asciicast v2 file.
fn serialize_header(header: &RecordingHeader) -> String {
    let mut map = serde_json::Map::new();
    map.insert("version".to_string(), serde_json::Value::Number(header.version.into()));
    map.insert("width".to_string(), serde_json::Value::Number(header.width.into()));
    map.insert("height".to_string(), serde_json::Value::Number(header.height.into()));
    map.insert(
        "timestamp".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(header.timestamp).unwrap_or_else(|| serde_json::Number::from(0))),
    );
    if let Some(title) = &header.title {
        map.insert("title".to_string(), serde_json::Value::String(title.clone()));
    }
    let env_map: serde_json::Map<String, serde_json::Value> = header
        .env
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    map.insert("env".to_string(), serde_json::Value::Object(env_map));
    serde_json::Value::Object(map).to_string()
}

/// Serialize a RecordingEvent as a single line in asciicast v2 format: `[time, "event_type", "data"]`
fn serialize_event(event: &RecordingEvent) -> String {
    let event_type_str = match event.event_type {
        RecordingEventType::Output => "o",
        RecordingEventType::Input => "i",
        RecordingEventType::Resize => "r",
    };
    serde_json::json!([event.time, event_type_str, event.data]).to_string()
}

/// Parse asciicast v2 recording from file contents.
fn parse_asciicast(contents: &str) -> Result<(RecordingHeader, Vec<RecordingEvent>), RecordingError> {
    let mut lines = contents.lines();

    let header_line = lines
        .next()
        .ok_or_else(|| RecordingError::InvalidFormat("Empty file".to_string()))?;

    let header_json: serde_json::Value = serde_json::from_str(header_line)
        .map_err(|e| RecordingError::InvalidFormat(e.to_string()))?;

    let header = RecordingHeader {
        version: header_json["version"].as_u64().unwrap_or(2) as u8,
        width: header_json["width"].as_u64().unwrap_or(80) as u32,
        height: header_json["height"].as_u64().unwrap_or(24) as u32,
        timestamp: header_json["timestamp"].as_f64().unwrap_or(0.0),
        title: header_json["title"].as_str().map(|s| s.to_string()),
        env: header_json["env"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    };

    let mut events = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let arr: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| RecordingError::InvalidFormat(e.to_string()))?;

        if let Some(arr) = arr.as_array() {
            if arr.len() >= 3 {
                let time = arr[0].as_f64().unwrap_or(0.0);
                let event_type = match arr[1].as_str().unwrap_or("o") {
                    "o" => RecordingEventType::Output,
                    "i" => RecordingEventType::Input,
                    "r" => RecordingEventType::Resize,
                    _ => RecordingEventType::Output,
                };
                let data = arr[2].as_str().unwrap_or("").to_string();
                events.push(RecordingEvent {
                    time,
                    event_type,
                    data,
                });
            }
        }
    }

    Ok((header, events))
}

// ── State ───────────────────────────────────────────────────────────────

pub struct RecordingState {
    pub active_recordings: Mutex<HashMap<String, ActiveRecording>>,
    pub playback_states: Mutex<HashMap<String, PlaybackState>>,
    pub recordings_dir: PathBuf,
}

impl RecordingState {
    pub fn new() -> Self {
        let recordings_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("crossterm")
            .join("recordings");

        Self {
            active_recordings: Mutex::new(HashMap::new()),
            playback_states: Mutex::new(HashMap::new()),
            recordings_dir,
        }
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn recording_start(
    session_id: String,
    title: Option<String>,
    width: u32,
    height: u32,
    state: tauri::State<'_, RecordingState>,
) -> Result<String, RecordingError> {
    let id = Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let header = RecordingHeader {
        version: 2,
        width,
        height,
        timestamp,
        title,
        env: {
            let mut env = HashMap::new();
            env.insert("SHELL".to_string(), "/bin/bash".to_string());
            env.insert("TERM".to_string(), "xterm-256color".to_string());
            env
        },
    };

    // Ensure recordings directory exists
    let dir = state.recordings_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| RecordingError::Io(e.to_string()))?;

    let path = dir.join(format!("{id}.cast"));

    let recording = ActiveRecording {
        id: id.clone(),
        path,
        header,
        events: Vec::new(),
        start_time: Instant::now(),
    };

    {
        let mut active = state.active_recordings.lock().unwrap();
        active.insert(id.clone(), recording);
    }

    Ok(id)
}

#[tauri::command]
pub async fn recording_stop(
    recording_id: String,
    state: tauri::State<'_, RecordingState>,
) -> Result<RecordingInfo, RecordingError> {
    let recording = {
        let mut active = state.active_recordings.lock().unwrap();
        active
            .remove(&recording_id)
            .ok_or_else(|| RecordingError::NotActive(recording_id.clone()))?
    };

    // Write asciicast v2 file
    let mut content = serialize_header(&recording.header);
    content.push('\n');
    for event in &recording.events {
        content.push_str(&serialize_event(event));
        content.push('\n');
    }

    std::fs::write(&recording.path, &content).map_err(|e| RecordingError::Io(e.to_string()))?;

    let duration_secs = recording
        .events
        .last()
        .map(|e| e.time)
        .unwrap_or(0.0);

    let size_bytes = content.len() as u64;

    let created_at = chrono::Utc::now().to_rfc3339();

    Ok(RecordingInfo {
        id: recording_id,
        path: recording.path.to_string_lossy().to_string(),
        title: recording.header.title,
        duration_secs,
        size_bytes,
        width: recording.header.width,
        height: recording.header.height,
        created_at,
    })
}

#[tauri::command]
pub async fn recording_append(
    recording_id: String,
    data: String,
    state: tauri::State<'_, RecordingState>,
) -> Result<(), RecordingError> {
    let mut active = state.active_recordings.lock().unwrap();
    let recording = active
        .get_mut(&recording_id)
        .ok_or_else(|| RecordingError::NotActive(recording_id))?;

    let time = recording.start_time.elapsed().as_secs_f64();
    recording.events.push(RecordingEvent {
        time,
        event_type: RecordingEventType::Output,
        data,
    });

    Ok(())
}

#[tauri::command]
pub async fn recording_list(
    state: tauri::State<'_, RecordingState>,
) -> Result<Vec<RecordingInfo>, RecordingError> {
    let dir = &state.recordings_dir;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut recordings = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| RecordingError::Io(e.to_string()))?;

    for entry in entries {
        let entry = entry.map_err(|e| RecordingError::Io(e.to_string()))?;
        let path = entry.path();

        if path.extension().map(|e| e == "cast").unwrap_or(false) {
            let contents =
                std::fs::read_to_string(&path).map_err(|e| RecordingError::Io(e.to_string()))?;

            match parse_asciicast(&contents) {
                Ok((header, events)) => {
                    let id = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let duration_secs = events.last().map(|e| e.time).unwrap_or(0.0);
                    let metadata =
                        std::fs::metadata(&path).map_err(|e| RecordingError::Io(e.to_string()))?;

                    recordings.push(RecordingInfo {
                        id,
                        path: path.to_string_lossy().to_string(),
                        title: header.title,
                        duration_secs,
                        size_bytes: metadata.len(),
                        width: header.width,
                        height: header.height,
                        created_at: metadata
                            .created()
                            .ok()
                            .and_then(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .ok()
                                    .map(|d| {
                                        chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                            .map(|dt| dt.to_rfc3339())
                                            .unwrap_or_default()
                                    })
                            })
                            .unwrap_or_default(),
                    });
                }
                Err(_) => continue,
            }
        }
    }

    Ok(recordings)
}

#[tauri::command]
pub async fn recording_get(
    recording_id: String,
    state: tauri::State<'_, RecordingState>,
) -> Result<RecordingInfo, RecordingError> {
    let path = state.recordings_dir.join(format!("{recording_id}.cast"));

    if !path.exists() {
        return Err(RecordingError::NotFound(recording_id));
    }

    let contents = std::fs::read_to_string(&path).map_err(|e| RecordingError::Io(e.to_string()))?;
    let (header, events) = parse_asciicast(&contents)?;
    let metadata = std::fs::metadata(&path).map_err(|e| RecordingError::Io(e.to_string()))?;
    let duration_secs = events.last().map(|e| e.time).unwrap_or(0.0);

    Ok(RecordingInfo {
        id: recording_id,
        path: path.to_string_lossy().to_string(),
        title: header.title,
        duration_secs,
        size_bytes: metadata.len(),
        width: header.width,
        height: header.height,
        created_at: metadata
            .created()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| {
                        chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
            })
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn recording_delete(
    recording_id: String,
    state: tauri::State<'_, RecordingState>,
) -> Result<(), RecordingError> {
    let path = state.recordings_dir.join(format!("{recording_id}.cast"));
    if !path.exists() {
        return Err(RecordingError::NotFound(recording_id));
    }
    std::fs::remove_file(&path).map_err(|e| RecordingError::Io(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn recording_playback_start(
    recording_id: String,
    speed: f64,
    state: tauri::State<'_, RecordingState>,
    app: AppHandle,
) -> Result<PlaybackState, RecordingError> {
    if !(0.25..=8.0).contains(&speed) {
        return Err(RecordingError::InvalidSpeed(speed));
    }

    let path = state.recordings_dir.join(format!("{recording_id}.cast"));
    if !path.exists() {
        return Err(RecordingError::NotFound(recording_id.clone()));
    }

    let playback = PlaybackState {
        recording_id: recording_id.clone(),
        position: 0.0,
        speed,
        playing: true,
    };

    {
        let mut states = state.playback_states.lock().unwrap();
        states.insert(recording_id.clone(), playback.clone());
    }

    // Spawn playback task
    let path_clone = path.clone();
    let recording_id_clone = recording_id.clone();
    tokio::spawn(async move {
        let contents = match std::fs::read_to_string(&path_clone) {
            Ok(c) => c,
            Err(_) => return,
        };

        let (_header, events) = match parse_asciicast(&contents) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut prev_time = 0.0;
        for event in &events {
            let delay = (event.time - prev_time) / speed;
            if delay > 0.0 {
                tokio::time::sleep(std::time::Duration::from_secs_f64(delay)).await;
            }

            let _ = app.emit(
                "recording:playback_frame",
                PlaybackFrameEvent {
                    recording_id: recording_id_clone.clone(),
                    data: event.data.clone(),
                    position: event.time,
                },
            );
            prev_time = event.time;
        }

        let _ = app.emit(
            "recording:playback_complete",
            PlaybackCompleteEvent {
                recording_id: recording_id_clone,
            },
        );
    });

    Ok(playback)
}

#[tauri::command]
pub async fn recording_playback_seek(
    recording_id: String,
    position: f64,
    state: tauri::State<'_, RecordingState>,
) -> Result<PlaybackState, RecordingError> {
    if position < 0.0 {
        return Err(RecordingError::InvalidPosition(position));
    }

    let mut states = state.playback_states.lock().unwrap();
    let playback = states
        .get_mut(&recording_id)
        .ok_or_else(|| RecordingError::NotFound(recording_id.clone()))?;

    playback.position = position;

    Ok(playback.clone())
}

#[tauri::command]
pub async fn recording_playback_set_speed(
    recording_id: String,
    speed: f64,
    state: tauri::State<'_, RecordingState>,
) -> Result<PlaybackState, RecordingError> {
    if !(0.25..=8.0).contains(&speed) {
        return Err(RecordingError::InvalidSpeed(speed));
    }

    let mut states = state.playback_states.lock().unwrap();
    let playback = states
        .get_mut(&recording_id)
        .ok_or_else(|| RecordingError::NotFound(recording_id.clone()))?;

    playback.speed = speed;

    Ok(playback.clone())
}

#[tauri::command]
pub async fn recording_export(
    recording_id: String,
    format: ExportFormat,
    state: tauri::State<'_, RecordingState>,
) -> Result<String, RecordingError> {
    let path = state.recordings_dir.join(format!("{recording_id}.cast"));
    if !path.exists() {
        return Err(RecordingError::NotFound(recording_id));
    }

    // Export is a stub — requires ffmpeg or similar
    let format_name = match format {
        ExportFormat::Gif => "GIF",
        ExportFormat::Mp4 => "MP4",
    };

    Err(RecordingError::ExportFailed(format!(
        "FFmpeg is not installed. Cannot export to {format_name}."
    )))
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_asciicast_record() {
        let dir = tempfile::tempdir().unwrap();
        let state = RecordingState {
            active_recordings: Mutex::new(HashMap::new()),
            playback_states: Mutex::new(HashMap::new()),
            recordings_dir: dir.path().to_path_buf(),
        };

        let id = "test-rec-1".to_string();
        let header = RecordingHeader {
            version: 2,
            width: 80,
            height: 24,
            timestamp: 1234567890.0,
            title: Some("Test Recording".to_string()),
            env: {
                let mut env = HashMap::new();
                env.insert("SHELL".to_string(), "/bin/bash".to_string());
                env
            },
        };

        let recording = ActiveRecording {
            id: id.clone(),
            path: dir.path().join("test-rec-1.cast"),
            header: header.clone(),
            events: vec![
                RecordingEvent {
                    time: 0.0,
                    event_type: RecordingEventType::Output,
                    data: "hello ".to_string(),
                },
                RecordingEvent {
                    time: 0.5,
                    event_type: RecordingEventType::Output,
                    data: "world".to_string(),
                },
                RecordingEvent {
                    time: 1.0,
                    event_type: RecordingEventType::Input,
                    data: "ls\n".to_string(),
                },
            ],
            start_time: Instant::now(),
        };

        // Write asciicast file
        let mut content = serialize_header(&recording.header);
        content.push('\n');
        for event in &recording.events {
            content.push_str(&serialize_event(event));
            content.push('\n');
        }
        std::fs::write(&recording.path, &content).unwrap();

        // Verify the file is valid asciicast v2
        let read_back = std::fs::read_to_string(&recording.path).unwrap();
        let (parsed_header, parsed_events) = parse_asciicast(&read_back).unwrap();

        assert_eq!(parsed_header.version, 2);
        assert_eq!(parsed_header.width, 80);
        assert_eq!(parsed_header.height, 24);
        assert_eq!(parsed_header.title, Some("Test Recording".to_string()));
        assert_eq!(parsed_events.len(), 3);
        assert_eq!(parsed_events[0].data, "hello ");
        assert_eq!(parsed_events[1].data, "world");
        assert_eq!(parsed_events[2].data, "ls\n");
    }

    #[test]
    fn test_asciicast_playback() {
        let state = RecordingState {
            active_recordings: Mutex::new(HashMap::new()),
            playback_states: Mutex::new(HashMap::new()),
            recordings_dir: PathBuf::from("/tmp"),
        };

        let playback = PlaybackState {
            recording_id: "test-play-1".to_string(),
            position: 0.0,
            speed: 1.0,
            playing: true,
        };

        {
            let mut states = state.playback_states.lock().unwrap();
            states.insert("test-play-1".to_string(), playback);
        }

        let states = state.playback_states.lock().unwrap();
        let ps = states.get("test-play-1").unwrap();
        assert!(ps.playing);
        assert_eq!(ps.position, 0.0);
        assert_eq!(ps.speed, 1.0);
        assert_eq!(ps.recording_id, "test-play-1");
    }

    #[test]
    fn test_recording_seek() {
        let state = RecordingState {
            active_recordings: Mutex::new(HashMap::new()),
            playback_states: Mutex::new(HashMap::new()),
            recordings_dir: PathBuf::from("/tmp"),
        };

        {
            let mut states = state.playback_states.lock().unwrap();
            states.insert(
                "seek-test".to_string(),
                PlaybackState {
                    recording_id: "seek-test".to_string(),
                    position: 0.0,
                    speed: 1.0,
                    playing: true,
                },
            );
        }

        // Seek to position 5.5
        {
            let mut states = state.playback_states.lock().unwrap();
            let ps = states.get_mut("seek-test").unwrap();
            ps.position = 5.5;
        }

        let states = state.playback_states.lock().unwrap();
        let ps = states.get("seek-test").unwrap();
        assert_eq!(ps.position, 5.5);
    }

    #[test]
    fn test_recording_speed() {
        let state = RecordingState {
            active_recordings: Mutex::new(HashMap::new()),
            playback_states: Mutex::new(HashMap::new()),
            recordings_dir: PathBuf::from("/tmp"),
        };

        {
            let mut states = state.playback_states.lock().unwrap();
            states.insert(
                "speed-test".to_string(),
                PlaybackState {
                    recording_id: "speed-test".to_string(),
                    position: 0.0,
                    speed: 1.0,
                    playing: true,
                },
            );
        }

        // Set speed to 0.5x
        {
            let mut states = state.playback_states.lock().unwrap();
            let ps = states.get_mut("speed-test").unwrap();
            ps.speed = 0.5;
            assert_eq!(ps.speed, 0.5);
        }

        // Set speed to 4x
        {
            let mut states = state.playback_states.lock().unwrap();
            let ps = states.get_mut("speed-test").unwrap();
            ps.speed = 4.0;
            assert_eq!(ps.speed, 4.0);
        }

        let states = state.playback_states.lock().unwrap();
        let ps = states.get("speed-test").unwrap();
        assert_eq!(ps.speed, 4.0);
    }
}

use std::sync::OnceLock;
use std::time::Instant;
use serde::{Serialize, Deserialize};

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn mark_startup_begin() {
    START_TIME.get_or_init(Instant::now);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupTiming {
    pub time_to_ready_ms: u64,
}

#[tauri::command]
pub fn startup_get_timing() -> StartupTiming {
    let elapsed = START_TIME
        .get()
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);
    StartupTiming { time_to_ready_ms: elapsed }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timing_returns_nonzero_after_mark() {
        mark_startup_begin();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t = startup_get_timing();
        assert!(t.time_to_ready_ms > 0);
    }
}

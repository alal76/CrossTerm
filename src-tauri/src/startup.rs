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

/// Return the current OS user's login name. Used by the UI as a sensible
/// default when the user does not supply a username (e.g. QuickConnect).
#[tauri::command]
pub fn system_default_username() -> String {
    #[cfg(windows)]
    let key = "USERNAME";
    #[cfg(not(windows))]
    let key = "USER";
    std::env::var(key).unwrap_or_default()
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

    #[test]
    fn default_username_returns_env_value() {
        // CI environments may not set USER, so we set a known value first.
        #[cfg(windows)]
        let key = "USERNAME";
        #[cfg(not(windows))]
        let key = "USER";

        // SAFETY: tests run single-threaded for env-mutating cases via #[cfg]
        unsafe { std::env::set_var(key, "alice"); }
        assert_eq!(system_default_username(), "alice");

        unsafe { std::env::remove_var(key); }
        assert_eq!(system_default_username(), "");
    }
}

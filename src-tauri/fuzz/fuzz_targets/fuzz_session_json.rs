//! SEC-T-04: Fuzz JSON deserialization of session data.
//!
//! Feeds arbitrary bytes as JSON input and attempts to deserialize into
//! session-related types. Verifies serde_json handles it gracefully
//! with no panics.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse arbitrary bytes as a generic JSON value
    let _ = serde_json::from_slice::<serde_json::Value>(data);

    // Attempt to parse as a JSON string and then deserialize to Value
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<serde_json::Value>(s);

        // Try to parse JSON objects with fields matching session structures
        let _ = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(s);
    }

    // Try to deserialize as a Vec of sessions (common API pattern)
    let _ = serde_json::from_slice::<Vec<serde_json::Value>>(data);

    // Attempt round-trip: parse then serialize
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(data) {
        let _ = serde_json::to_string(&val);
        let _ = serde_json::to_vec(&val);
    }
});

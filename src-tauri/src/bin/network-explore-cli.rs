//! Standalone Network Explorer runner — no Tauri app, no vault unlock, no UI
//! login. Runs the exact same scan pipeline as the app's Network Explorer
//! (`network_explore_start`) and writes the full result set as JSON.
//!
//! Usage:
//!   cargo run --bin network-explore-cli -- [CIDR] [OUT_PATH] [TIMEOUT_MS]
//!
//! Defaults: CIDR=192.168.0.0/24, OUT_PATH=network_scan.json, TIMEOUT_MS=1500
//!
//! Scans the default service/port set (`DEFAULT_EXPLORE_SERVICES`) — the
//! same one the app uses by default. For a custom port list, adjust
//! `run_explore_and_dump`'s `services`/`extra_ports` arguments directly.

use app_lib::network::run_explore_and_dump;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cidr = args.get(1).cloned().unwrap_or_else(|| "192.168.0.0/24".to_string());
    let out_path = args.get(2).cloned().unwrap_or_else(|| "network_scan.json".to_string());
    let timeout_ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1500);

    eprintln!("scanning {cidr} (timeout {timeout_ms}ms/probe)...");
    match run_explore_and_dump(&cidr, None, &[], timeout_ms, &out_path).await {
        Ok(count) => eprintln!("wrote {count} host(s) to {out_path}"),
        Err(e) => {
            eprintln!("scan failed: {e}");
            std::process::exit(1);
        }
    }
}

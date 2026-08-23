//! Live scanning: shells out to the `network-explore-cli` binary built
//! alongside this one, in a background thread, so the UI stays responsive
//! while a scan runs.
//!
//! This deliberately shells out rather than linking `app_lib` (the Tauri
//! app's own crate) directly: crossterm-audit-tui's whole point is to have
//! none of app_lib's Tauri/GTK/WebKit dependency tree, and depending on
//! app_lib as a library would pull that in at compile time regardless of
//! which code paths actually run.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;

pub enum ScanOutcome {
    Done(PathBuf),
    Failed(String),
}

/// Locates the `network-explore-cli` binary: first right beside this
/// executable (the common case — both binaries come from the same
/// workspace build and are typically deployed together), falling back to
/// whatever `network-explore-cli` resolves to on `PATH`.
fn locate_cli() -> PathBuf {
    if let Ok(mut path) = std::env::current_exe() {
        let name = if cfg!(windows) { "network-explore-cli.exe" } else { "network-explore-cli" };
        path.set_file_name(name);
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from("network-explore-cli")
}

/// Kicks off a scan of `cidr` in a background thread, writing its JSON
/// output to a fresh temp file. The returned receiver yields exactly one
/// `ScanOutcome` once the subprocess finishes.
pub fn start_scan(cidr: String) -> mpsc::Receiver<ScanOutcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let outcome = run_scan(&cidr);
        // Only fails if the receiver was dropped (e.g. the app already
        // quit) — nothing useful to do about that, so ignore it.
        let _ = tx.send(outcome);
    });
    rx
}

fn run_scan(cidr: &str) -> ScanOutcome {
    let out_path = std::env::temp_dir().join(format!("crossterm-audit-tui-scan-{}.json", std::process::id()));
    let cli = locate_cli();
    let output = Command::new(&cli)
        .arg(cidr)
        .arg("--out")
        .arg(&out_path)
        .arg("--quiet")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(o) if o.status.success() => ScanOutcome::Done(out_path),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let detail = stderr.trim();
            let suffix = if detail.is_empty() { String::new() } else { format!(": {detail}") };
            ScanOutcome::Failed(format!("{} exited with {}{suffix}", cli.display(), o.status))
        }
        Err(e) => ScanOutcome::Failed(format!("couldn't run {}: {e}", cli.display())),
    }
}

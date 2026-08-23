//! crossterm-audit-tui — a standalone, full-screen terminal UI for auditing
//! network reachability, built on the same scan-result JSON schema
//! `network-explore-cli` writes. No Tauri/GUI dependency at build or run
//! time; see docs/network-audit-tui-plan.md for the design and roadmap this
//! implements (Phase 2: live scanning via `network-explore-cli`, shelled out
//! to in the background — Phase 1 was a static host browser only).
//!
//! Usage:
//!   crossterm-audit-tui [SCAN_JSON]
//!
//! Run `network-explore-cli` first to produce a scan file, then browse it
//! here: `network-explore-cli 192.168.1.0/24 && crossterm-audit-tui network_scan.json`
//! — or start with nothing loaded and press `n` inside the browser to run a
//! scan directly.

mod app;
mod model;
mod scan;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "crossterm-audit-tui",
    about = "Full-screen TUI for browsing CrossTerm network-audit scan results.",
    long_about = "Browses the JSON scan output of network-explore-cli in a full-screen, \
keyboard-driven interface, MC/nano-style. No Tauri/GUI dependency. \
See docs/network-audit-tui-plan.md for the design and roadmap."
)]
struct Args {
    /// Path to a scan JSON file produced by network-explore-cli. If omitted,
    /// starts with no data loaded.
    file: Option<PathBuf>,
}

/// RAII guard: restores the real terminal (leaves raw mode / alternate
/// screen) on drop, including on panic — a TUI that dies mid-render and
/// leaves the user's shell in raw mode with no visible cursor is a real,
/// user-hostile failure mode worth guarding against explicitly.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut app = App::new();
    if let Some(path) = &args.file {
        if let Err(e) = app.load_file(path) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    let _guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run(&mut terminal, &mut app);

    // _guard's Drop restores the terminal before we print anything further,
    // so a load/runtime error below lands on the user's real shell.
    drop(_guard);

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // A shorter poll timeout while a background scan is running means
        // its completion gets picked up (and the screen refreshed) promptly
        // instead of waiting out a full idle-tick; 200ms is plenty
        // responsive the rest of the time without busy-looping.
        let poll_timeout = if app.is_scanning() { Duration::from_millis(50) } else { Duration::from_millis(200) };
        if event::poll(poll_timeout)? {
            if let Event::Key(key) = event::read()? {
                // Crossterm reports both press and release on some
                // platforms/terminals; only act on press to avoid
                // double-handling a single keystroke.
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }

        app.poll_scan();

        if app.should_quit {
            return Ok(());
        }
    }
}

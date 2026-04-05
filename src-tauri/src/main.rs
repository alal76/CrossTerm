// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // ── HELP-35: Handle --help flag for Linux man-page style usage ──
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        println!("CrossTerm — Cross-platform terminal emulator & remote access suite");
        println!();
        println!("USAGE:");
        println!("    crossterm [OPTIONS]");
        println!();
        println!("OPTIONS:");
        println!("    -h, --help       Print this help message and exit");
        println!("    --version        Print version information and exit");
        println!();
        println!("CrossTerm provides SSH, SFTP, and local terminal sessions with an");
        println!("encrypted credential vault, split panes, and a customizable interface.");
        println!();
        println!("For more information, visit the built-in help panel (F1) or see");
        println!("the documentation at docs/help/.");
        std::process::exit(0);
    }

    app_lib::run();
}

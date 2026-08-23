//! Application state and key-handling. Deliberately kept free of any
//! terminal/rendering calls so it's unit-testable directly — `handle_key`
//! takes a plain `KeyCode` and mutates `App`, nothing more.

use crate::model::ExploreDump;
use crate::scan::{self, ScanOutcome};
use crossterm::event::KeyCode;
use std::path::Path;
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Browser,
    Detail(usize),
    Help,
    QuitConfirm,
    /// Typing a CIDR to scan; the String is the input buffer so far.
    ScanPrompt(String),
    /// A background scan of this CIDR is running.
    Scanning(String),
    /// The last scan (or the load of its result) failed; dismiss with any key.
    ScanError(String),
}

pub struct App {
    pub dump: Option<ExploreDump>,
    pub source_path: Option<String>,
    pub selected: usize,
    pub screen: Screen,
    pub should_quit: bool,
    /// Set while a background scan is running; polled once per tick by
    /// `poll_scan`. `None` the rest of the time.
    pending_scan: Option<mpsc::Receiver<ScanOutcome>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            dump: None,
            source_path: None,
            selected: 0,
            screen: Screen::Browser,
            should_quit: false,
            pending_scan: None,
        }
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let contents = std::fs::read_to_string(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
        let dump: ExploreDump = serde_json::from_str(&contents).map_err(|e| format!("couldn't parse {}: {e}", path.display()))?;
        self.source_path = Some(path.display().to_string());
        self.dump = Some(dump);
        self.selected = 0;
        Ok(())
    }

    fn host_count(&self) -> usize {
        self.dump.as_ref().map(|d| d.results.len()).unwrap_or(0)
    }

    /// True while a background scan is running — used by the event loop to
    /// pick a shorter poll timeout so the UI notices completion promptly.
    pub fn is_scanning(&self) -> bool {
        self.pending_scan.is_some()
    }

    /// Pure key-handling: given the current screen and a key, decide the
    /// next state. No I/O, no rendering — this is what the event loop calls,
    /// and what tests call directly.
    pub fn handle_key(&mut self, key: KeyCode) {
        match &self.screen {
            Screen::Browser => self.handle_key_browser(key),
            Screen::Detail(_) => {
                if matches!(key, KeyCode::Esc) {
                    self.screen = Screen::Browser;
                }
            }
            Screen::Help => {
                if matches!(key, KeyCode::Esc) {
                    self.screen = Screen::Browser;
                }
            }
            Screen::QuitConfirm => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.should_quit = true,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.screen = Screen::Browser,
                _ => {}
            },
            Screen::ScanPrompt(_) => self.handle_key_scan_prompt(key),
            // A scan in flight has nothing to cancel yet (no Child handle
            // kept around) — ignore input until it finishes. Revisit if a
            // long-running scan on a big CIDR turns out to need this.
            Screen::Scanning(_) => {}
            Screen::ScanError(_) => self.screen = Screen::Browser,
        }
    }

    fn handle_key_browser(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.host_count().saturating_sub(1);
                if self.selected < max {
                    self.selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::F(3) => {
                if self.host_count() > 0 {
                    self.screen = Screen::Detail(self.selected);
                }
            }
            KeyCode::F(1) => self.screen = Screen::Help,
            KeyCode::F(10) | KeyCode::Char('q') => self.screen = Screen::QuitConfirm,
            KeyCode::Char('n') | KeyCode::F(2) => {
                // Pre-fill with the currently loaded CIDR, if any, so
                // re-scanning the same range is just Enter.
                let prefill = self.dump.as_ref().map(|d| d.cidr.clone()).unwrap_or_default();
                self.screen = Screen::ScanPrompt(prefill);
            }
            _ => {}
        }
    }

    fn handle_key_scan_prompt(&mut self, key: KeyCode) {
        let Screen::ScanPrompt(input) = &mut self.screen else { return };
        match key {
            KeyCode::Char(c) => input.push(c),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Esc => self.screen = Screen::Browser,
            KeyCode::Enter => self.confirm_scan(),
            _ => {}
        }
    }

    /// Starts a background scan for the CIDR currently typed into
    /// `Screen::ScanPrompt`. Ignored (stays on the prompt) if the input is
    /// empty.
    fn confirm_scan(&mut self) {
        let Screen::ScanPrompt(input) = &self.screen else { return };
        let cidr = input.trim().to_string();
        if cidr.is_empty() {
            return;
        }
        self.pending_scan = Some(scan::start_scan(cidr.clone()));
        self.screen = Screen::Scanning(cidr);
    }

    /// Checks whether a running background scan has finished, and if so,
    /// loads its result (or surfaces the error) and clears the pending
    /// receiver. Called once per tick from the event loop; a no-op if no
    /// scan is in flight or it hasn't finished yet.
    pub fn poll_scan(&mut self) {
        let Some(rx) = &self.pending_scan else { return };
        match rx.try_recv() {
            Ok(ScanOutcome::Done(path)) => {
                self.pending_scan = None;
                match self.load_file(&path) {
                    Ok(()) => self.screen = Screen::Browser,
                    Err(e) => self.screen = Screen::ScanError(e),
                }
            }
            Ok(ScanOutcome::Failed(msg)) => {
                self.pending_scan = None;
                self.screen = Screen::ScanError(msg);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending_scan = None;
                self.screen = Screen::ScanError("scan thread ended unexpectedly".to_string());
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_dump_file(host_count: usize) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let results: Vec<String> = (0..host_count)
            .map(|i| format!(r#"{{"ip": "10.0.0.{i}", "open_ports": []}}"#))
            .collect();
        write!(f, r#"{{"cidr": "10.0.0.0/24", "host_count": {host_count}, "results": [{}]}}"#, results.join(",")).unwrap();
        f
    }

    #[test]
    fn starts_on_browser_with_no_data_loaded() {
        let app = App::new();
        assert_eq!(app.screen, Screen::Browser);
        assert!(app.dump.is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn load_file_populates_dump_and_resets_selection() {
        let file = sample_dump_file(3);
        let mut app = App::new();
        app.selected = 2;
        app.load_file(file.path()).unwrap();
        assert_eq!(app.dump.as_ref().unwrap().results.len(), 3);
        assert_eq!(app.selected, 0);
        assert!(app.source_path.is_some());
    }

    #[test]
    fn load_file_reports_a_clear_error_for_missing_file() {
        let mut app = App::new();
        let err = app.load_file(Path::new("/nonexistent/path/scan.json")).unwrap_err();
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn load_file_reports_a_clear_error_for_invalid_json() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "not json").unwrap();
        let mut app = App::new();
        let err = app.load_file(f.path()).unwrap_err();
        assert!(err.contains("couldn't parse"));
    }

    #[test]
    fn down_arrow_moves_selection_and_stops_at_last_host() {
        let file = sample_dump_file(3);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected, 1);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected, 2);
        app.handle_key(KeyCode::Down); // already at last host - stays put
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn up_arrow_stops_at_zero() {
        let file = sample_dump_file(3);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected, 0, "selection must not underflow below the first host");
    }

    #[test]
    fn vim_keys_j_and_k_move_selection_same_as_arrows() {
        let file = sample_dump_file(2);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::Char('j'));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyCode::Char('k'));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn enter_opens_detail_for_selected_host_only_when_hosts_exist() {
        let mut app = App::new(); // no data loaded
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Browser, "must not open detail with nothing loaded");

        let file = sample_dump_file(2);
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Detail(1));
    }

    #[test]
    fn f3_is_equivalent_to_enter() {
        let file = sample_dump_file(1);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::F(3));
        assert_eq!(app.screen, Screen::Detail(0));
    }

    #[test]
    fn esc_from_detail_returns_to_browser() {
        let file = sample_dump_file(1);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.screen = Screen::Detail(0);
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Browser);
    }

    #[test]
    fn f1_opens_help_and_esc_returns_to_browser() {
        let mut app = App::new();
        app.handle_key(KeyCode::F(1));
        assert_eq!(app.screen, Screen::Help);
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Browser);
    }

    #[test]
    fn q_and_f10_both_open_quit_confirm() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('q'));
        assert_eq!(app.screen, Screen::QuitConfirm);

        let mut app2 = App::new();
        app2.handle_key(KeyCode::F(10));
        assert_eq!(app2.screen, Screen::QuitConfirm);
    }

    #[test]
    fn quit_confirm_y_sets_should_quit() {
        let mut app = App::new();
        app.screen = Screen::QuitConfirm;
        app.handle_key(KeyCode::Char('y'));
        assert!(app.should_quit);
    }

    #[test]
    fn quit_confirm_n_or_esc_cancels_back_to_browser_without_quitting() {
        let mut app = App::new();
        app.screen = Screen::QuitConfirm;
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.screen, Screen::Browser);
        assert!(!app.should_quit);

        let mut app2 = App::new();
        app2.screen = Screen::QuitConfirm;
        app2.handle_key(KeyCode::Esc);
        assert_eq!(app2.screen, Screen::Browser);
        assert!(!app2.should_quit);
    }

    #[test]
    fn quit_confirm_ignores_unrelated_keys() {
        let mut app = App::new();
        app.screen = Screen::QuitConfirm;
        app.handle_key(KeyCode::Char('x'));
        assert_eq!(app.screen, Screen::QuitConfirm, "an unrelated key must not dismiss the prompt either way");
        assert!(!app.should_quit);
    }

    #[test]
    fn n_opens_scan_prompt_empty_when_nothing_loaded() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.screen, Screen::ScanPrompt(String::new()));
    }

    #[test]
    fn n_opens_scan_prompt_prefilled_with_the_loaded_cidr() {
        let file = sample_dump_file(1);
        let mut app = App::new();
        app.load_file(file.path()).unwrap();
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.screen, Screen::ScanPrompt("10.0.0.0/24".to_string()));
    }

    #[test]
    fn f2_also_opens_scan_prompt() {
        let mut app = App::new();
        app.handle_key(KeyCode::F(2));
        assert_eq!(app.screen, Screen::ScanPrompt(String::new()));
    }

    #[test]
    fn typing_in_scan_prompt_appends_and_backspace_removes() {
        let mut app = App::new();
        app.screen = Screen::ScanPrompt(String::new());
        app.handle_key(KeyCode::Char('1'));
        app.handle_key(KeyCode::Char('0'));
        app.handle_key(KeyCode::Char('.'));
        assert_eq!(app.screen, Screen::ScanPrompt("10.".to_string()));
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.screen, Screen::ScanPrompt("10".to_string()));
    }

    #[test]
    fn esc_from_scan_prompt_cancels_to_browser() {
        let mut app = App::new();
        app.screen = Screen::ScanPrompt("10.0.0.0/24".to_string());
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Browser);
        assert!(!app.is_scanning());
    }

    #[test]
    fn enter_with_empty_scan_prompt_input_stays_on_the_prompt() {
        let mut app = App::new();
        app.screen = Screen::ScanPrompt(String::new());
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::ScanPrompt(String::new()));
        assert!(!app.is_scanning());
    }

    #[test]
    fn enter_with_a_cidr_starts_a_scan_and_moves_to_the_scanning_screen() {
        let mut app = App::new();
        app.screen = Screen::ScanPrompt("10.0.0.0/24".to_string());
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Scanning("10.0.0.0/24".to_string()));
        assert!(app.is_scanning());
    }

    #[test]
    fn keys_are_ignored_while_a_scan_is_in_flight() {
        let mut app = App::new();
        app.screen = Screen::ScanPrompt("10.0.0.0/24".to_string());
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Char('q'));
        assert_eq!(app.screen, Screen::Scanning("10.0.0.0/24".to_string()), "a scan in flight has nothing to cancel yet");
    }

    #[test]
    fn poll_scan_with_nothing_pending_is_a_noop() {
        let mut app = App::new();
        app.screen = Screen::Browser;
        app.poll_scan();
        assert_eq!(app.screen, Screen::Browser);
    }

    #[test]
    fn poll_scan_loads_the_result_and_returns_to_browser_on_success() {
        let file = sample_dump_file(2);
        let (tx, rx) = mpsc::channel();
        tx.send(ScanOutcome::Done(file.path().to_path_buf())).unwrap();

        let mut app = App::new();
        app.screen = Screen::Scanning("10.0.0.0/24".to_string());
        app.pending_scan = Some(rx);
        app.poll_scan();

        assert_eq!(app.screen, Screen::Browser);
        assert!(!app.is_scanning());
        assert_eq!(app.dump.as_ref().unwrap().results.len(), 2);
    }

    #[test]
    fn poll_scan_surfaces_an_error_if_the_result_file_cant_be_loaded() {
        let (tx, rx) = mpsc::channel();
        tx.send(ScanOutcome::Done(std::path::PathBuf::from("/nonexistent/scan.json"))).unwrap();

        let mut app = App::new();
        app.screen = Screen::Scanning("10.0.0.0/24".to_string());
        app.pending_scan = Some(rx);
        app.poll_scan();

        assert!(matches!(app.screen, Screen::ScanError(_)));
        assert!(!app.is_scanning());
    }

    #[test]
    fn poll_scan_surfaces_a_failed_outcome_as_a_scan_error() {
        let (tx, rx) = mpsc::channel();
        tx.send(ScanOutcome::Failed("network-explore-cli exited with status 1".to_string())).unwrap();

        let mut app = App::new();
        app.screen = Screen::Scanning("10.0.0.0/24".to_string());
        app.pending_scan = Some(rx);
        app.poll_scan();

        assert_eq!(app.screen, Screen::ScanError("network-explore-cli exited with status 1".to_string()));
        assert!(!app.is_scanning());
    }

    #[test]
    fn poll_scan_surfaces_an_error_if_the_scan_thread_disappears_without_sending() {
        let (tx, rx) = mpsc::channel::<ScanOutcome>();
        drop(tx);

        let mut app = App::new();
        app.screen = Screen::Scanning("10.0.0.0/24".to_string());
        app.pending_scan = Some(rx);
        app.poll_scan();

        assert!(matches!(app.screen, Screen::ScanError(_)));
        assert!(!app.is_scanning());
    }

    #[test]
    fn poll_scan_does_nothing_while_the_scan_is_still_running() {
        let (_tx, rx) = mpsc::channel();
        let mut app = App::new();
        app.screen = Screen::Scanning("10.0.0.0/24".to_string());
        app.pending_scan = Some(rx);
        app.poll_scan();
        assert_eq!(app.screen, Screen::Scanning("10.0.0.0/24".to_string()));
        assert!(app.is_scanning(), "receiver with no message yet must not be cleared");
    }

    #[test]
    fn any_key_dismisses_a_scan_error_back_to_the_browser() {
        let mut app = App::new();
        app.screen = Screen::ScanError("boom".to_string());
        app.handle_key(KeyCode::Char('z'));
        assert_eq!(app.screen, Screen::Browser);
    }
}

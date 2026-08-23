//! Application state and key-handling. Deliberately kept free of any
//! terminal/rendering calls so it's unit-testable directly — `handle_key`
//! takes a plain `KeyCode` and mutates `App`, nothing more.

use crate::model::ExploreDump;
use crossterm::event::KeyCode;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Browser,
    Detail(usize),
    Help,
    QuitConfirm,
}

pub struct App {
    pub dump: Option<ExploreDump>,
    pub source_path: Option<String>,
    pub selected: usize,
    pub screen: Screen,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            dump: None,
            source_path: None,
            selected: 0,
            screen: Screen::Browser,
            should_quit: false,
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
            _ => {}
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
}

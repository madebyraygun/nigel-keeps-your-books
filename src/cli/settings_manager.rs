use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::password_manager::{PasswordAction, PasswordManager};
use crate::db;
use crate::error::Result;
use crate::settings::{get_data_dir, load_settings, save_settings};
use crate::tui::{FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

pub enum SettingsAction {
    Continue,
    Close,
}

enum Screen {
    Main,
    EditingName,
    Password(PasswordManager),
}

/// Menu items on the main settings screen.
const MENU_BUSINESS_NAME: usize = 0;
const MENU_PASSWORD: usize = 1;
const MENU_UPDATE_CHECK: usize = 2;
const MENU_LAST: usize = MENU_UPDATE_CHECK;

pub struct SettingsManager {
    greeting: String,
    screen: Screen,
    selection: usize,
    company_name: String,
    edit_buffer: String,
    status_message: Option<(String, bool)>,
    status_ttl: u8,
    encrypted: bool,
    update_check: bool,
}

impl SettingsManager {
    pub fn new(conn: &Connection, greeting: &str) -> Result<Self> {
        let company_name = db::get_metadata(conn, "company_name").unwrap_or_default();
        let db_path = get_data_dir().join("nigel.db");
        let encrypted = db::is_encrypted(&db_path)?;
        let settings = load_settings();
        Ok(Self {
            greeting: greeting.to_string(),
            screen: Screen::Main,
            selection: 0,
            company_name,
            edit_buffer: String::new(),
            status_message: None,
            status_ttl: 0,
            encrypted,
            update_check: settings.update_check,
        })
    }

    fn set_status(&mut self, msg: String, success: bool) {
        self.status_message = Some((msg, success));
        self.status_ttl = 3;
    }

    fn tick_status(&mut self) {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::Main => self.draw_main(frame),
            Screen::EditingName => self.draw_main(frame),
            Screen::Password(mgr) => mgr.draw(frame),
        }
    }

    fn menu_row(label: &str, value: &str, selected: bool) -> Line<'static> {
        let marker = if selected { ">" } else { " " };
        let label_style = if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(vec![
            Span::styled(format!(" {marker} {label:<17}"), label_style),
            Span::styled(
                value.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ])
    }

    fn draw_main(&self, frame: &mut Frame) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        let [header_area, sep, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        // Header
        frame.render_widget(
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        let sep_line = "\u{2501}".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        // Content
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Settings",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Business Name row
        let name_selected = self.selection == MENU_BUSINESS_NAME;
        if let Screen::EditingName = &self.screen {
            let marker = if name_selected { ">" } else { " " };
            let label_style = if name_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {marker} Business Name    "), label_style),
                Span::styled(format!("{}_", self.edit_buffer), SELECTED_STYLE),
            ]));
        } else {
            let display_name = if self.company_name.is_empty() {
                "(not set)"
            } else {
                &self.company_name
            };
            lines.push(Self::menu_row("Business Name", display_name, name_selected));
        }

        lines.push(Line::from(""));

        // Password section
        let pw_status = if self.encrypted {
            "(encrypted)"
        } else {
            "(not set)"
        };
        lines.push(Self::menu_row(
            "Password",
            pw_status,
            self.selection == MENU_PASSWORD,
        ));

        lines.push(Line::from(""));

        // Update check toggle
        let uc_status = if self.update_check {
            "(enabled)"
        } else {
            "(disabled)"
        };
        lines.push(Self::menu_row(
            "Auto-update check",
            uc_status,
            self.selection == MENU_UPDATE_CHECK,
        ));

        // Status message
        if let Some((msg, success)) = &self.status_message {
            lines.push(Line::from(""));
            let color = if *success { Color::Green } else { Color::Red };
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(color),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        // Hints
        let hints = match &self.screen {
            Screen::EditingName => "Enter=save  Esc=cancel",
            _ => "Enter=select  Esc=back  q=quit",
        };
        frame.render_widget(
            Paragraph::new(format!(" {hints}")).style(FOOTER_STYLE),
            hints_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> SettingsAction {
        self.tick_status();

        match &mut self.screen {
            Screen::Main => self.handle_main_key(code, conn),
            Screen::EditingName => self.handle_edit_name_key(code, conn),
            Screen::Password(mgr) => {
                match mgr.handle_key(code) {
                    PasswordAction::Close => {
                        // Refresh encrypted status when returning from password manager
                        let db_path = get_data_dir().join("nigel.db");
                        match db::is_encrypted(&db_path) {
                            Ok(enc) => self.encrypted = enc,
                            Err(e) => {
                                // Preserve previous state rather than defaulting to false
                                self.set_status(
                                    format!("Could not verify encryption status: {e}"),
                                    false,
                                );
                            }
                        }
                        self.screen = Screen::Main;
                    }
                    PasswordAction::Continue => {}
                }
                SettingsAction::Continue
            }
        }
    }

    fn handle_main_key(&mut self, code: KeyCode, _conn: &Connection) -> SettingsAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => SettingsAction::Close,
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                SettingsAction::Continue
            }
            KeyCode::Down => {
                self.selection = (self.selection + 1).min(MENU_LAST);
                SettingsAction::Continue
            }
            KeyCode::Enter => {
                match self.selection {
                    MENU_BUSINESS_NAME => {
                        self.edit_buffer = self.company_name.clone();
                        self.screen = Screen::EditingName;
                    }
                    MENU_PASSWORD => match PasswordManager::new(&self.greeting) {
                        Ok(mgr) => self.screen = Screen::Password(mgr),
                        Err(e) => {
                            self.set_status(format!("Could not open password settings: {e}"), false)
                        }
                    },
                    MENU_UPDATE_CHECK => {
                        self.update_check = !self.update_check;
                        let mut settings = load_settings();
                        settings.update_check = self.update_check;
                        match save_settings(&settings) {
                            Ok(()) => {
                                let state = if self.update_check {
                                    "enabled"
                                } else {
                                    "disabled"
                                };
                                self.set_status(format!("Auto-update check {state}."), true);
                            }
                            Err(e) => {
                                // Revert on save failure
                                self.update_check = !self.update_check;
                                self.set_status(format!("Could not save setting: {e}"), false);
                            }
                        }
                    }
                    _ => {}
                }
                SettingsAction::Continue
            }
            _ => SettingsAction::Continue,
        }
    }

    fn handle_edit_name_key(&mut self, code: KeyCode, conn: &Connection) -> SettingsAction {
        match code {
            KeyCode::Esc => {
                self.edit_buffer.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Enter => {
                let new_name = self.edit_buffer.trim().to_string();
                match db::set_metadata(conn, "company_name", &new_name) {
                    Ok(()) => {
                        self.company_name = new_name;
                        self.set_status("Business name saved.".into(), true);
                    }
                    Err(e) => {
                        self.set_status(format!("Could not save business name: {e}"), false);
                    }
                }
                self.edit_buffer.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            _ => {}
        }
        SettingsAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn new_loads_company_name() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Acme LLC").unwrap();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.company_name, "Acme LLC");
    }

    #[test]
    fn new_with_no_company_name() {
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.company_name, "");
    }

    #[test]
    fn esc_closes() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        let action = mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(action, SettingsAction::Close));
    }

    #[test]
    fn q_closes() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        let action = mgr.handle_key(KeyCode::Char('q'), &conn);
        assert!(matches!(action, SettingsAction::Close));
    }

    #[test]
    fn navigate_menu() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.selection, MENU_BUSINESS_NAME);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_PASSWORD);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_UPDATE_CHECK);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_UPDATE_CHECK); // clamped
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, MENU_PASSWORD);
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, MENU_BUSINESS_NAME);
    }

    #[test]
    fn edit_business_name_save() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit mode
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::EditingName));

        // Type a name
        for c in "Test Corp".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        assert_eq!(mgr.edit_buffer, "Test Corp");

        // Save
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
        assert_eq!(mgr.company_name, "Test Corp");

        // Verify persisted
        let saved = db::get_metadata(&conn, "company_name").unwrap();
        assert_eq!(saved, "Test Corp");
    }

    #[test]
    fn edit_business_name_cancel() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Original").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit mode
        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "Changed".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }

        // Cancel
        mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
        assert_eq!(mgr.company_name, "Original");

        // Verify DB unchanged
        let saved = db::get_metadata(&conn, "company_name").unwrap();
        assert_eq!(saved, "Original");
    }

    #[test]
    fn edit_business_name_backspace() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "ABC".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Backspace, &conn);
        assert_eq!(mgr.edit_buffer, "AB");
    }

    #[test]
    fn enter_password_screen() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Navigate to password
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_PASSWORD);

        // Enter password manager
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Password(_)));

        // Esc returns to main
        mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
    }

    #[test]
    fn edit_trims_whitespace() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "  Acme LLC  ".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.company_name, "Acme LLC");
        assert_eq!(db::get_metadata(&conn, "company_name").unwrap(), "Acme LLC");
    }

    #[test]
    fn edit_empty_name_saves() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Old Name").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit, clear, save empty
        mgr.handle_key(KeyCode::Enter, &conn);
        // Buffer is pre-populated; clear it
        for _ in 0..mgr.edit_buffer.len() {
            mgr.handle_key(KeyCode::Backspace, &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.company_name, "");
    }

    #[test]
    fn edit_prepopulates_buffer() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Existing Corp").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::EditingName));
        assert_eq!(mgr.edit_buffer, "Existing Corp");
    }

    #[test]
    fn status_message_ttl() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Save a name to trigger status message
        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "Test".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(mgr.status_message.is_some());

        // 3 more keypresses should keep status alive (TTL decrements from 3)
        mgr.handle_key(KeyCode::Down, &conn); // tick 3->2
        assert!(mgr.status_message.is_some());
        mgr.handle_key(KeyCode::Up, &conn); // tick 2->1
        assert!(mgr.status_message.is_some());
        mgr.handle_key(KeyCode::Down, &conn); // tick 1->0, cleared
        assert!(mgr.status_message.is_none());
    }

    #[test]
    fn toggle_update_check() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Default is enabled
        assert!(mgr.update_check);

        // Navigate to update check menu item
        mgr.handle_key(KeyCode::Down, &conn);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_UPDATE_CHECK);

        // Toggle off
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(!mgr.update_check);

        // Toggle back on
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(mgr.update_check);
    }

    #[test]
    fn update_check_loads_from_settings() {
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        // update_check defaults to true from settings
        assert!(mgr.update_check);
    }
}

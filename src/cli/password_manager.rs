use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use zeroize::Zeroize;

use crate::db;
use crate::error::Result;
use crate::settings::get_data_dir;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

#[derive(Clone, Copy)]
enum Action {
    Set,
    Change,
    Remove,
}

const ACTIONS_ENCRYPTED: &[(&str, usize)] = &[
    ("Change password", 0), // Action::Change
    ("Remove password", 1), // Action::Remove
];

const ACTIONS_PLAIN: &[(&str, usize)] = &[
    ("Set password", 2), // Action::Set
];

enum Phase {
    Menu,
    InputCurrent,
    InputNew,
    InputConfirm,
    Result(String, bool),
}

pub struct PasswordManager {
    encrypted: bool,
    selection: usize,
    phase: Phase,
    current_pw: String,
    new_pw: String,
    confirm_pw: String,
    cursor: usize,
    chosen_action: Option<Action>,
    greeting: String,
}

pub enum PasswordAction {
    Continue,
    Close,
}

impl PasswordManager {
    pub fn new(greeting: &str) -> Result<Self> {
        let db_path = get_data_dir().join("nigel.db");
        let encrypted = db::is_encrypted(&db_path)?;
        Ok(Self {
            encrypted,
            selection: 0,
            phase: Phase::Menu,
            current_pw: String::new(),
            new_pw: String::new(),
            confirm_pw: String::new(),
            cursor: 0,
            chosen_action: None,
            greeting: greeting.to_string(),
        })
    }

    fn actions(&self) -> &[(&str, usize)] {
        if self.encrypted {
            ACTIONS_ENCRYPTED
        } else {
            ACTIONS_PLAIN
        }
    }

    fn active_input_mut(&mut self) -> &mut String {
        match self.phase {
            Phase::InputCurrent => &mut self.current_pw,
            Phase::InputNew => &mut self.new_pw,
            Phase::InputConfirm => &mut self.confirm_pw,
            _ => unreachable!(),
        }
    }

    fn execute(&mut self) {
        let db_path = get_data_dir().join("nigel.db");
        let result = match self.chosen_action {
            Some(Action::Set) => {
                if self.new_pw.trim() != self.confirm_pw.trim() {
                    Err("Passwords do not match.".to_string())
                } else if self.new_pw.trim().is_empty() {
                    Err("Password cannot be empty.".to_string())
                } else {
                    self.do_encrypt(&db_path)
                }
            }
            Some(Action::Change) => {
                if self.new_pw.trim() != self.confirm_pw.trim() {
                    Err("Passwords do not match.".to_string())
                } else if self.new_pw.trim().is_empty() {
                    Err("New password cannot be empty.".to_string())
                } else {
                    self.do_change(&db_path)
                }
            }
            Some(Action::Remove) => self.do_remove(&db_path),
            None => Err("No action selected.".to_string()),
        };

        match result {
            Ok(msg) => {
                self.encrypted =
                    !self.encrypted || matches!(self.chosen_action, Some(Action::Change));
                self.phase = Phase::Result(msg, true);
            }
            Err(msg) => {
                self.phase = Phase::Result(msg, false);
            }
        }
    }

    fn do_encrypt(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        let pw = self.new_pw.trim();
        let trimmed = pw.len() != self.new_pw.len();
        super::password::encrypt_database(db_path, pw).map_err(|e| e.to_string())?;
        db::set_db_password(Some(pw.to_string()));
        let mut msg = "Database encrypted successfully.".to_string();
        if trimmed {
            msg.push_str(" Note: leading/trailing spaces were removed from password.");
        }
        Ok(msg)
    }

    fn do_change(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        let new_pw = self.new_pw.trim();
        let trimmed = new_pw.len() != self.new_pw.len();
        super::password::rekey_database(db_path, self.current_pw.trim(), new_pw)
            .map_err(|e| e.to_string())?;
        db::set_db_password(Some(new_pw.to_string()));
        let mut msg = "Password changed successfully.".to_string();
        if trimmed {
            msg.push_str(" Note: leading/trailing spaces were removed from password.");
        }
        Ok(msg)
    }

    fn do_remove(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        super::password::decrypt_database(db_path, self.current_pw.trim())
            .map_err(|e| e.to_string())?;
        db::set_db_password(None);
        Ok("Database decrypted. Password removed.".into())
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let [_top, greeting_area, _gap0, title_area, _gap, content_area, _gap2, hints_area, _bottom] =
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        frame.render_widget(
            Paragraph::new(Span::styled(&self.greeting, HEADER_STYLE))
                .alignment(ratatui::layout::Alignment::Center),
            greeting_area,
        );

        let status = if self.encrypted {
            "Database is encrypted"
        } else {
            "Database is not encrypted"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("Password Management  ({status})"),
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(ratatui::layout::Alignment::Center),
            title_area,
        );

        let form_width = 50u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered = Rect::new(form_x, content_area.y, form_width, content_area.height);

        match &self.phase {
            Phase::Menu => self.draw_menu(frame, centered),
            Phase::InputCurrent => {
                self.draw_input(frame, centered, "Current password:", &self.current_pw)
            }
            Phase::InputNew => {
                self.draw_input(frame, centered, "New password:", &self.new_pw)
            }
            Phase::InputConfirm => {
                self.draw_input(frame, centered, "Confirm password:", &self.confirm_pw)
            }
            Phase::Result(msg, success) => {
                let color = if *success { Color::Green } else { Color::Red };
                frame.render_widget(
                    Paragraph::new(Span::styled(msg.as_str(), Style::default().fg(color)))
                        .alignment(ratatui::layout::Alignment::Center),
                    centered,
                );
            }
        }

        let hints = match &self.phase {
            Phase::Menu => "Enter=select  Esc=back",
            Phase::InputCurrent | Phase::InputNew | Phase::InputConfirm => {
                "Enter=confirm  Esc=cancel"
            }
            Phase::Result(_, _) => "Esc=back",
        };
        frame.render_widget(
            Paragraph::new(format!(" {hints}"))
                .style(FOOTER_STYLE)
                .alignment(ratatui::layout::Alignment::Center),
            hints_area,
        );
    }

    fn draw_menu(&self, frame: &mut Frame, area: Rect) {
        let actions = self.actions();
        let lines: Vec<Line> = actions
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let marker = if i == self.selection { ">" } else { " " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {marker} {label}"), style))
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn draw_input(&self, frame: &mut Frame, area: Rect, label: &str, value: &str) {
        let [label_area, input_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

        frame.render_widget(
            Paragraph::new(Span::styled(
                label,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            label_area,
        );

        let masked: String = "\u{25cf}".repeat(value.chars().count());
        let cursor_display = format!("{masked}\u{2588}");
        let width = input_area.width as usize;
        let padded = format!("{:<width$}", cursor_display, width = width);
        frame.render_widget(
            Paragraph::new(Span::styled(padded, SELECTED_STYLE)),
            input_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode) -> PasswordAction {
        match &self.phase {
            Phase::Menu => self.handle_menu_key(code),
            Phase::InputCurrent | Phase::InputNew | Phase::InputConfirm => {
                self.handle_input_key(code)
            }
            Phase::Result(_, _) => {
                if code == KeyCode::Esc || code == KeyCode::Enter {
                    self.reset();
                }
                PasswordAction::Continue
            }
        }
    }

    fn handle_menu_key(&mut self, code: KeyCode) -> PasswordAction {
        let actions = self.actions();
        match code {
            KeyCode::Esc => return PasswordAction::Close,
            KeyCode::Up => self.selection = self.selection.saturating_sub(1),
            KeyCode::Down => {
                self.selection = (self.selection + 1).min(actions.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                let action = match actions.get(self.selection) {
                    Some((_, 0)) => Action::Change,
                    Some((_, 1)) => Action::Remove,
                    Some((_, 2)) => Action::Set,
                    _ => return PasswordAction::Continue,
                };
                let first_phase = match action {
                    Action::Set => Phase::InputNew,
                    Action::Change | Action::Remove => Phase::InputCurrent,
                };
                self.chosen_action = Some(action);
                self.phase = first_phase;
            }
            _ => {}
        }
        PasswordAction::Continue
    }

    fn handle_input_key(&mut self, code: KeyCode) -> PasswordAction {
        match code {
            KeyCode::Esc => {
                self.reset();
                return PasswordAction::Continue;
            }
            KeyCode::Enter => {
                match (&self.phase, &self.chosen_action) {
                    (Phase::InputCurrent, Some(Action::Change)) => {
                        self.phase = Phase::InputNew;
                        self.cursor = 0;
                    }
                    (Phase::InputCurrent, Some(Action::Remove)) => {
                        self.execute();
                    }
                    (Phase::InputNew, _) => {
                        self.phase = Phase::InputConfirm;
                        self.cursor = 0;
                    }
                    (Phase::InputConfirm, _) => {
                        self.execute();
                    }
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                self.active_input_mut().push(c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.active_input_mut().pop();
                    self.cursor -= 1;
                }
            }
            _ => {}
        }
        PasswordAction::Continue
    }

    fn reset(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
        self.cursor = 0;
        self.selection = 0;
        self.chosen_action = None;
        self.phase = Phase::Menu;
    }
}

impl Drop for PasswordManager {
    fn drop(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
    }
}

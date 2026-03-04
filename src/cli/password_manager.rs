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
    #[cfg(feature = "totp")]
    TotpEnable,
    #[cfg(feature = "totp")]
    TotpDisable,
}

enum Phase {
    Menu,
    InputCurrent,
    InputNew,
    InputConfirm,
    Result(String, bool),
    #[cfg(feature = "totp")]
    TotpDisplay(String), // base32 secret displayed to user
    #[cfg(feature = "totp")]
    TotpVerify, // user enters code to verify
    #[cfg(feature = "totp")]
    TotpConfirmDisable, // user enters code to confirm disable
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
    #[cfg(feature = "totp")]
    totp_enabled: bool,
    #[cfg(feature = "totp")]
    totp_secret: String,
    #[cfg(feature = "totp")]
    totp_code: String,
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
            #[cfg(feature = "totp")]
            totp_enabled: {
                let check_path = get_data_dir().join("nigel.db");
                if encrypted {
                    crate::db::get_connection(&check_path)
                        .ok()
                        .map(|conn| crate::totp::is_enabled(&conn))
                        .unwrap_or(false)
                } else {
                    false
                }
            },
            #[cfg(feature = "totp")]
            totp_secret: String::new(),
            #[cfg(feature = "totp")]
            totp_code: String::new(),
        })
    }

    fn actions(&self) -> Vec<(&str, usize)> {
        let mut items = Vec::new();
        if self.encrypted {
            items.push(("Change password", 0));
            items.push(("Remove password", 1));
            #[cfg(feature = "totp")]
            if self.totp_enabled {
                items.push(("Disable 2FA", 4));
            } else {
                items.push(("Enable 2FA", 3));
            }
        } else {
            items.push(("Set password", 2));
        }
        items
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
            #[cfg(feature = "totp")]
            Some(Action::TotpEnable) | Some(Action::TotpDisable) => {
                return; // TOTP actions are handled in handle_totp_key
            }
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
        #[cfg(feature = "totp")]
        {
            if let Ok(conn) = db::open_connection(db_path, Some(self.current_pw.trim())) {
                if crate::totp::is_enabled(&conn) {
                    let _ = crate::totp::disable(&conn, db_path);
                }
            }
        }
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
            Phase::InputNew => self.draw_input(frame, centered, "New password:", &self.new_pw),
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
            #[cfg(feature = "totp")]
            Phase::TotpDisplay(ref secret) => {
                self.draw_totp_display(frame, centered, secret);
            }
            #[cfg(feature = "totp")]
            Phase::TotpVerify => {
                self.draw_totp_code_input(frame, centered, "Enter code to verify:");
            }
            #[cfg(feature = "totp")]
            Phase::TotpConfirmDisable => {
                self.draw_totp_code_input(
                    frame,
                    centered,
                    "Enter authenticator code to disable 2FA:",
                );
            }
        }

        let hints = match &self.phase {
            Phase::Menu => "Enter=select  Esc=back",
            Phase::InputCurrent | Phase::InputNew | Phase::InputConfirm => {
                "Enter=confirm  Esc=cancel"
            }
            Phase::Result(_, _) => "Esc=back",
            #[cfg(feature = "totp")]
            Phase::TotpDisplay(_) => "Enter=continue  Esc=cancel",
            #[cfg(feature = "totp")]
            Phase::TotpVerify | Phase::TotpConfirmDisable => "Enter=confirm  Esc=cancel",
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

    #[cfg(feature = "totp")]
    fn draw_totp_display(&self, frame: &mut Frame, area: Rect, secret: &str) {
        let [label_area, _gap, secret_area, _gap2, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(Span::styled(
                "Add this secret to your authenticator app:",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            label_area,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("  {secret}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            secret_area,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Press Enter to continue",
                Style::default().fg(Color::DarkGray),
            )),
            hint_area,
        );
    }

    #[cfg(feature = "totp")]
    fn draw_totp_code_input(&self, frame: &mut Frame, area: Rect, label: &str) {
        let [label_area, input_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

        frame.render_widget(
            Paragraph::new(Span::styled(
                label,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            label_area,
        );

        let cursor_display = format!("{}\u{2588}", self.totp_code);
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
            #[cfg(feature = "totp")]
            Phase::TotpDisplay(_) | Phase::TotpVerify | Phase::TotpConfirmDisable => {
                self.handle_totp_key(code)
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
                    #[cfg(feature = "totp")]
                    Some((_, 3)) => Action::TotpEnable,
                    #[cfg(feature = "totp")]
                    Some((_, 4)) => Action::TotpDisable,
                    _ => return PasswordAction::Continue,
                };
                let first_phase = match action {
                    Action::Set => Phase::InputNew,
                    Action::Change | Action::Remove => Phase::InputCurrent,
                    #[cfg(feature = "totp")]
                    Action::TotpEnable => {
                        let company = {
                            let db_path = get_data_dir().join("nigel.db");
                            crate::db::get_connection(&db_path)
                                .ok()
                                .and_then(|conn| crate::db::get_metadata(&conn, "company_name"))
                                .unwrap_or_else(|| "Nigel".to_string())
                        };
                        match crate::totp::generate_secret(&company, "database") {
                            Ok((base32, _)) => {
                                self.totp_secret = base32.clone();
                                Phase::TotpDisplay(base32)
                            }
                            Err(e) => {
                                Phase::Result(format!("Failed to generate secret: {e}"), false)
                            }
                        }
                    }
                    #[cfg(feature = "totp")]
                    Action::TotpDisable => Phase::TotpConfirmDisable,
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
            KeyCode::Enter => match (&self.phase, &self.chosen_action) {
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
            },
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

    #[cfg(feature = "totp")]
    fn handle_totp_key(&mut self, code: KeyCode) -> PasswordAction {
        match code {
            KeyCode::Esc => {
                self.reset();
            }
            KeyCode::Enter => match &self.phase {
                Phase::TotpDisplay(_) => {
                    self.totp_code.clear();
                    self.phase = Phase::TotpVerify;
                }
                Phase::TotpVerify => {
                    if crate::totp::verify_code(&self.totp_secret, self.totp_code.trim()) {
                        let db_path = get_data_dir().join("nigel.db");
                        match crate::db::get_connection(&db_path) {
                            Ok(conn) => {
                                match crate::totp::enable(&conn, &db_path, &self.totp_secret) {
                                    Ok(()) => {
                                        self.totp_enabled = true;
                                        self.phase = Phase::Result(
                                            "Two-factor authentication enabled.".into(),
                                            true,
                                        );
                                    }
                                    Err(e) => {
                                        self.phase = Phase::Result(
                                            format!("Failed to enable 2FA: {e}"),
                                            false,
                                        )
                                    }
                                }
                            }
                            Err(e) => {
                                self.phase = Phase::Result(format!("DB error: {e}"), false)
                            }
                        }
                    } else {
                        self.phase =
                            Phase::Result("Invalid code. 2FA was not enabled.".into(), false);
                    }
                    self.totp_code.clear();
                }
                Phase::TotpConfirmDisable => {
                    let db_path = get_data_dir().join("nigel.db");
                    let secret = match crate::totp::get_secret(&db_path) {
                        Ok(s) => s,
                        Err(e) => {
                            self.phase = Phase::Result(format!("Keychain error: {e}"), false);
                            return PasswordAction::Continue;
                        }
                    };
                    if crate::totp::verify_code(&secret, self.totp_code.trim()) {
                        match crate::db::get_connection(&db_path) {
                            Ok(conn) => match crate::totp::disable(&conn, &db_path) {
                                Ok(()) => {
                                    self.totp_enabled = false;
                                    self.phase = Phase::Result(
                                        "Two-factor authentication disabled.".into(),
                                        true,
                                    );
                                }
                                Err(e) => {
                                    self.phase = Phase::Result(
                                        format!("Failed to disable 2FA: {e}"),
                                        false,
                                    )
                                }
                            },
                            Err(e) => {
                                self.phase = Phase::Result(format!("DB error: {e}"), false)
                            }
                        }
                    } else {
                        self.phase =
                            Phase::Result("Invalid code. 2FA was not disabled.".into(), false);
                    }
                    self.totp_code.clear();
                }
                _ => {}
            },
            KeyCode::Char(c) => {
                if matches!(self.phase, Phase::TotpVerify | Phase::TotpConfirmDisable)
                    && c.is_ascii_digit()
                    && self.totp_code.len() < 6
                {
                    self.totp_code.push(c);
                }
            }
            KeyCode::Backspace => {
                if matches!(self.phase, Phase::TotpVerify | Phase::TotpConfirmDisable) {
                    self.totp_code.pop();
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
        #[cfg(feature = "totp")]
        {
            self.totp_secret.zeroize();
            self.totp_code.zeroize();
        }
    }
}

impl Drop for PasswordManager {
    fn drop(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
        #[cfg(feature = "totp")]
        {
            self.totp_secret.zeroize();
            self.totp_code.zeroize();
        }
    }
}

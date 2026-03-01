use std::path::PathBuf;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::categorizer::categorize_transactions;
use crate::importer::import_file;
use crate::settings::get_data_dir;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

pub enum ImportAction {
    Continue,
    Close,
}

enum Screen {
    Form,
    Result(ImportResult),
}

struct ImportResult {
    message: String,
    is_error: bool,
}

const FIELD_FILE: usize = 0;
const FIELD_ACCOUNT: usize = 1;

pub struct ImportScreen {
    accounts: Vec<String>,
    account_idx: usize,
    file_path: String,
    focused: usize,
    screen: Screen,
    status_message: Option<String>,
    greeting: String,
}

impl ImportScreen {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        let accounts = load_account_names(conn);
        Self {
            accounts,
            account_idx: 0,
            file_path: String::new(),
            focused: FIELD_FILE,
            screen: Screen::Form,
            status_message: None,
            greeting: greeting.to_string(),
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        let [header_area, sep, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(format!(" {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        let sep_line = "\u{2501}".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        match &self.screen {
            Screen::Form => self.draw_form(frame, content_area, hints_area),
            Screen::Result(result) => self.draw_result(frame, content_area, hints_area, result),
        }
    }

    fn draw_form(
        &self,
        frame: &mut Frame,
        content_area: ratatui::layout::Rect,
        hints_area: ratatui::layout::Rect,
    ) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Import a Statement",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.accounts.is_empty() {
            lines.push(Line::from("   No accounts found. Add one first."));
        } else {
            // File path input
            let is_focused = self.focused == FIELD_FILE;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let cursor = if is_focused { "_" } else { "" };
            lines.push(Line::from(vec![
                Span::styled("   File path      ", label_style),
                Span::styled(
                    format!("{}{}", self.file_path, cursor),
                    if is_focused {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ),
            ]));

            // Account selector
            let account_name = &self.accounts[self.account_idx];
            let is_focused = self.focused == FIELD_ACCOUNT;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let arrows = if is_focused {
                ("< ", " >")
            } else {
                ("  ", "  ")
            };
            lines.push(Line::from(vec![
                Span::styled("   Account        ", label_style),
                Span::styled(
                    format!("{}{}{}", arrows.0, account_name, arrows.1),
                    if is_focused {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ),
            ]));
        }

        if let Some(msg) = &self.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Tab/Up/Down=fields  Left/Right=account  Enter=import  Esc=back")
                .style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn draw_result(
        &self,
        frame: &mut Frame,
        content_area: ratatui::layout::Rect,
        hints_area: ratatui::layout::Rect,
        result: &ImportResult,
    ) {
        let color = if result.is_error {
            Color::Red
        } else {
            Color::Green
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Import Result",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for line in result.message.lines() {
            lines.push(Line::from(Span::styled(
                format!("   {line}"),
                Style::default().fg(color),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Esc=back to dashboard").style(FOOTER_STYLE),
            hints_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> ImportAction {
        match &self.screen {
            Screen::Form => self.handle_form_key(code, conn),
            Screen::Result(_) => match code {
                KeyCode::Esc | KeyCode::Char('q') => ImportAction::Close,
                _ => ImportAction::Continue,
            },
        }
    }

    fn handle_form_key(&mut self, code: KeyCode, conn: &Connection) -> ImportAction {
        match code {
            KeyCode::Esc => return ImportAction::Close,
            KeyCode::Tab | KeyCode::Down => {
                if !self.accounts.is_empty() {
                    self.focused = (self.focused + 1) % 2;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if !self.accounts.is_empty() {
                    self.focused = if self.focused == 0 { 1 } else { 0 };
                }
            }
            KeyCode::Left => {
                if self.focused == FIELD_ACCOUNT && !self.accounts.is_empty() {
                    self.account_idx = if self.account_idx == 0 {
                        self.accounts.len() - 1
                    } else {
                        self.account_idx - 1
                    };
                }
            }
            KeyCode::Right => {
                if self.focused == FIELD_ACCOUNT && !self.accounts.is_empty() {
                    self.account_idx = (self.account_idx + 1) % self.accounts.len();
                }
            }
            KeyCode::Char(c) => {
                if self.focused == FIELD_FILE {
                    self.file_path.push(c);
                }
                self.status_message = None;
            }
            KeyCode::Backspace => {
                if self.focused == FIELD_FILE {
                    self.file_path.pop();
                }
                self.status_message = None;
            }
            KeyCode::Enter => {
                if self.accounts.is_empty() {
                    return ImportAction::Continue;
                }
                let path_str = self.file_path.trim().to_string();
                if path_str.is_empty() {
                    self.status_message = Some("File path is required".into());
                    return ImportAction::Continue;
                }

                let file_path = PathBuf::from(shellexpand(&path_str));
                if !file_path.exists() {
                    self.status_message = Some(format!("File not found: {}", file_path.display()));
                    return ImportAction::Continue;
                }

                let account_name = self.accounts[self.account_idx].clone();
                self.screen = Screen::Result(run_import(conn, &file_path, &account_name));
            }
            _ => {}
        }
        ImportAction::Continue
    }
}

fn run_import(conn: &Connection, file_path: &PathBuf, account_name: &str) -> ImportResult {
    // Snapshot before import
    let data_dir = get_data_dir();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
    if let Err(e) = crate::cli::backup::snapshot(conn, &snap_path) {
        return ImportResult {
            message: format!("Snapshot failed: {e}"),
            is_error: true,
        };
    }

    match import_file(conn, file_path, account_name, None) {
        Err(e) => ImportResult {
            message: format!("Import failed: {e}"),
            is_error: true,
        },
        Ok(result) => {
            if result.duplicate_file {
                return ImportResult {
                    message: "This file has already been imported (duplicate checksum).".into(),
                    is_error: false,
                };
            }

            let mut msg = format!(
                "{} imported, {} skipped (duplicates)",
                result.imported, result.skipped
            );

            match categorize_transactions(conn) {
                Ok(cat) => {
                    msg.push_str(&format!(
                        "\n{} categorized, {} still flagged",
                        cat.categorized, cat.still_flagged
                    ));
                }
                Err(e) => {
                    msg.push_str(&format!("\nCategorization error: {e}"));
                }
            }

            ImportResult {
                message: msg,
                is_error: false,
            }
        }
    }
}

fn load_account_names(conn: &Connection) -> Vec<String> {
    let mut stmt = match conn.prepare("SELECT name FROM accounts ORDER BY name") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| row.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Expand `~` to home directory.
fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

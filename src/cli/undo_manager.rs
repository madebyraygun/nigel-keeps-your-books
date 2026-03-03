use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::undo::{delete_import, get_last_import, LastImport};
use crate::error::Result;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

pub enum UndoAction {
    Continue,
    Close,
}

enum Phase {
    Confirm,
    Result(String, bool), // (message, is_success)
}

pub struct UndoScreen {
    greeting: String,
    last_import: Option<LastImport>,
    phase: Phase,
}

impl UndoScreen {
    pub fn new(conn: &Connection, greeting: &str) -> Result<Self> {
        let last_import = get_last_import(conn)?;
        Ok(Self {
            greeting: greeting.to_string(),
            last_import,
            phase: Phase::Confirm,
        })
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

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Undo Last Import",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        match &self.phase {
            Phase::Confirm => {
                if let Some(import) = &self.last_import {
                    lines.push(Line::from(format!("   File:         {}", import.filename)));
                    lines.push(Line::from(format!(
                        "   Account:      {}",
                        import.account_name
                    )));
                    lines.push(Line::from(format!(
                        "   Imported:     {}",
                        import.import_date
                    )));
                    lines.push(Line::from(format!(
                        "   Transactions: {}",
                        import.transaction_count
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "   Press Enter to undo, Esc to cancel",
                        Style::default().fg(Color::Yellow),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        "   No imports to undo.",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            Phase::Result(msg, is_success) => {
                let color = if *is_success {
                    Color::Green
                } else {
                    Color::Red
                };
                lines.push(Line::from(Span::styled(
                    format!("   {msg}"),
                    Style::default().fg(color),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        let hints = match &self.phase {
            Phase::Confirm if self.last_import.is_some() => " Enter=undo  Esc=back",
            _ => " Esc=back",
        };
        frame.render_widget(Paragraph::new(hints).style(FOOTER_STYLE), hints_area);
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> UndoAction {
        match &self.phase {
            Phase::Confirm => match code {
                KeyCode::Esc => UndoAction::Close,
                KeyCode::Enter => {
                    if let Some(import) = &self.last_import {
                        let import_id = import.import_id;
                        let filename = import.filename.clone();
                        match delete_import(conn, import_id) {
                            Ok(deleted) => {
                                self.phase = Phase::Result(
                                    format!(
                                        "Rolled back import of \"{filename}\" ({deleted} transactions removed)"
                                    ),
                                    true,
                                );
                                self.last_import = None;
                            }
                            Err(e) => {
                                self.phase = Phase::Result(format!("Error: {e}"), false);
                            }
                        }
                    }
                    UndoAction::Continue
                }
                _ => UndoAction::Continue,
            },
            Phase::Result(_, _) => match code {
                KeyCode::Esc | KeyCode::Enter => UndoAction::Close,
                _ => UndoAction::Continue,
            },
        }
    }
}

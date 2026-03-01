use std::path::PathBuf;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::settings::{get_data_dir, load_settings, save_settings, shellexpand_path};
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

pub enum LoadAction {
    Continue,
    Close,
    /// Data directory was changed; dashboard must reload with new connection.
    Reload,
}

pub struct LoadScreen {
    current_dir: String,
    path: String,
    status_message: Option<(String, bool)>, // (message, is_error)
    greeting: String,
    done: bool,
}

impl LoadScreen {
    pub fn new(greeting: &str) -> Self {
        let current = get_data_dir();
        Self {
            current_dir: current.to_string_lossy().to_string(),
            path: String::new(),
            status_message: None,
            greeting: greeting.to_string(),
            done: false,
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

        let cursor = if self.done { "" } else { "_" };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Load a Different Data File",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("   Current:  {}", self.current_dir)),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "   New path  ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}{}", self.path, cursor),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
        ];

        if let Some((msg, is_error)) = &self.status_message {
            let color = if *is_error { Color::Red } else { Color::Green };
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(color),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        let hints = if self.done {
            " Esc=back to dashboard"
        } else {
            " Enter=load  Esc=back"
        };
        frame.render_widget(Paragraph::new(hints).style(FOOTER_STYLE), hints_area);
    }

    pub fn handle_key(&mut self, code: KeyCode) -> LoadAction {
        if self.done {
            return match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => LoadAction::Reload,
                _ => LoadAction::Continue,
            };
        }

        match code {
            KeyCode::Esc => LoadAction::Close,
            KeyCode::Char(c) => {
                self.path.push(c);
                self.status_message = None;
                LoadAction::Continue
            }
            KeyCode::Backspace => {
                self.path.pop();
                self.status_message = None;
                LoadAction::Continue
            }
            KeyCode::Enter => {
                let trimmed = self.path.trim().to_string();
                if trimmed.is_empty() {
                    self.status_message = Some(("Path is required".into(), true));
                    return LoadAction::Continue;
                }

                let resolved = PathBuf::from(shellexpand_path(&trimmed));
                let db_path = resolved.join("nigel.db");

                if !db_path.exists() {
                    self.status_message = Some((
                        format!("No database found at {}", db_path.display()),
                        true,
                    ));
                    return LoadAction::Continue;
                }

                let mut settings = load_settings();
                settings.data_dir = resolved.to_string_lossy().to_string();
                match save_settings(&settings) {
                    Ok(()) => {
                        self.status_message = Some((
                            format!("Switched to {}", resolved.display()),
                            false,
                        ));
                        self.done = true;
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Error: {e}"), true));
                    }
                }
                LoadAction::Continue
            }
            _ => LoadAction::Continue,
        }
    }
}

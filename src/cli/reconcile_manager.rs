use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::accounts;
use crate::fmt::money;
use crate::reconciler;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

pub enum ReconcileAction {
    Continue,
    Close,
}

enum Screen {
    Form,
    Result(ReconcileResult),
}

struct ReconcileResult {
    is_reconciled: bool,
    statement_balance: f64,
    calculated_balance: f64,
    discrepancy: f64,
}

const FIELD_ACCOUNT: usize = 0;
const FIELD_MONTH: usize = 1;
const FIELD_BALANCE: usize = 2;

pub struct ReconcileScreen {
    accounts: Vec<String>,
    account_idx: usize,
    month: String,
    balance: String,
    focused: usize,
    screen: Screen,
    status_message: Option<String>,
    greeting: String,
}

impl ReconcileScreen {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        let accounts = accounts::account_names(conn);
        Self {
            accounts,
            account_idx: 0,
            month: String::new(),
            balance: String::new(),
            focused: FIELD_ACCOUNT,
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

    fn draw_form(&self, frame: &mut Frame, content_area: ratatui::layout::Rect, hints_area: ratatui::layout::Rect) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Reconcile an Account",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.accounts.is_empty() {
            lines.push(Line::from("   No accounts found. Add one first."));
        } else {
            // Account selector
            let account_name = &self.accounts[self.account_idx];
            let is_focused = self.focused == FIELD_ACCOUNT;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let arrows = if is_focused { ("< ", " >") } else { ("  ", "  ") };
            lines.push(Line::from(vec![
                Span::styled("   Account        ", label_style),
                Span::styled(
                    format!("{}{}{}", arrows.0, account_name, arrows.1),
                    if is_focused { Style::default().fg(Color::Cyan) } else { Style::default() },
                ),
            ]));

            // Month input
            let is_focused = self.focused == FIELD_MONTH;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let cursor = if is_focused { "_" } else { "" };
            let placeholder = if self.month.is_empty() && !is_focused { "YYYY-MM" } else { "" };
            lines.push(Line::from(vec![
                Span::styled("   Month          ", label_style),
                Span::styled(
                    format!("{}{}{}", self.month, cursor, placeholder),
                    if is_focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) },
                ),
            ]));

            // Balance input
            let is_focused = self.focused == FIELD_BALANCE;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let cursor = if is_focused { "_" } else { "" };
            lines.push(Line::from(vec![
                Span::styled("   Balance      $ ", label_style),
                Span::styled(
                    format!("{}{}", self.balance, cursor),
                    if is_focused { Style::default().fg(Color::Cyan) } else { Style::default() },
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
            Paragraph::new(" Tab/Up/Down=fields  Left/Right=account  Enter=reconcile  Esc=back")
                .style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn draw_result(
        &self,
        frame: &mut Frame,
        content_area: ratatui::layout::Rect,
        hints_area: ratatui::layout::Rect,
        result: &ReconcileResult,
    ) {
        let account_name = self.accounts.get(self.account_idx).map(|s| s.as_str()).unwrap_or("?");

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Reconciliation Result",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("   Account:     {account_name}")),
            Line::from(format!("   Month:       {}", self.month)),
            Line::from(""),
        ];

        if result.is_reconciled {
            lines.push(Line::from(Span::styled(
                format!("   Reconciled! Calculated: {}", money(result.calculated_balance)),
                Style::default().fg(Color::Green),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "   DISCREPANCY",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(format!("   Statement:   {}", money(result.statement_balance))));
            lines.push(Line::from(format!("   Calculated:  {}", money(result.calculated_balance))));
            lines.push(Line::from(Span::styled(
                format!("   Difference:  {}", money(result.discrepancy)),
                Style::default().fg(Color::Red),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Esc=back to dashboard").style(FOOTER_STYLE),
            hints_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> ReconcileAction {
        match &self.screen {
            Screen::Form => self.handle_form_key(code, conn),
            Screen::Result(_) => {
                match code {
                    KeyCode::Esc => ReconcileAction::Close,
                    _ => ReconcileAction::Continue,
                }
            }
        }
    }

    fn handle_form_key(&mut self, code: KeyCode, conn: &Connection) -> ReconcileAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return ReconcileAction::Close,
            KeyCode::Tab | KeyCode::Down => {
                if !self.accounts.is_empty() {
                    self.focused = (self.focused + 1) % 3;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if !self.accounts.is_empty() {
                    self.focused = if self.focused == 0 { 2 } else { self.focused - 1 };
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
                match self.focused {
                    FIELD_MONTH => {
                        if c.is_ascii_digit() || c == '-' {
                            self.month.push(c);
                        }
                    }
                    FIELD_BALANCE => {
                        if c.is_ascii_digit() || c == '.' || c == '-' || c == ',' {
                            self.balance.push(c);
                        }
                    }
                    _ => {}
                }
                self.status_message = None;
            }
            KeyCode::Backspace => {
                match self.focused {
                    FIELD_MONTH => { self.month.pop(); }
                    FIELD_BALANCE => { self.balance.pop(); }
                    _ => {}
                }
                self.status_message = None;
            }
            KeyCode::Enter => {
                if self.accounts.is_empty() {
                    return ReconcileAction::Continue;
                }
                if self.month.is_empty() {
                    self.status_message = Some("Month is required (YYYY-MM)".into());
                    return ReconcileAction::Continue;
                }
                if self.balance.is_empty() {
                    self.status_message = Some("Balance is required".into());
                    return ReconcileAction::Continue;
                }
                let balance: f64 = match self.balance.replace(',', "").parse() {
                    Ok(b) => b,
                    Err(_) => {
                        self.status_message = Some("Invalid balance amount".into());
                        return ReconcileAction::Continue;
                    }
                };
                let account_name = &self.accounts[self.account_idx];
                match reconciler::reconcile(conn, account_name, &self.month, balance) {
                    Ok(r) => {
                        self.screen = Screen::Result(ReconcileResult {
                            is_reconciled: r.is_reconciled,
                            statement_balance: r.statement_balance,
                            calculated_balance: r.calculated_balance,
                            discrepancy: r.discrepancy,
                        });
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Error: {e}"));
                    }
                }
            }
            _ => {}
        }
        ReconcileAction::Continue
    }
}

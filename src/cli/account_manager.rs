use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::accounts;
use crate::models::Account;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

const ACCOUNT_TYPES: &[&str] = &["checking", "credit_card", "line_of_credit", "payroll"];

// Field indices for AccountForm::new_add() — keep in sync with field order
const NAME_IDX: usize = 0;
const TYPE_IDX: usize = 1;
const INST_IDX: usize = 2;
const LAST_IDX: usize = 3;

pub enum AccountAction {
    Continue,
    Close,
}

enum Screen {
    List,
    Add(AccountForm),
    Rename(AccountForm),
    ConfirmDelete,
}

struct AccountForm {
    fields: Vec<FormField>,
    focused: usize,
}

struct FormField {
    label: &'static str,
    value: String,
    kind: FieldKind,
}

enum FieldKind {
    Text,
    Selector { options: Vec<String>, selected: usize },
}

impl AccountForm {
    fn new_add() -> Self {
        Self {
            fields: vec![
                FormField {
                    label: "Name",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Type",
                    value: ACCOUNT_TYPES[0].to_string(),
                    kind: FieldKind::Selector {
                        options: ACCOUNT_TYPES.iter().map(|s| s.to_string()).collect(),
                        selected: 0,
                    },
                },
                FormField {
                    label: "Institution",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Last Four",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
            ],
            focused: 0,
        }
    }

    fn new_rename(current_name: &str) -> Self {
        Self {
            fields: vec![FormField {
                label: "Name",
                value: current_name.to_string(),
                kind: FieldKind::Text,
            }],
            focused: 0,
        }
    }

}

pub struct AccountManager {
    accounts: Vec<Account>,
    selection: usize,
    screen: Screen,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl AccountManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        let accounts = accounts::list_accounts(conn).unwrap_or_default();
        Self {
            accounts,
            selection: 0,
            screen: Screen::List,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload(&mut self, conn: &Connection) {
        self.accounts = accounts::list_accounts(conn).unwrap_or_default();
        if !self.accounts.is_empty() {
            self.selection = self.selection.min(self.accounts.len() - 1);
        } else {
            self.selection = 0;
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::List | Screen::ConfirmDelete => self.draw_list(frame),
            Screen::Add(form) => self.draw_form(frame, "Add Account", form),
            Screen::Rename(form) => self.draw_form(frame, "Rename Account", form),
        }
    }

    fn draw_list(&self, frame: &mut Frame) {
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

        let sep_line = "━".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        // Content: title + table
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Accounts",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.accounts.is_empty() {
            lines.push(Line::from("   No accounts yet. Press 'a' to add one."));
        } else {
            // Table header
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    format!(
                        "{:<24} {:<18} {:<20} {}",
                        "Name", "Type", "Institution", "Last Four"
                    ),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            for (i, account) in self.accounts.iter().enumerate() {
                let marker = if i == self.selection { " > " } else { "   " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let inst = account.institution.as_deref().unwrap_or("");
                let last = account.last_four.as_deref().unwrap_or("");
                lines.push(Line::from(Span::styled(
                    format!(
                        "{marker}{:<24} {:<18} {:<20} {}",
                        account.name, account.account_type, inst, last
                    ),
                    style,
                )));
            }
        }

        // Delete confirmation inline
        if let Screen::ConfirmDelete = &self.screen {
            if let Some(account) = self.accounts.get(self.selection) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("   Delete '{}'? (y/n)", account.name),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        // Hints / status
        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else if let Screen::ConfirmDelete = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=confirm  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(" a=add  r=rename  d=delete  Esc=back  q=quit")
                    .style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    fn draw_form(&self, frame: &mut Frame, title: &str, form: &AccountForm) {
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
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        let sep_line = "━".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" {title}"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (i, field) in form.fields.iter().enumerate() {
            let is_focused = i == form.focused;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            match &field.kind {
                FieldKind::Text => {
                    let cursor = if is_focused { "_" } else { "" };
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {:<14} ", field.label), label_style),
                        Span::styled(
                            format!("{}{cursor}", field.value),
                            if is_focused {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
                FieldKind::Selector { options, selected } => {
                    let arrows = if is_focused { ("< ", " >") } else { ("  ", "  ") };
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {:<14} ", field.label), label_style),
                        Span::styled(
                            format!("{}{}{}", arrows.0, options[*selected], arrows.1),
                            if is_focused {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
            }
        }

        // Status message below form
        if let Some(msg) = &self.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Tab=next field  Enter=save  Esc=cancel").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_ttl = 3;
    }

    pub fn handle_key(
        &mut self,
        code: crossterm::event::KeyCode,
        conn: &Connection,
    ) -> AccountAction {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        match &mut self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::Add(_) => self.handle_form_key(code, conn, FormMode::Add),
            Screen::Rename(_) => self.handle_form_key(code, conn, FormMode::Rename),
            Screen::ConfirmDelete => self.handle_delete_key(code, conn),
        }
    }

    fn handle_list_key(
        &mut self,
        code: crossterm::event::KeyCode,
        conn: &Connection,
    ) -> AccountAction {
        use crossterm::event::KeyCode::*;
        match code {
            Up => {
                self.selection = self.selection.saturating_sub(1);
            }
            Down => {
                if !self.accounts.is_empty() {
                    self.selection = (self.selection + 1).min(self.accounts.len() - 1);
                }
            }
            Char('a') => {
                self.screen = Screen::Add(AccountForm::new_add());
            }
            Char('r') => {
                if let Some(account) = self.accounts.get(self.selection) {
                    self.screen = Screen::Rename(AccountForm::new_rename(&account.name));
                }
            }
            Char('d') => {
                if !self.accounts.is_empty() {
                    // Check transaction count first to give immediate feedback
                    if let Some(account) = self.accounts.get(self.selection) {
                        match accounts::transaction_count(conn, account.id) {
                            Ok(count) if count > 0 => {
                                self.set_status(format!(
                                    "Cannot delete: account has {count} transactions"
                                ));
                            }
                            Ok(_) => {
                                self.screen = Screen::ConfirmDelete;
                            }
                            Err(e) => {
                                self.set_status(format!("Error: {e}"));
                            }
                        }
                    }
                }
            }
            Char('q') | Esc => return AccountAction::Close,
            _ => {}
        }
        AccountAction::Continue
    }

    fn handle_form_key(
        &mut self,
        code: crossterm::event::KeyCode,
        conn: &Connection,
        mode: FormMode,
    ) -> AccountAction {
        use crossterm::event::KeyCode::*;

        // We need to temporarily take the screen to get mutable access to the form
        let form = match &mut self.screen {
            Screen::Add(f) | Screen::Rename(f) => f,
            _ => return AccountAction::Continue,
        };

        match code {
            Esc => {
                self.screen = Screen::List;
                return AccountAction::Continue;
            }
            Tab | Down => {
                form.focused = (form.focused + 1) % form.fields.len();
            }
            BackTab | Up => {
                form.focused = if form.focused == 0 {
                    form.fields.len() - 1
                } else {
                    form.focused - 1
                };
            }
            Left => {
                if let FieldKind::Selector { options, selected } = &mut form.fields[form.focused].kind {
                    *selected = if *selected == 0 {
                        options.len() - 1
                    } else {
                        *selected - 1
                    };
                    form.fields[form.focused].value = options[*selected].clone();
                }
            }
            Right => {
                if let FieldKind::Selector { options, selected } = &mut form.fields[form.focused].kind {
                    *selected = (*selected + 1) % options.len();
                    form.fields[form.focused].value = options[*selected].clone();
                }
            }
            Char(c) => {
                if let FieldKind::Text = &form.fields[form.focused].kind {
                    form.fields[form.focused].value.push(c);
                }
            }
            Backspace => {
                if let FieldKind::Text = &form.fields[form.focused].kind {
                    form.fields[form.focused].value.pop();
                }
            }
            Enter => {
                match mode {
                    FormMode::Add => {
                        let name = form.fields[NAME_IDX].value.trim().to_string();
                        if name.is_empty() {
                            self.set_status("Name is required".into());
                            return AccountAction::Continue;
                        }
                        let acct_type = form.fields[TYPE_IDX].value.clone();
                        let institution = {
                            let v = form.fields[INST_IDX].value.trim().to_string();
                            if v.is_empty() { None } else { Some(v) }
                        };
                        let last_four = {
                            let v = form.fields[LAST_IDX].value.trim().to_string();
                            if v.is_empty() {
                                None
                            } else if !v.chars().all(|c| c.is_ascii_digit()) || v.len() != 4 {
                                self.set_status("Last four must be exactly 4 digits".into());
                                return AccountAction::Continue;
                            } else {
                                Some(v)
                            }
                        };
                        match accounts::add_account(
                            conn,
                            &name,
                            &acct_type,
                            institution.as_deref(),
                            last_four.as_deref(),
                        ) {
                            Ok(()) => {
                                self.reload(conn);
                                self.screen = Screen::List;
                                self.set_status(format!("Added account: {name}"));
                            }
                            Err(e) => {
                                self.set_status(e.to_string());
                            }
                        }
                    }
                    FormMode::Rename => {
                        let new_name = form.fields[NAME_IDX].value.trim().to_string();
                        if let Some(account) = self.accounts.get(self.selection) {
                            match accounts::rename_account(conn, account.id, &new_name) {
                                Ok(()) => {
                                    self.reload(conn);
                                    self.screen = Screen::List;
                                    self.set_status(format!("Renamed to: {new_name}"));
                                }
                                Err(e) => {
                                    self.set_status(e.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        AccountAction::Continue
    }

    fn handle_delete_key(
        &mut self,
        code: crossterm::event::KeyCode,
        conn: &Connection,
    ) -> AccountAction {
        use crossterm::event::KeyCode::*;
        match code {
            Char('y') => {
                if let Some(account) = self.accounts.get(self.selection) {
                    let name = account.name.clone();
                    match accounts::delete_account(conn, account.id) {
                        Ok(()) => {
                            self.reload(conn);
                            self.screen = Screen::List;
                            self.set_status(format!("Deleted account: {name}"));
                        }
                        Err(e) => {
                            self.screen = Screen::List;
                            self.set_status(e.to_string());
                        }
                    }
                }
            }
            Char('n') | Esc => {
                self.screen = Screen::List;
            }
            _ => {}
        }
        AccountAction::Continue
    }
}

enum FormMode {
    Add,
    Rename,
}

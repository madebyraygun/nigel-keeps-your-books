use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::tui::{FOOTER_STYLE, HEADER_STYLE};

pub enum RulesAction {
    Continue,
    Close,
}

struct RuleRow {
    id: i64,
    pattern: String,
    match_type: String,
    vendor: String,
    category: String,
    priority: i64,
    hits: i64,
}

enum Screen {
    List,
    ConfirmDelete,
}

pub struct RulesManager {
    rules: Vec<RuleRow>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    status_ttl: u8,
    greeting: String,
}

impl RulesManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        let rules = load_rules(conn);
        Self {
            rules,
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload(&mut self, conn: &Connection) {
        self.rules = load_rules(conn);
        if !self.rules.is_empty() {
            self.selection = self.selection.min(self.rules.len() - 1);
        } else {
            self.selection = 0;
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
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

        // Content
        let visible_height = content_area.height as usize;
        // 3 lines for title area + 1 for column header = 4 lines overhead
        let data_rows = visible_height.saturating_sub(4);
        self.last_visible_rows = data_rows;

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Categorization Rules ({})", self.rules.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.rules.is_empty() {
            lines.push(Line::from("   No rules defined yet."));
        } else {
            // Column header
            lines.push(Line::from(Span::styled(
                format!(
                    "   {:<5} {:<24} {:<10} {:<16} {:<24} {:<5} {}",
                    "ID", "Pattern", "Type", "Vendor", "Category", "Pri", "Hits"
                ),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));

            let end = (self.scroll_offset + data_rows).min(self.rules.len());
            for i in self.scroll_offset..end {
                let rule = &self.rules[i];
                let marker = if i == self.selection { " > " } else { "   " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let pattern_display = truncate(&rule.pattern, 22);
                let vendor_display = truncate(&rule.vendor, 14);
                let category_display = truncate(&rule.category, 22);

                lines.push(Line::from(Span::styled(
                    format!(
                        "{marker}{:<5} {:<24} {:<10} {:<16} {:<24} {:<5} {}",
                        rule.id,
                        pattern_display,
                        rule.match_type,
                        vendor_display,
                        category_display,
                        rule.priority,
                        rule.hits
                    ),
                    style,
                )));
            }
        }

        // Delete confirmation
        if let Screen::ConfirmDelete = &self.screen {
            if let Some(rule) = self.rules.get(self.selection) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!(
                        "   Delete rule {}? '{}' \u{2192} {} (y/n)",
                        rule.id, rule.pattern, rule.category
                    ),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        // Hints
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
                Paragraph::new(" Up/Down=navigate  d=delete  Esc=back").style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_ttl = 3;
    }

    fn ensure_visible(&mut self, visible_rows: usize) {
        if self.selection < self.scroll_offset {
            self.scroll_offset = self.selection;
        } else if self.selection >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selection - visible_rows + 1;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> RulesAction {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        match &self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::ConfirmDelete => self.handle_delete_key(code, conn),
        }
    }

    fn handle_list_key(&mut self, code: KeyCode, _conn: &Connection) -> RulesAction {
        match code {
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                self.ensure_visible(self.last_visible_rows);
            }
            KeyCode::Down => {
                if !self.rules.is_empty() {
                    self.selection = (self.selection + 1).min(self.rules.len() - 1);
                    self.ensure_visible(self.last_visible_rows);
                }
            }
            KeyCode::Char('d') => {
                if !self.rules.is_empty() {
                    self.screen = Screen::ConfirmDelete;
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return RulesAction::Close,
            _ => {}
        }
        RulesAction::Continue
    }

    fn handle_delete_key(&mut self, code: KeyCode, conn: &Connection) -> RulesAction {
        match code {
            KeyCode::Char('y') => {
                if let Some(rule) = self.rules.get(self.selection) {
                    let id = rule.id;
                    let pattern = rule.pattern.clone();
                    match conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id]) {
                        Ok(_) => {
                            self.reload(conn);
                            self.screen = Screen::List;
                            self.set_status(format!("Deleted rule {id}: '{pattern}'"));
                        }
                        Err(e) => {
                            self.screen = Screen::List;
                            self.set_status(format!("Error: {e}"));
                        }
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::List;
            }
            _ => {}
        }
        RulesAction::Continue
    }
}

fn load_rules(conn: &Connection) -> Vec<RuleRow> {
    let mut stmt = match conn.prepare(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.is_active = 1 ORDER BY r.priority DESC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map([], |row| {
        Ok(RuleRow {
            id: row.get(0)?,
            pattern: row.get(1)?,
            match_type: row.get(2)?,
            vendor: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            category: row.get(4)?,
            priority: row.get(5)?,
            hits: row.get(6)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}\u{2026}")
    }
}

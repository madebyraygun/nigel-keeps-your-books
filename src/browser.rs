use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Cell, Paragraph, Row, Table},
    DefaultTerminal, Frame,
};

use crate::fmt::money;
use crate::reports::RegisterRow;
use crate::tui::{self, FOOTER_STYLE, HEADER_STYLE};

const PAGE_SIZE: usize = 20;

enum InputMode {
    Normal,
    GotoPage(String),
    GotoDate(String),
    FindId(String),
}

pub struct RegisterBrowser {
    rows: Vec<RegisterRow>,
    total: f64,
    filters_desc: String,
    offset: usize,
    visible_count: usize,
    input_mode: InputMode,
    status_message: Option<String>,
}

impl RegisterBrowser {
    pub fn new(rows: Vec<RegisterRow>, total: f64, filters_desc: String) -> Self {
        Self {
            rows,
            total,
            filters_desc,
            offset: 0,
            visible_count: PAGE_SIZE,
            input_mode: InputMode::Normal,
            status_message: None,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        if self.rows.is_empty() {
            println!("No transactions found.");
            return Ok(());
        }

        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            ratatui::restore();
            hook(info);
        }));

        let mut terminal = ratatui::init();
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    /// Draw the browser into the given frame. Callable from an external event loop.
    pub fn draw_frame(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let narrow = area.width < 120;

        let [title_area, table_area, status_area, keys_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        // Title
        frame.render_widget(
            Paragraph::new("Transaction Register").style(HEADER_STYLE),
            title_area,
        );

        // Compute description column width from fixed columns + spacing
        let (fixed_cols, num_cols): (u16, u16) = if narrow {
            (6 + 10 + 12 + 28, 5)
        } else {
            (6 + 10 + 12 + 28 + 20 + 20, 7)
        };
        let spacing = num_cols - 1;
        let desc_width = table_area
            .width
            .saturating_sub(fixed_cols + spacing) as usize;
        let desc_width = desc_width.max(10);

        // Build only the visible rows (with text wrapping)
        let header_overhead = 2u16; // header row + bottom_margin
        let available_height = table_area.height.saturating_sub(header_overhead) as usize;
        let mut rendered_rows = Vec::new();
        let mut total_height = 0usize;
        let mut vis = 0usize;

        for row_data in self.rows.iter().skip(self.offset) {
            let (wrapped_desc, line_count) = tui::wrap_text(&row_data.description, desc_width);
            let h = line_count as usize;

            if total_height + h > available_height && vis > 0 {
                break;
            }

            let cat = row_data.category.as_deref().unwrap_or("\u{2014}").to_string();
            let amt = tui::money_span(row_data.amount);

            let cells: Vec<Cell> = if narrow {
                vec![
                    Cell::from(row_data.id.to_string()),
                    Cell::from(row_data.date.clone()),
                    Cell::from(wrapped_desc),
                    Cell::from(amt),
                    Cell::from(cat),
                ]
            } else {
                let vendor = row_data.vendor.as_deref().unwrap_or("").to_string();
                vec![
                    Cell::from(row_data.id.to_string()),
                    Cell::from(row_data.date.clone()),
                    Cell::from(wrapped_desc),
                    Cell::from(amt),
                    Cell::from(cat),
                    Cell::from(vendor),
                    Cell::from(row_data.account_name.clone()),
                ]
            };

            rendered_rows.push(Row::new(cells).height(line_count));
            total_height += h;
            vis += 1;
        }

        self.visible_count = vis.max(1);

        // Table column constraints
        let widths: Vec<Constraint> = if narrow {
            vec![
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(28),
            ]
        } else {
            vec![
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(28),
                Constraint::Length(20),
                Constraint::Length(20),
            ]
        };

        let header_cells: Vec<&str> = if narrow {
            vec!["ID", "Date", "Description", "Amount", "Category"]
        } else {
            vec![
                "ID",
                "Date",
                "Description",
                "Amount",
                "Category",
                "Vendor",
                "Account",
            ]
        };

        let table = Table::new(rendered_rows, widths)
            .header(Row::new(header_cells).style(HEADER_STYLE).bottom_margin(1))
            .column_spacing(1);

        frame.render_widget(table, table_area);

        // Status line
        let end_row = (self.offset + self.visible_count).min(self.rows.len());
        let filters = if self.filters_desc.is_empty() {
            String::new()
        } else {
            format!(" | {}", self.filters_desc)
        };
        let status = if let Some(ref msg) = self.status_message {
            format!(
                "Rows {}-{} of {} | Net: {}{} | {}",
                self.offset + 1,
                end_row,
                self.rows.len(),
                money(self.total),
                filters,
                msg,
            )
        } else {
            format!(
                "Rows {}-{} of {} | Net: {}{}",
                self.offset + 1,
                end_row,
                self.rows.len(),
                money(self.total),
                filters,
            )
        };
        frame.render_widget(Paragraph::new(status).style(FOOTER_STYLE), status_area);

        // Keys / input prompt
        let keys_widget = match &self.input_mode {
            InputMode::Normal => {
                Paragraph::new("n/\u{2192} next  p/\u{2190} prev  Home/End  g:page  d:date  /:id  q:quit")
                    .style(FOOTER_STYLE)
            }
            InputMode::GotoPage(input) => {
                Paragraph::new(format!("Go to page: {input}\u{2588}"))
            }
            InputMode::GotoDate(input) => {
                Paragraph::new(format!("Jump to date (YYYY-MM-DD): {input}\u{2588}"))
            }
            InputMode::FindId(input) => {
                Paragraph::new(format!("Find transaction ID: {input}\u{2588}"))
            }
        };
        frame.render_widget(keys_widget, keys_area);
    }

    /// Handle a key event. Returns true if the browser should close (q/Esc).
    /// Callable from an external event loop (e.g. dashboard).
    pub fn handle_key_event(&mut self, code: KeyCode) -> bool {
        self.status_message = None;

        let in_input = !matches!(self.input_mode, InputMode::Normal);

        if !in_input {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('n') | KeyCode::Right | KeyCode::PageDown => {
                    self.scroll_down();
                }
                KeyCode::Char('p') | KeyCode::Left | KeyCode::PageUp => {
                    self.scroll_up();
                }
                KeyCode::Home => self.offset = 0,
                KeyCode::End => self.scroll_to_end(),
                KeyCode::Char('g') => {
                    self.input_mode = InputMode::GotoPage(String::new());
                }
                KeyCode::Char('d') => {
                    self.input_mode = InputMode::GotoDate(String::new());
                }
                KeyCode::Char('/') => {
                    self.input_mode = InputMode::FindId(String::new());
                }
                _ => {}
            }
        } else {
            match code {
                KeyCode::Esc => self.input_mode = InputMode::Normal,
                KeyCode::Enter => self.submit_input(),
                KeyCode::Backspace => self.input_backspace(),
                KeyCode::Char(c) => self.input_push(c),
                _ => {}
            }
        }
        false
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.draw_frame(frame))?;

            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) = event::read()?
            {
                if kind != KeyEventKind::Press {
                    continue;
                }

                if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
                    break;
                }

                if self.handle_key_event(code) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn scroll_down(&mut self) {
        let new_offset = self.offset + self.visible_count;
        if new_offset < self.rows.len() {
            self.offset = new_offset;
        }
    }

    fn scroll_up(&mut self) {
        self.offset = self.offset.saturating_sub(self.visible_count);
    }

    fn scroll_to_end(&mut self) {
        // Walk backward to find the offset that shows the last row.
        // This accounts for variable row heights from text wrapping.
        // We use PAGE_SIZE as an approximation since we don't know the
        // actual row heights without a frame, but it's a safe lower bound.
        self.offset = self.rows.len().saturating_sub(PAGE_SIZE);
    }

    fn input_push(&mut self, c: char) {
        match &mut self.input_mode {
            InputMode::GotoPage(s) | InputMode::GotoDate(s) | InputMode::FindId(s) => s.push(c),
            _ => {}
        }
    }

    fn input_backspace(&mut self) {
        match &mut self.input_mode {
            InputMode::GotoPage(s) | InputMode::GotoDate(s) | InputMode::FindId(s) => {
                s.pop();
            }
            _ => {}
        }
    }

    fn submit_input(&mut self) {
        let mode = std::mem::replace(&mut self.input_mode, InputMode::Normal);
        match &mode {
            InputMode::GotoPage(input) => {
                if let Ok(page) = input.trim().parse::<usize>() {
                    if page >= 1 {
                        let target = (page - 1) * PAGE_SIZE;
                        self.offset = target.min(self.rows.len().saturating_sub(1));
                    }
                }
            }
            InputMode::GotoDate(input) => {
                let target = input.trim();
                if !target.is_empty() {
                    if let Some(idx) = self.rows.iter().position(|r| r.date.as_str() >= target) {
                        self.offset = idx;
                    } else {
                        self.status_message = Some(format!("No transactions on or after {target}"));
                    }
                }
            }
            InputMode::FindId(input) => {
                if let Ok(id) = input.trim().parse::<i64>() {
                    if let Some(idx) = self.rows.iter().position(|r| r.id == id) {
                        self.offset = idx;
                    } else {
                        self.status_message = Some(format!("Transaction #{id} not found"));
                    }
                }
            }
            InputMode::Normal => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::RegisterRow;

    fn make_rows(n: usize) -> Vec<RegisterRow> {
        (0..n)
            .map(|i| RegisterRow {
                id: (i + 1) as i64,
                date: format!("2025-01-{:02}", (i % 28) + 1),
                description: format!("Transaction {}", i + 1),
                amount: if i % 2 == 0 { 100.0 } else { -50.0 },
                category: Some("Test Category".to_string()),
                category_id: Some(1),
                vendor: None,
                account_name: "Test Account".to_string(),
                is_flagged: false,
            })
            .collect()
    }

    #[test]
    fn test_scroll_down() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());
        assert_eq!(browser.offset, 0);

        browser.scroll_down();
        assert_eq!(browser.offset, PAGE_SIZE);

        browser.scroll_down();
        assert_eq!(browser.offset, PAGE_SIZE * 2);
    }

    #[test]
    fn test_scroll_down_stops_at_end() {
        let rows = make_rows(10);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());
        browser.scroll_down(); // 10 < PAGE_SIZE, so offset stays
        assert_eq!(browser.offset, 0);
    }

    #[test]
    fn test_scroll_up() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());
        browser.offset = PAGE_SIZE * 2;

        browser.scroll_up();
        assert_eq!(browser.offset, PAGE_SIZE);

        browser.scroll_up();
        assert_eq!(browser.offset, 0);

        browser.scroll_up(); // doesn't go negative
        assert_eq!(browser.offset, 0);
    }

    #[test]
    fn test_scroll_to_end() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());
        browser.scroll_to_end();
        assert_eq!(browser.offset, 50 - PAGE_SIZE);
    }

    #[test]
    fn test_scroll_to_end_small_dataset() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());
        browser.scroll_to_end();
        assert_eq!(browser.offset, 0); // 5 < PAGE_SIZE, stays at 0
    }

    #[test]
    fn test_goto_page() {
        let rows = make_rows(100);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());

        browser.input_mode = InputMode::GotoPage("3".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 2 * PAGE_SIZE);
    }

    #[test]
    fn test_goto_date_found() {
        let rows = make_rows(30);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());

        browser.input_mode = InputMode::GotoDate("2025-01-15".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 14); // 0-indexed, date "2025-01-15" is at index 14
    }

    #[test]
    fn test_goto_date_not_found() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());

        browser.input_mode = InputMode::GotoDate("2026-01-01".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 0); // unchanged
        assert!(browser.status_message.is_some());
        assert!(browser.status_message.as_ref().unwrap().contains("2026-01-01"));
    }

    #[test]
    fn test_find_id_found() {
        let rows = make_rows(30);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());

        browser.input_mode = InputMode::FindId("25".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 24); // id 25 is at index 24
    }

    #[test]
    fn test_find_id_not_found() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new());

        browser.input_mode = InputMode::FindId("999".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 0); // unchanged
        assert!(browser.status_message.is_some());
        assert!(browser.status_message.as_ref().unwrap().contains("999"));
    }
}

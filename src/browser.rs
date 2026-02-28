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

enum InputMode {
    Normal,
    GotoPage(String),
    GotoDate(String),
    FindId(String),
}

pub struct RegisterBrowser {
    rows: Vec<RegisterRow>,
    total: f64,
    count: usize,
    filters_desc: String,
    offset: usize,
    visible_count: usize,
    input_mode: InputMode,
}

impl RegisterBrowser {
    pub fn new(rows: Vec<RegisterRow>, total: f64, count: usize, filters_desc: String) -> Self {
        Self {
            rows,
            total,
            count,
            filters_desc,
            offset: 0,
            visible_count: 20,
            input_mode: InputMode::Normal,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        if self.rows.is_empty() {
            println!("No transactions found.");
            return Ok(());
        }

        let mut terminal = ratatui::init();
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn draw(&mut self, frame: &mut Frame) {
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
        let status = format!(
            "Rows {}-{} of {} | Net: {}{}",
            if self.rows.is_empty() {
                0
            } else {
                self.offset + 1
            },
            end_row,
            self.count,
            money(self.total),
            filters,
        );
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

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

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

                let in_input = !matches!(self.input_mode, InputMode::Normal);

                if !in_input {
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
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
        if self.rows.len() > self.visible_count {
            self.offset = self.rows.len() - self.visible_count;
        }
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
                        let target = (page - 1) * self.visible_count;
                        self.offset = target.min(self.rows.len().saturating_sub(1));
                    }
                }
            }
            InputMode::GotoDate(input) => {
                let target = input.trim();
                if !target.is_empty() {
                    if let Some(idx) = self.rows.iter().position(|r| r.date.as_str() >= target) {
                        self.offset = idx;
                    }
                }
            }
            InputMode::FindId(input) => {
                if let Ok(id) = input.trim().parse::<i64>() {
                    if let Some(idx) = self.rows.iter().position(|r| r.id == id) {
                        self.offset = idx;
                    }
                }
            }
            InputMode::Normal => {}
        }
    }
}

use std::io;

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, TableState},
    DefaultTerminal, Frame,
};

use crate::fmt::money;
use crate::reports::RegisterRow;
use crate::reviewer::CategoryChoice;
use crate::tui::{self, FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

const PAGE_SIZE: usize = 20;

enum BrowseMode {
    Normal,
    GotoPage(String),
    GotoDate(String),
    FindId(String),
    EditCategory { query: String, selection: usize },
    EditVendor(String),
}

pub enum BrowseAction {
    Continue,
    Close,
    CommitEdit,
    ToggleFlag,
}

pub struct RegisterBrowser {
    rows: Vec<RegisterRow>,
    total: f64,
    filters_desc: String,
    offset: usize,
    visible_count: usize,
    selected: usize,
    mode: BrowseMode,
    status_message: Option<String>,
    categories: Vec<CategoryChoice>,
    cat_labels: Vec<String>,
    pending_category_idx: Option<usize>,
    pending_vendor: Option<String>,
    table_state: TableState,
}

impl RegisterBrowser {
    pub fn new(
        rows: Vec<RegisterRow>,
        total: f64,
        filters_desc: String,
        categories: Vec<CategoryChoice>,
    ) -> Self {
        let cat_labels: Vec<String> = categories
            .iter()
            .map(|c| {
                let tag = if c.category_type == "income" { "inc" } else { "exp" };
                format!("{} ({})", c.name, tag)
            })
            .collect();
        Self {
            rows,
            total,
            filters_desc,
            offset: 0,
            visible_count: PAGE_SIZE,
            selected: 0,
            mode: BrowseMode::Normal,
            status_message: None,
            categories,
            cat_labels,
            pending_category_idx: None,
            pending_vendor: None,
            table_state: TableState::default(),
        }
    }

    /// Scroll so that the last transaction on or before today is visible.
    pub fn scroll_to_today(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        // Find the last row with date <= today
        let idx = self
            .rows
            .iter()
            .rposition(|r| r.date.as_str() <= today.as_str());
        if let Some(i) = idx {
            // Position that row on screen (offset so it's visible, near middle)
            self.offset = i.saturating_sub(PAGE_SIZE / 2);
            self.selected = i - self.offset;
        } else if !self.rows.is_empty() {
            // All transactions are in the future — start at the beginning
            self.offset = 0;
            self.selected = 0;
        }
    }

    pub fn run(&mut self, conn: &rusqlite::Connection) -> io::Result<()> {
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
        let result = self.event_loop(&mut terminal, conn);
        ratatui::restore();
        result
    }

    /// Draw the browser into the given frame. Callable from an external event loop.
    pub fn draw_frame(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let narrow = area.width < 120;

        let edit_height: u16 = match &self.mode {
            BrowseMode::EditCategory { .. } => {
                let matches = self.filtered_categories().len();
                1 + matches.min(9) as u16
            }
            BrowseMode::EditVendor(_) => 1,
            _ => 0,
        };

        let areas = Layout::vertical([
            Constraint::Length(1),      // title
            Constraint::Fill(1),        // table
            Constraint::Length(edit_height), // edit panel
            Constraint::Length(1),      // status
            Constraint::Length(1),      // keys
        ])
        .split(area);
        let title_area = areas[0];
        let table_area = areas[1];
        let edit_area = areas[2];
        let status_area = areas[3];
        let keys_area = areas[4];

        // Title
        frame.render_widget(
            Paragraph::new("Transaction Register").style(HEADER_STYLE),
            title_area,
        );

        // Compute description column width from fixed columns + spacing
        let (fixed_cols, num_cols): (u16, u16) = if narrow {
            (2 + 6 + 10 + 12 + 28, 6)
        } else {
            (2 + 6 + 10 + 12 + 28 + 20 + 20, 8)
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
            let flag_cell = Cell::from(if row_data.is_flagged { "!" } else { "" });

            let cells: Vec<Cell> = if narrow {
                vec![
                    flag_cell,
                    Cell::from(row_data.id.to_string()),
                    Cell::from(row_data.date.clone()),
                    Cell::from(wrapped_desc),
                    Cell::from(amt),
                    Cell::from(cat),
                ]
            } else {
                let vendor = row_data.vendor.as_deref().unwrap_or("").to_string();
                vec![
                    flag_cell,
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
                Constraint::Length(2),
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(28),
            ]
        } else {
            vec![
                Constraint::Length(2),
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
            vec!["", "ID", "Date", "Description", "Amount", "Category"]
        } else {
            vec![
                "",
                "ID",
                "Date",
                "Description",
                "Amount",
                "Category",
                "Vendor",
                "Account",
            ]
        };

        self.table_state.select(Some(self.selected));
        let table = Table::new(rendered_rows, widths)
            .header(Row::new(header_cells).style(HEADER_STYLE).bottom_margin(1))
            .column_spacing(1)
            .row_highlight_style(SELECTED_STYLE);

        frame.render_stateful_widget(table, table_area, &mut self.table_state);

        // Edit panel
        if edit_height > 0 {
            let edit_lines: Vec<Line> = match &self.mode {
                BrowseMode::EditCategory { query, selection } => {
                    let matches = self.filtered_categories();
                    let mut lines = vec![Line::from(format!("  Category: {query}\u{2588}"))];
                    if !query.is_empty() && matches.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "    (no matches)",
                            Style::default().fg(Color::DarkGray),
                        )));
                    } else {
                        for (i, (_, label)) in matches.iter().enumerate() {
                            let marker = if i == *selection { ">" } else { " " };
                            lines.push(Line::from(format!("  {marker} {label}")));
                        }
                    }
                    lines
                }
                BrowseMode::EditVendor(input) => {
                    vec![Line::from(format!(
                        "  Vendor (Enter to skip): {input}\u{2588}"
                    ))]
                }
                _ => vec![],
            };
            frame.render_widget(Paragraph::new(edit_lines), edit_area);
        }

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
        let keys_widget = match &self.mode {
            BrowseMode::Normal => {
                Paragraph::new("\u{2191}/\u{2193}:select  e:edit  f:flag  n/\u{2192}:next  p/\u{2190}:prev  g:page  d:date  /:id  q:quit")
                    .style(FOOTER_STYLE)
            }
            BrowseMode::GotoPage(input) => {
                Paragraph::new(format!("Go to page: {input}\u{2588}"))
            }
            BrowseMode::GotoDate(input) => {
                Paragraph::new(format!("Jump to date (YYYY-MM-DD): {input}\u{2588}"))
            }
            BrowseMode::FindId(input) => {
                Paragraph::new(format!("Find transaction ID: {input}\u{2588}"))
            }
            BrowseMode::EditCategory { .. } => {
                Paragraph::new("Type to filter, Enter=select, Esc=cancel")
                    .style(FOOTER_STYLE)
            }
            BrowseMode::EditVendor(_) => {
                Paragraph::new("Enter=confirm (empty to skip), Esc=cancel")
                    .style(FOOTER_STYLE)
            }
        };
        frame.render_widget(keys_widget, keys_area);
    }

    /// Handle a key event. Returns a BrowseAction indicating what the caller should do.
    pub fn handle_key_event(&mut self, code: KeyCode) -> BrowseAction {
        self.status_message = None;

        match &self.mode {
            BrowseMode::Normal => match code {
                KeyCode::Char('q') | KeyCode::Esc => return BrowseAction::Close,
                KeyCode::Down => {
                    if self.selected + 1 < self.visible_count.min(self.rows.len() - self.offset) {
                        self.selected += 1;
                    } else if self.offset + self.visible_count < self.rows.len() {
                        self.offset += 1;
                    }
                }
                KeyCode::Up => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    } else if self.offset > 0 {
                        self.offset -= 1;
                    }
                }
                KeyCode::Char('n') | KeyCode::Right | KeyCode::PageDown => {
                    self.scroll_down();
                    self.selected = 0;
                }
                KeyCode::Char('p') | KeyCode::Left | KeyCode::PageUp => {
                    self.scroll_up();
                    self.selected = 0;
                }
                KeyCode::Home => {
                    self.offset = 0;
                    self.selected = 0;
                }
                KeyCode::End => {
                    self.scroll_to_end();
                    self.selected = 0;
                }
                KeyCode::Char('g') => {
                    self.mode = BrowseMode::GotoPage(String::new());
                }
                KeyCode::Char('d') => {
                    self.mode = BrowseMode::GotoDate(String::new());
                }
                KeyCode::Char('/') => {
                    self.mode = BrowseMode::FindId(String::new());
                }
                KeyCode::Char('e') | KeyCode::Enter => {
                    if !self.categories.is_empty() {
                        self.mode = BrowseMode::EditCategory {
                            query: String::new(),
                            selection: 0,
                        };
                    }
                }
                KeyCode::Char('f') => {
                    return BrowseAction::ToggleFlag;
                }
                _ => {}
            },
            BrowseMode::GotoPage(_)
            | BrowseMode::GotoDate(_)
            | BrowseMode::FindId(_) => match code {
                KeyCode::Esc => self.mode = BrowseMode::Normal,
                KeyCode::Enter => self.submit_input(),
                KeyCode::Backspace => self.input_backspace(),
                KeyCode::Char(c) => self.input_push(c),
                _ => {}
            },
            BrowseMode::EditCategory { .. } => {
                return self.handle_edit_category_key(code);
            }
            BrowseMode::EditVendor(_) => {
                return self.handle_edit_vendor_key(code);
            }
        }
        BrowseAction::Continue
    }

    fn event_loop(
        &mut self,
        terminal: &mut DefaultTerminal,
        conn: &rusqlite::Connection,
    ) -> io::Result<()> {
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

                match self.handle_key_event(code) {
                    BrowseAction::Close => break,
                    BrowseAction::Continue => {}
                    BrowseAction::CommitEdit => {
                        if let Err(e) = self.commit_edit(conn) {
                            self.status_message = Some(format!("Edit failed: {e}"));
                        }
                    }
                    BrowseAction::ToggleFlag => {
                        if let Err(e) = self.toggle_flag(conn) {
                            self.status_message = Some(format!("Flag toggle failed: {e}"));
                        }
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
        self.offset = self.rows.len().saturating_sub(PAGE_SIZE);
    }

    fn input_push(&mut self, c: char) {
        match &mut self.mode {
            BrowseMode::GotoPage(s) | BrowseMode::GotoDate(s) | BrowseMode::FindId(s) => {
                s.push(c);
            }
            _ => {}
        }
    }

    fn input_backspace(&mut self) {
        match &mut self.mode {
            BrowseMode::GotoPage(s) | BrowseMode::GotoDate(s) | BrowseMode::FindId(s) => {
                s.pop();
            }
            _ => {}
        }
    }

    fn submit_input(&mut self) {
        let mode = std::mem::replace(&mut self.mode, BrowseMode::Normal);
        match &mode {
            BrowseMode::GotoPage(input) => {
                if let Ok(page) = input.trim().parse::<usize>() {
                    if page >= 1 {
                        let target = (page - 1) * PAGE_SIZE;
                        self.offset = target.min(self.rows.len().saturating_sub(1));
                        self.selected = 0;
                    }
                }
            }
            BrowseMode::GotoDate(input) => {
                let target = input.trim();
                if !target.is_empty() {
                    if let Some(idx) = self.rows.iter().position(|r| r.date.as_str() >= target) {
                        self.offset = idx;
                        self.selected = 0;
                    } else {
                        self.status_message =
                            Some(format!("No transactions on or after {target}"));
                    }
                }
            }
            BrowseMode::FindId(input) => {
                if let Ok(id) = input.trim().parse::<i64>() {
                    if let Some(idx) = self.rows.iter().position(|r| r.id == id) {
                        self.offset = idx;
                        self.selected = 0;
                    } else {
                        self.status_message = Some(format!("Transaction #{id} not found"));
                    }
                }
            }
            _ => {}
        }
    }

    fn filtered_categories(&self) -> Vec<(usize, &str)> {
        let query = match &self.mode {
            BrowseMode::EditCategory { query, .. } => query,
            _ => return vec![],
        };
        if query.is_empty() {
            return vec![];
        }
        let q = query.to_lowercase();
        self.cat_labels
            .iter()
            .enumerate()
            .filter(|(_, label)| label.to_lowercase().contains(&q))
            .map(|(i, s)| (i, s.as_str()))
            .take(9)
            .collect()
    }

    fn handle_edit_category_key(&mut self, code: KeyCode) -> BrowseAction {
        match code {
            KeyCode::Char(c) => {
                if let BrowseMode::EditCategory { query, selection } = &mut self.mode {
                    query.push(c);
                    *selection = 0;
                }
            }
            KeyCode::Backspace => {
                if let BrowseMode::EditCategory { query, selection } = &mut self.mode {
                    query.pop();
                    *selection = 0;
                }
            }
            KeyCode::Up => {
                if let BrowseMode::EditCategory { selection, .. } = &mut self.mode {
                    *selection = selection.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                // Compute count before mutably borrowing self.mode (borrow checker constraint)
                let count = self.filtered_categories().len();
                if let BrowseMode::EditCategory { selection, .. } = &mut self.mode {
                    if count > 0 && *selection + 1 < count {
                        *selection += 1;
                    }
                }
            }
            KeyCode::Enter => {
                let matches = self.filtered_categories();
                if !matches.is_empty() {
                    let sel_idx = match &self.mode {
                        BrowseMode::EditCategory { selection, .. } => {
                            (*selection).min(matches.len() - 1)
                        }
                        _ => 0,
                    };
                    self.pending_category_idx = Some(matches[sel_idx].0);
                    self.mode = BrowseMode::EditVendor(String::new());
                }
            }
            KeyCode::Esc => {
                self.mode = BrowseMode::Normal;
                self.pending_category_idx = None;
            }
            _ => {}
        }
        BrowseAction::Continue
    }

    fn handle_edit_vendor_key(&mut self, code: KeyCode) -> BrowseAction {
        match code {
            KeyCode::Char(c) => {
                if let BrowseMode::EditVendor(input) = &mut self.mode {
                    input.push(c);
                }
            }
            KeyCode::Backspace => {
                if let BrowseMode::EditVendor(input) = &mut self.mode {
                    input.pop();
                }
            }
            KeyCode::Enter => {
                let vendor = match &self.mode {
                    BrowseMode::EditVendor(input) => {
                        if input.is_empty() {
                            None
                        } else {
                            Some(input.clone())
                        }
                    }
                    _ => None,
                };
                self.pending_vendor = vendor;
                self.mode = BrowseMode::Normal;
                return BrowseAction::CommitEdit;
            }
            KeyCode::Esc => {
                self.mode = BrowseMode::Normal;
                self.pending_category_idx = None;
                self.pending_vendor = None;
            }
            _ => {}
        }
        BrowseAction::Continue
    }

    fn apply_edit_to_local_row(&mut self) {
        let abs_idx = self.offset + self.selected;
        if let Some(cat_idx) = self.pending_category_idx {
            if let Some(row) = self.rows.get_mut(abs_idx) {
                let cat = &self.categories[cat_idx];
                row.category = Some(cat.name.clone());
                row.category_id = Some(cat.id);
                if let Some(ref v) = self.pending_vendor {
                    row.vendor = Some(v.clone());
                } else {
                    row.vendor = None;
                }
            }
        }
        self.pending_category_idx = None;
        self.pending_vendor = None;
    }

    fn apply_flag_toggle_to_local_row(&mut self, new_state: bool) {
        let abs_idx = self.offset + self.selected;
        if let Some(row) = self.rows.get_mut(abs_idx) {
            row.is_flagged = new_state;
        }
    }

    pub fn commit_edit(&mut self, conn: &rusqlite::Connection) -> crate::error::Result<()> {
        let abs_idx = self.offset + self.selected;
        let row = self.rows.get(abs_idx)
            .ok_or_else(|| crate::error::NigelError::Other("No row selected".into()))?;
        let txn_id = row.id;

        if let Some(cat_idx) = self.pending_category_idx {
            let cat_id = self.categories[cat_idx].id;
            crate::reviewer::update_transaction_category(conn, txn_id, cat_id)?;
            if let Some(ref v) = self.pending_vendor {
                crate::reviewer::update_transaction_vendor(conn, txn_id, Some(v))?;
            } else {
                crate::reviewer::update_transaction_vendor(conn, txn_id, None)?;
            }
        }

        self.apply_edit_to_local_row();
        self.status_message = Some(format!("Updated transaction #{txn_id}"));
        Ok(())
    }

    /// Toggle the flag on the selected transaction.
    /// Flags are non-destructive metadata — single-keypress toggle is intentional
    /// since it's instantly reversible (press `f` again).
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
    }

    pub fn toggle_flag(&mut self, conn: &rusqlite::Connection) -> crate::error::Result<()> {
        let abs_idx = self.offset + self.selected;
        let row = self.rows.get(abs_idx)
            .ok_or_else(|| crate::error::NigelError::Other("No row selected".into()))?;
        let txn_id = row.id;
        let new_state = crate::reviewer::toggle_transaction_flag(conn, txn_id)?;
        self.apply_flag_toggle_to_local_row(new_state);
        let label = if new_state { "flagged" } else { "unflagged" };
        self.status_message = Some(format!("Transaction #{txn_id} {label}"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::RegisterRow;
    use crate::reviewer::CategoryChoice;

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

    fn make_categories() -> Vec<CategoryChoice> {
        vec![
            CategoryChoice { id: 1, name: "Advertising".to_string(), category_type: "expense".to_string() },
            CategoryChoice { id: 2, name: "Software & Subscriptions".to_string(), category_type: "expense".to_string() },
            CategoryChoice { id: 3, name: "Revenue".to_string(), category_type: "income".to_string() },
        ]
    }

    #[test]
    fn test_scroll_down() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        assert_eq!(browser.offset, 0);

        browser.scroll_down();
        assert_eq!(browser.offset, PAGE_SIZE);

        browser.scroll_down();
        assert_eq!(browser.offset, PAGE_SIZE * 2);
    }

    #[test]
    fn test_scroll_down_stops_at_end() {
        let rows = make_rows(10);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        browser.scroll_down(); // 10 < PAGE_SIZE, so offset stays
        assert_eq!(browser.offset, 0);
    }

    #[test]
    fn test_scroll_up() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
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
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        browser.scroll_to_end();
        assert_eq!(browser.offset, 50 - PAGE_SIZE);
    }

    #[test]
    fn test_scroll_to_end_small_dataset() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        browser.scroll_to_end();
        assert_eq!(browser.offset, 0); // 5 < PAGE_SIZE, stays at 0
    }

    #[test]
    fn test_goto_page() {
        let rows = make_rows(100);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);

        browser.mode = BrowseMode::GotoPage("3".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 2 * PAGE_SIZE);
    }

    #[test]
    fn test_goto_date_found() {
        let rows = make_rows(30);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);

        browser.mode = BrowseMode::GotoDate("2025-01-15".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 14); // 0-indexed, date "2025-01-15" is at index 14
    }

    #[test]
    fn test_goto_date_not_found() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);

        browser.mode = BrowseMode::GotoDate("2026-01-01".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 0); // unchanged
        assert!(browser.status_message.is_some());
        assert!(browser.status_message.as_ref().unwrap().contains("2026-01-01"));
    }

    #[test]
    fn test_find_id_found() {
        let rows = make_rows(30);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);

        browser.mode = BrowseMode::FindId("25".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 24); // id 25 is at index 24
    }

    #[test]
    fn test_find_id_not_found() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);

        browser.mode = BrowseMode::FindId("999".to_string());
        browser.submit_input();
        assert_eq!(browser.offset, 0); // unchanged
        assert!(browser.status_message.is_some());
        assert!(browser.status_message.as_ref().unwrap().contains("999"));
    }

    #[test]
    fn test_handle_key_returns_close_on_q() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        let action = browser.handle_key_event(KeyCode::Char('q'));
        assert!(matches!(action, BrowseAction::Close));
    }

    #[test]
    fn test_handle_key_returns_continue_on_nav() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        let action = browser.handle_key_event(KeyCode::Char('n'));
        assert!(matches!(action, BrowseAction::Continue));
    }

    #[test]
    fn test_selected_row_up_down() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        assert_eq!(browser.selected, 0);

        browser.handle_key_event(KeyCode::Down);
        assert_eq!(browser.selected, 1);

        browser.handle_key_event(KeyCode::Down);
        assert_eq!(browser.selected, 2);

        browser.handle_key_event(KeyCode::Up);
        assert_eq!(browser.selected, 1);

        browser.handle_key_event(KeyCode::Up);
        assert_eq!(browser.selected, 0);

        // Can't go below 0
        browser.handle_key_event(KeyCode::Up);
        assert_eq!(browser.selected, 0);
    }

    #[test]
    fn test_page_navigation_resets_selected() {
        let rows = make_rows(50);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        browser.selected = 5;

        browser.handle_key_event(KeyCode::Char('n'));
        assert_eq!(browser.selected, 0);
    }

    #[test]
    fn test_enter_edit_mode() {
        let rows = make_rows(5);
        let cats = make_categories();
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), cats);

        browser.handle_key_event(KeyCode::Char('e'));
        assert!(matches!(browser.mode, BrowseMode::EditCategory { .. }));
    }

    #[test]
    fn test_edit_category_filter_and_select() {
        let rows = make_rows(5);
        let cats = make_categories();
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), cats);

        browser.handle_key_event(KeyCode::Char('e'));
        // Type "adv" to filter
        browser.handle_key_event(KeyCode::Char('a'));
        browser.handle_key_event(KeyCode::Char('d'));
        browser.handle_key_event(KeyCode::Char('v'));

        // Filter should match "Advertising"
        let matches = browser.filtered_categories();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].1.contains("Advertising"));

        // Select it
        browser.handle_key_event(KeyCode::Enter);
        assert!(matches!(browser.mode, BrowseMode::EditVendor(_)));
        assert_eq!(browser.pending_category_idx, Some(0));
    }

    #[test]
    fn test_edit_vendor_and_commit() {
        let rows = make_rows(5);
        let cats = make_categories();
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), cats);

        // Enter edit, select first category
        browser.mode = BrowseMode::EditCategory { query: String::new(), selection: 0 };
        browser.handle_key_event(KeyCode::Char('a'));
        browser.handle_key_event(KeyCode::Enter);

        // Now in vendor mode, type vendor name
        browser.handle_key_event(KeyCode::Char('F'));
        browser.handle_key_event(KeyCode::Char('o'));
        browser.handle_key_event(KeyCode::Char('o'));
        let action = browser.handle_key_event(KeyCode::Enter);
        assert!(matches!(action, BrowseAction::CommitEdit));
        assert_eq!(browser.pending_vendor.as_deref(), Some("Foo"));
    }

    #[test]
    fn test_esc_cancels_edit() {
        let rows = make_rows(5);
        let cats = make_categories();
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), cats);

        browser.handle_key_event(KeyCode::Char('e'));
        browser.handle_key_event(KeyCode::Esc);
        assert!(matches!(browser.mode, BrowseMode::Normal));
    }

    #[test]
    fn test_commit_edit_updates_row() {
        let rows = make_rows(5);
        let cats = make_categories();
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), cats);

        browser.pending_category_idx = Some(0); // "Advertising"
        browser.pending_vendor = Some("TestVendor".to_string());
        browser.selected = 0;

        browser.apply_edit_to_local_row();
        assert_eq!(browser.rows[0].category.as_deref(), Some("Advertising"));
        assert_eq!(browser.rows[0].vendor.as_deref(), Some("TestVendor"));
        assert_eq!(browser.rows[0].category_id, Some(1));
    }

    #[test]
    fn test_toggle_flag_updates_row() {
        let rows = make_rows(5);
        let mut browser = RegisterBrowser::new(rows, 0.0, String::new(), vec![]);
        assert!(!browser.rows[0].is_flagged);

        browser.apply_flag_toggle_to_local_row(true);
        assert!(browser.rows[0].is_flagged);
    }
}

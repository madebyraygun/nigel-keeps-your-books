use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{LineGauge, Paragraph},
    Frame,
};

use crate::db::get_connection;
use crate::error::Result;
use crate::reviewer::{apply_review, get_categories, get_flagged_transactions, CategoryChoice, FlaggedTxn};
use crate::settings::get_data_dir;
use crate::tui::money_span;

enum ReviewState {
    PickCategory,
    InputVendor,
    ConfirmRule,
    InputRulePattern,
}

struct TransactionReviewer {
    flagged: Vec<FlaggedTxn>,
    categories: Vec<CategoryChoice>,
    labels: Vec<String>,
    current_txn: usize,
    state: ReviewState,
    cat_query: String,
    cat_selection: usize,
    text_input: String,
    confirm_value: bool,
    selected_category_idx: Option<usize>,
    vendor: Option<String>,
}

impl TransactionReviewer {
    fn new(flagged: Vec<FlaggedTxn>, categories: Vec<CategoryChoice>) -> Self {
        let labels: Vec<String> = categories
            .iter()
            .map(|c| {
                let tag = if c.category_type == "income" { "inc" } else { "exp" };
                format!("{} ({})", c.name, tag)
            })
            .collect();
        Self {
            flagged,
            categories,
            labels,
            current_txn: 0,
            state: ReviewState::PickCategory,
            cat_query: String::new(),
            cat_selection: 0,
            text_input: String::new(),
            confirm_value: false,
            selected_category_idx: None,
            vendor: None,
        }
    }

    fn filtered_categories(&self) -> Vec<(usize, &str)> {
        if self.cat_query.is_empty() {
            return vec![];
        }
        let q = self.cat_query.to_lowercase();
        self.labels
            .iter()
            .enumerate()
            .filter(|(_, label)| label.to_lowercase().contains(&q))
            .map(|(i, s)| (i, s.as_str()))
            .take(9)
            .collect()
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let txn = &self.flagged[self.current_txn];
        let total = self.flagged.len();

        // Compute category chart rows dynamically
        let col_width = self.labels.iter().map(|e| e.len()).max().unwrap_or(20) + 2;
        let cols = (area.width as usize / col_width).max(1);
        let chart_rows = ((self.labels.len() + cols - 1) / cols) as u16 + 1; // +1 for "Categories" header

        let [chart_area, progress_area, detail_area, interaction_area, hints_area] =
            Layout::vertical([
                Constraint::Length(chart_rows),
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(area);

        // Category chart
        let mut chart_lines: Vec<Line> = vec![Line::from(
            Span::styled("Categories", Style::default().fg(Color::DarkGray)),
        )];
        let rows = ((self.labels.len() + cols - 1) / cols).max(1);
        for row in 0..rows {
            let mut spans = Vec::new();
            for col in 0..cols {
                let idx = col * rows + row;
                if let Some(entry) = self.labels.get(idx) {
                    spans.push(Span::styled(
                        format!("{:<width$}", entry, width = col_width),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            chart_lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(chart_lines), chart_area);

        // Progress bar
        let ratio = if total > 1 {
            self.current_txn as f64 / (total - 1) as f64
        } else {
            1.0
        };
        let gauge = LineGauge::default()
            .label(format!("{} of {}", self.current_txn + 1, total))
            .ratio(ratio)
            .filled_style(Style::default().fg(Color::Green).bold())
            .unfilled_style(Style::default().fg(Color::DarkGray))
            .line_set(ratatui::symbols::line::THICK);
        frame.render_widget(gauge, progress_area);

        // Transaction details
        let detail_lines = vec![
            Line::from(""),
            Line::from(format!("  Date:        {}", txn.date)),
            Line::from(format!("  Description: {}", txn.description)),
            Line::from(vec![
                Span::raw("  Amount:      "),
                money_span(txn.amount),
            ]),
            Line::from(format!("  Account:     {}", txn.account_name)),
            Line::from(""),
        ];
        frame.render_widget(Paragraph::new(detail_lines), detail_area);

        // Interaction area — changes per state
        let interaction_lines: Vec<Line> = match &self.state {
            ReviewState::PickCategory => {
                let matches = self.filtered_categories();
                let mut lines = vec![Line::from(format!(
                    "  Category: {}\u{2588}",
                    self.cat_query
                ))];
                if !self.cat_query.is_empty() && matches.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    (no matches)",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (i, (_, label)) in matches.iter().enumerate() {
                        let marker = if i == self.cat_selection { ">" } else { " " };
                        lines.push(Line::from(format!("  {marker} {label}")));
                    }
                }
                lines
            }
            ReviewState::InputVendor => {
                vec![Line::from(format!(
                    "  Vendor (Enter to skip): {}\u{2588}",
                    self.text_input
                ))]
            }
            ReviewState::ConfirmRule => {
                let (yes_style, no_style) = if self.confirm_value {
                    (
                        Style::default().fg(Color::White).bg(Color::Blue),
                        Style::default(),
                    )
                } else {
                    (
                        Style::default(),
                        Style::default().fg(Color::White).bg(Color::Blue),
                    )
                };
                vec![Line::from(vec![
                    Span::raw("  Create rule for future matches?  "),
                    Span::styled(" Yes ", yes_style),
                    Span::raw("  "),
                    Span::styled(" No ", no_style),
                ])]
            }
            ReviewState::InputRulePattern => {
                vec![Line::from(format!(
                    "  Rule pattern: {}\u{2588}",
                    self.text_input
                ))]
            }
        };
        frame.render_widget(Paragraph::new(interaction_lines), interaction_area);

        // Hints
        let hints = match &self.state {
            ReviewState::PickCategory => {
                "Type to filter, Enter=select, Esc=skip, Ctrl+C=quit"
            }
            ReviewState::InputVendor => "Enter=confirm (empty to skip), Ctrl+C=quit",
            ReviewState::ConfirmRule => "y/n or Left/Right to toggle, Enter=confirm, Ctrl+C=quit",
            ReviewState::InputRulePattern => "Enter=confirm, Ctrl+C=quit",
        };
        frame.render_widget(
            Paragraph::new(hints).style(Style::default().fg(Color::DarkGray)),
            hints_area,
        );
    }

    fn handle_key(&mut self, code: KeyCode) -> HandleResult {
        match &self.state {
            ReviewState::PickCategory => match code {
                KeyCode::Char(c) => {
                    self.cat_query.push(c);
                    self.cat_selection = 0;
                    HandleResult::Continue
                }
                KeyCode::Backspace => {
                    self.cat_query.pop();
                    self.cat_selection = 0;
                    HandleResult::Continue
                }
                KeyCode::Up => {
                    self.cat_selection = self.cat_selection.saturating_sub(1);
                    HandleResult::Continue
                }
                KeyCode::Down => {
                    let matches = self.filtered_categories();
                    if !matches.is_empty() {
                        self.cat_selection =
                            (self.cat_selection + 1).min(matches.len() - 1);
                    }
                    HandleResult::Continue
                }
                KeyCode::Enter => {
                    let matches = self.filtered_categories();
                    if !matches.is_empty() {
                        let sel = self.cat_selection.min(matches.len() - 1);
                        self.selected_category_idx = Some(matches[sel].0);
                        self.text_input.clear();
                        self.state = ReviewState::InputVendor;
                    }
                    HandleResult::Continue
                }
                KeyCode::Esc => {
                    self.advance();
                    HandleResult::check_done(self)
                }
                _ => HandleResult::Continue,
            },
            ReviewState::InputVendor => match code {
                KeyCode::Char(c) => {
                    self.text_input.push(c);
                    HandleResult::Continue
                }
                KeyCode::Backspace => {
                    self.text_input.pop();
                    HandleResult::Continue
                }
                KeyCode::Enter => {
                    self.vendor = if self.text_input.is_empty() {
                        None
                    } else {
                        Some(self.text_input.clone())
                    };
                    self.text_input.clear();
                    self.confirm_value = false;
                    self.state = ReviewState::ConfirmRule;
                    HandleResult::Continue
                }
                _ => HandleResult::Continue,
            },
            ReviewState::ConfirmRule => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.confirm_value = true;
                    HandleResult::Continue
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.confirm_value = false;
                    HandleResult::Continue
                }
                KeyCode::Left | KeyCode::Right => {
                    self.confirm_value = !self.confirm_value;
                    HandleResult::Continue
                }
                KeyCode::Enter => {
                    if self.confirm_value {
                        // Prefill rule pattern with first 2 words
                        let txn = &self.flagged[self.current_txn];
                        let words: Vec<&str> = txn.description.split_whitespace().collect();
                        self.text_input = if words.len() >= 2 {
                            format!("{} {}", words[0], words[1])
                        } else {
                            words.first().unwrap_or(&"").to_string()
                        };
                        self.state = ReviewState::InputRulePattern;
                    } else {
                        return HandleResult::CommitAndAdvance;
                    }
                    HandleResult::Continue
                }
                _ => HandleResult::Continue,
            },
            ReviewState::InputRulePattern => match code {
                KeyCode::Char(c) => {
                    self.text_input.push(c);
                    HandleResult::Continue
                }
                KeyCode::Backspace => {
                    self.text_input.pop();
                    HandleResult::Continue
                }
                KeyCode::Enter => HandleResult::CommitAndAdvance,
                _ => HandleResult::Continue,
            },
        }
    }

    fn commit_review(&mut self, conn: &rusqlite::Connection) -> Result<()> {
        let txn = &self.flagged[self.current_txn];
        let cat_idx = self.selected_category_idx.unwrap();
        let cat = &self.categories[cat_idx];

        let create_rule = self.confirm_value;
        let rule_pattern = if create_rule && !self.text_input.is_empty() {
            Some(self.text_input.as_str())
        } else {
            None
        };

        apply_review(
            conn,
            txn.id,
            cat.id,
            self.vendor.as_deref(),
            create_rule,
            rule_pattern,
        )?;

        self.advance();
        Ok(())
    }

    fn advance(&mut self) {
        self.current_txn += 1;
        self.state = ReviewState::PickCategory;
        self.cat_query.clear();
        self.cat_selection = 0;
        self.text_input.clear();
        self.confirm_value = false;
        self.selected_category_idx = None;
        self.vendor = None;
    }

    fn is_done(&self) -> bool {
        self.current_txn >= self.flagged.len()
    }
}

enum HandleResult {
    Continue,
    CommitAndAdvance,
    Done,
}

impl HandleResult {
    fn check_done(reviewer: &TransactionReviewer) -> Self {
        if reviewer.is_done() {
            HandleResult::Done
        } else {
            HandleResult::Continue
        }
    }
}

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let flagged = get_flagged_transactions(&conn)?;

    if flagged.is_empty() {
        println!("No flagged transactions to review.");
        return Ok(());
    }

    let categories = get_categories(&conn)?;
    let total = flagged.len();
    println!("{total} transactions to review");

    let mut reviewer = TransactionReviewer::new(flagged, categories);
    let mut terminal = ratatui::init();

    let result = loop {
        terminal.draw(|frame| reviewer.draw(frame)).unwrap();

        if let Event::Key(key) = event::read().unwrap() {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break Ok(());
            }

            match reviewer.handle_key(key.code) {
                HandleResult::Continue => {}
                HandleResult::CommitAndAdvance => {
                    if let Err(e) = reviewer.commit_review(&conn) {
                        break Err(e);
                    }
                    if reviewer.is_done() {
                        break Ok(());
                    }
                }
                HandleResult::Done => break Ok(()),
            }
        }
    };

    ratatui::restore();

    match &result {
        Ok(()) => println!("Review complete!"),
        Err(e) => eprintln!("Review error: {e}"),
    }
    result
}

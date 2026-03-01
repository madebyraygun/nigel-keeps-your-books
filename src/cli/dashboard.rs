use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph},
    Frame,
};

use crate::browser::{BrowseAction, RegisterBrowser};
use crate::cli::account_manager::{AccountAction, AccountManager};
use crate::cli::import_manager::{ImportAction, ImportScreen};
use crate::cli::load_manager::{LoadAction, LoadScreen};
use crate::cli::reconcile_manager::{ReconcileAction, ReconcileScreen};
use crate::cli::review::{HandleResult, TransactionReviewer};
use crate::cli::rules_manager::{RulesAction, RulesManager};
use crate::cli::snake::{SnakeAction, SnakeGame};
use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::number;
use crate::reports;
use crate::reviewer::{get_categories, get_flagged_transactions};
use crate::settings::{get_data_dir, load_settings, save_settings, settings_file_exists};
use crate::tui::{money_span, ReportView, ReportViewAction, FOOTER_STYLE, HEADER_STYLE};

const GREETINGS: &[&str] = &[
    "Kettle's on.",
    "Right then, let's have a look at the numbers.",
    "Lovely day to reconcile, innit?",
    "Books won't balance themselves.",
    "Back again? Brilliant.",
    "Another day, another CSV.",
    "Shall we see where the money's gone?",
    "Pull up a chair.",
    "Everything's in order. Well, mostly.",
    "No surprises today. Well... let's not get ahead of ourselves.",
    "Fancy a quick look at the numbers?",
    "Ah, there you are.",
    "The spreadsheets send their regards.",
    "Right then, where were we?",
];

const MENU_ITEMS: &[(&str, char)] = &[
    ("[b] Browse the register", 'b'),
    ("[i] Import a statement", 'i'),
    ("[r] Review flagged transactions", 'r'),
    ("[c] Reconcile an account", 'c'),
    ("[a] Add or modify accounts", 'a'),
    ("[u] View or edit categorization rules", 'u'),
    ("[v] View a report", 'v'),
    ("[e] Export a report", 'e'),
    ("[l] Load a different data file", 'l'),
    ("[s] Snake", 's'),
];

/// Number of menu items in the left column; remainder goes in the right column.
const MENU_LEFT_COUNT: usize = 5;

const REPORT_TYPES: &[&str] = &[
    "Profit & Loss",
    "Expense Breakdown",
    "Tax Summary",
    "Cash Flow",
    "Transaction Register",
    "Flagged Transactions",
    "Cash Position",
    "K-1 Prep (1120-S)",
];

const EXPORT_TYPES: &[&str] = &[
    "Profit & Loss",
    "Expense Breakdown",
    "Tax Summary",
    "Cash Flow",
    "Transaction Register",
    "Flagged Transactions",
    "Cash Position",
    "K-1 Prep (1120-S)",
    "All Reports",
];

#[derive(Clone, Copy)]
enum ReportPickerMode {
    View,
    Export,
}

enum DashboardScreen {
    Home,
    Browse(RegisterBrowser),
    Import(ImportScreen),
    Review(TransactionReviewer),
    Accounts(AccountManager),
    Rules(RulesManager),
    Reconcile(ReconcileScreen),
    Load(LoadScreen),
    ReportPicker { selection: usize, mode: ReportPickerMode },
    ReportView(Box<dyn ReportView>),
    Snake(SnakeGame),
}

struct HomeData {
    total_income: f64,
    total_expenses: f64,
    net: f64,
    txn_count: i64,
    flagged_count: usize,
    balances: Vec<(String, f64)>,
    cashflow_labels: Vec<String>,
    cashflow_income: Vec<u64>,
    cashflow_expenses: Vec<u64>,
    top_expenses: Vec<(String, f64)>,
}

struct Dashboard {
    screen: DashboardScreen,
    greeting: String,
    menu_selection: usize,
    home_data: Option<HomeData>,
    pending_report_view: Option<usize>,
    pending_export: Option<usize>,
    status_message: Option<String>,
    needs_reload: bool,
    /// Tracks which report index is currently displayed (for reload on date change)
    current_report_idx: Option<usize>,
}

impl Dashboard {
    fn new(user_name: Option<String>) -> Self {
        let mut rng = rand::thread_rng();
        let random_greeting = GREETINGS
            .choose(&mut rng)
            .unwrap_or(&"Hello.")
            .to_string();
        let first_name = user_name
            .as_deref()
            .and_then(|n| n.split_whitespace().next())
            .unwrap_or("");
        let greeting = if first_name.is_empty() {
            format!("Nigel: {random_greeting}")
        } else {
            format!("Hello, {first_name}. {random_greeting}")
        };
        Self {
            screen: DashboardScreen::Home,
            greeting,
            menu_selection: 0,
            home_data: None,
            pending_report_view: None,
            pending_export: None,
            status_message: None,
            needs_reload: false,
            current_report_idx: None,
        }
    }

    fn load_data(&mut self, conn: &rusqlite::Connection) -> Result<()> {
        let year = chrono::Local::now().year();

        let pnl = reports::get_pnl(conn, Some(year), None, None, None)?;
        let balance = reports::get_balance(conn)?;
        let cashflow = reports::get_cashflow(conn, Some(year), None)?;
        let flagged = reports::get_flagged(conn)?;

        let txn_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;

        let balances: Vec<(String, f64)> = balance
            .accounts
            .iter()
            .map(|a| (a.name.clone(), a.balance))
            .collect();

        let cashflow_labels: Vec<String> = cashflow
            .months
            .iter()
            .map(|m| {
                let parts: Vec<&str> = m.month.split('-').collect();
                if parts.len() == 2 {
                    match parts[1] {
                        "01" => "Jan",
                        "02" => "Feb",
                        "03" => "Mar",
                        "04" => "Apr",
                        "05" => "May",
                        "06" => "Jun",
                        "07" => "Jul",
                        "08" => "Aug",
                        "09" => "Sep",
                        "10" => "Oct",
                        "11" => "Nov",
                        "12" => "Dec",
                        _ => &m.month,
                    }
                    .to_string()
                } else {
                    m.month.clone()
                }
            })
            .collect();

        let cashflow_income: Vec<u64> = cashflow
            .months
            .iter()
            .map(|m| m.inflows.max(0.0) as u64)
            .collect();

        let cashflow_expenses: Vec<u64> = cashflow
            .months
            .iter()
            .map(|m| m.outflows.abs().max(0.0) as u64)
            .collect();

        // Top 5 expense categories (pnl.expenses is sorted by total ASC, most negative first)
        let top_expenses: Vec<(String, f64)> = pnl
            .expenses
            .iter()
            .take(5)
            .map(|e| (e.name.clone(), e.total.abs()))
            .collect();

        self.home_data = Some(HomeData {
            total_income: pnl.total_income,
            total_expenses: pnl.total_expenses,
            net: pnl.net,
            txn_count,
            flagged_count: flagged.len(),
            balances,
            cashflow_labels,
            cashflow_income,
            cashflow_expenses,
            top_expenses,
        });
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        if let DashboardScreen::Browse(ref mut browser) = self.screen {
            browser.draw_frame(frame);
            return;
        }
        if let DashboardScreen::Import(ref import) = self.screen {
            import.draw(frame);
            return;
        }
        if let DashboardScreen::Review(ref reviewer) = self.screen {
            reviewer.draw(frame);
            return;
        }
        if let DashboardScreen::Accounts(ref manager) = self.screen {
            manager.draw(frame);
            return;
        }
        if let DashboardScreen::Rules(ref mut rules) = self.screen {
            rules.draw(frame);
            return;
        }
        if let DashboardScreen::Reconcile(ref reconcile) = self.screen {
            reconcile.draw(frame);
            return;
        }
        if let DashboardScreen::Load(ref load) = self.screen {
            load.draw(frame);
            return;
        }
        if let DashboardScreen::ReportView(ref mut view) = self.screen {
            view.draw(frame);
            return;
        }
        if let DashboardScreen::ReportPicker { selection, mode } = self.screen {
            let (title, items) = match mode {
                ReportPickerMode::View => ("Select a report to view", REPORT_TYPES as &[&str]),
                ReportPickerMode::Export => ("Select a report to export", EXPORT_TYPES as &[&str]),
            };
            self.draw_picker(frame, title, items, selection);
            return;
        }
        if let DashboardScreen::Snake(ref mut game) = self.screen {
            game.draw(frame);
            return;
        }
        self.draw_home(frame);
    }

    fn draw_home(&self, frame: &mut Frame) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        let menu_rows = MENU_LEFT_COUNT as u16 + 1;

        let [header_area, sep1, stats_area, sep2, charts_area, sep3, menu_area, hints_area] =
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(menu_rows),
                Constraint::Length(1),
            ])
            .areas(area);

        // Header
        frame.render_widget(
            Paragraph::new(format!(" {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        // Thick separator lines
        let sep_line = "━".repeat(area.width as usize);
        let sep_widget = Paragraph::new(sep_line.as_str()).style(border_style);
        frame.render_widget(sep_widget.clone(), sep1);
        frame.render_widget(sep_widget.clone(), sep2);
        frame.render_widget(sep_widget.clone(), sep3);

        if let Some(data) = &self.home_data {
            // Stats + Balances — same 50/50 split used for charts below
            let [left_area, right_area] = Layout::horizontal([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .areas(stats_area);

            // YTD summary — 1-space indent to align with "N" in " Nigel:"
            let stats_lines = vec![
                Line::from(vec![
                    Span::raw(" YTD Income     "),
                    money_span(data.total_income),
                ]),
                Line::from(vec![
                    Span::raw(" YTD Expenses   "),
                    money_span(data.total_expenses),
                ]),
                Line::from(vec![
                    Span::raw(" Net Profit     "),
                    money_span(data.net),
                ]),
                Line::from(format!(
                    " Transactions   {}",
                    number(data.txn_count)
                )),
                Line::from(format!(" Flagged        {}", data.flagged_count)),
            ];
            frame.render_widget(Paragraph::new(stats_lines), left_area);

            // Account balances
            let mut balance_lines = vec![Line::from(Span::styled(
                " Account Balances",
                Style::default().add_modifier(Modifier::BOLD),
            ))];
            for (name, bal) in &data.balances {
                balance_lines.push(Line::from(vec![
                    Span::raw(format!(" {:<20}", name)),
                    money_span(*bal),
                ]));
            }
            frame.render_widget(Paragraph::new(balance_lines), right_area);

            // Charts — same 50/50 split so right column aligns with Account Balances
            let [chart_left, chart_right] = Layout::horizontal([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .areas(charts_area);

            // Monthly Cash Flow bar chart with y-axis labels
            if !data.cashflow_labels.is_empty() {
                let income_style = Style::default().fg(crate::tui::GREEN);
                let expense_style = Style::default().fg(Color::Red);

                // Pick round y-axis tick values based on max data value
                let max_val = data
                    .cashflow_income
                    .iter()
                    .chain(data.cashflow_expenses.iter())
                    .copied()
                    .max()
                    .unwrap_or(1) as f64;

                // Round ticks: pick nice round numbers for the axis
                let (top_tick, mid_tick) = y_axis_ticks(max_val);
                let top_label = format_k(top_tick);
                let mid_label = format_k(mid_tick);
                let y_label_width = top_label.len().max(mid_label.len()) as u16 + 1;

                let [y_axis_area, bar_area] = Layout::horizontal([
                    Constraint::Length(y_label_width),
                    Constraint::Fill(1),
                ])
                .areas(chart_left);

                // Y-axis labels: top tick near top, mid tick at middle
                let inner_height = bar_area.height.saturating_sub(2); // title + month labels
                let mid_row = inner_height / 2;
                let mut y_lines: Vec<Line> = Vec::new();
                y_lines.push(Line::from("")); // title row
                for row in 0..inner_height {
                    if row == 0 {
                        y_lines.push(Line::from(Span::styled(
                            format!("{:>width$}", top_label, width = y_label_width as usize),
                            FOOTER_STYLE,
                        )));
                    } else if row == mid_row {
                        y_lines.push(Line::from(Span::styled(
                            format!("{:>width$}", mid_label, width = y_label_width as usize),
                            FOOTER_STYLE,
                        )));
                    } else {
                        y_lines.push(Line::from(""));
                    }
                }
                frame.render_widget(Paragraph::new(y_lines), y_axis_area);

                let groups: Vec<BarGroup> = data
                    .cashflow_labels
                    .iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let inc = data.cashflow_income.get(i).copied().unwrap_or(0);
                        let exp = data.cashflow_expenses.get(i).copied().unwrap_or(0);
                        let bars = vec![
                            Bar::default().value(inc).style(income_style),
                            Bar::default().value(exp).style(expense_style),
                        ];
                        BarGroup::default()
                            .label(Line::from(label.as_str()))
                            .bars(&bars)
                    })
                    .collect();

                let block = Block::default()
                    .title("Monthly Cash Flow")
                    .title_style(Style::default().add_modifier(Modifier::BOLD))
                    .borders(Borders::NONE);

                let mut chart = BarChart::default()
                    .block(block)
                    .bar_width(2)
                    .bar_gap(0)
                    .group_gap(1);
                for group in &groups {
                    chart = chart.data(group.clone());
                }
                frame.render_widget(chart, bar_area);
            }

            // Top Expenses — simple text table (no bars)
            if !data.top_expenses.is_empty() {
                let name_width = data
                    .top_expenses
                    .iter()
                    .map(|(n, _)| n.len())
                    .max()
                    .unwrap_or(10);

                let mut lines = vec![Line::from(Span::styled(
                    " Top Expenses",
                    Style::default().add_modifier(Modifier::BOLD),
                ))];
                for (name, val) in &data.top_expenses {
                    lines.push(Line::from(vec![
                        Span::raw(format!(" {:<width$}  ", name, width = name_width)),
                        money_span(-val), // negative to show as expense (red)
                    ]));
                }
                frame.render_widget(Paragraph::new(lines), chart_right);
            }
        }

        // Command menu — 2 columns
        let flagged_count = self
            .home_data
            .as_ref()
            .map(|d| d.flagged_count)
            .unwrap_or(0);

        let [menu_title_area, menu_cols_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(menu_area);

        frame.render_widget(
            Paragraph::new(Span::styled(
                " What would you like to do?",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            menu_title_area,
        );

        let [menu_left, menu_right] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .areas(menu_cols_area);

        let left_lines: Vec<Line> = (0..MENU_LEFT_COUNT)
            .map(|i| self.menu_item_line(i, flagged_count))
            .collect();
        frame.render_widget(Paragraph::new(left_lines), menu_left);

        let right_lines: Vec<Line> = (MENU_LEFT_COUNT..MENU_ITEMS.len())
            .map(|i| self.menu_item_line(i, flagged_count))
            .collect();
        frame.render_widget(Paragraph::new(right_lines), menu_right);

        // Hints / status message
        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(" Up/Down=navigate  Enter=select  F5=refresh  q=quit")
                    .style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    fn draw_picker(&self, frame: &mut Frame, title: &str, items: &[&str], selection: usize) {
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
        for (i, item) in items.iter().enumerate() {
            let marker = if i == selection { ">" } else { " " };
            let style = if i == selection {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!(" {marker} {item}"),
                style,
            )));
        }
        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Up/Down=navigate  Enter=select  Esc=back  q=quit").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn menu_item_line(&self, i: usize, flagged_count: usize) -> Line<'static> {
        let marker = if i == self.menu_selection { ">" } else { " " };
        let (item, _) = MENU_ITEMS[i];
        let label = if i == 2 {
            format!(" {marker} {item} ({flagged_count})")
        } else {
            format!(" {marker} {item}")
        };
        let style = if i == self.menu_selection {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(Span::styled(label, style))
    }

    fn activate_menu_item(&mut self, idx: usize, conn: &rusqlite::Connection) {
        match idx {
            0 => self.screen = self.enter_browse(conn),
            1 => self.screen = DashboardScreen::Import(ImportScreen::new(conn, &self.greeting)),
            2 => self.screen = self.enter_review(conn),
            3 => self.screen = DashboardScreen::Reconcile(ReconcileScreen::new(conn, &self.greeting)),
            4 => self.screen = DashboardScreen::Accounts(AccountManager::new(conn, &self.greeting)),
            5 => self.screen = DashboardScreen::Rules(RulesManager::new(conn, &self.greeting)),
            6 => self.screen = DashboardScreen::ReportPicker { selection: 0, mode: ReportPickerMode::View },
            7 => self.screen = DashboardScreen::ReportPicker { selection: 0, mode: ReportPickerMode::Export },
            8 => self.screen = DashboardScreen::Load(LoadScreen::new(&self.greeting)),
            9 => self.screen = DashboardScreen::Snake(SnakeGame::new()),
            _ => {}
        }
    }

    fn handle_home_key(&mut self, code: KeyCode, conn: &rusqlite::Connection) -> bool {
        self.status_message = None;
        match code {
            KeyCode::Up => {
                self.menu_selection = self.menu_selection.saturating_sub(1);
            }
            KeyCode::Down => {
                self.menu_selection = (self.menu_selection + 1).min(MENU_ITEMS.len() - 1);
            }
            KeyCode::Char('q') => return true,
            KeyCode::Enter => self.activate_menu_item(self.menu_selection, conn),
            KeyCode::Char(ch) => {
                if let Some(idx) = MENU_ITEMS.iter().position(|(_, key)| *key == ch) {
                    self.activate_menu_item(idx, conn);
                }
            }
            _ => {}
        }
        false
    }

    fn enter_browse(&mut self, conn: &rusqlite::Connection) -> DashboardScreen {
        match reports::get_register(conn, None, None, None, None, None) {
            Ok(data) => {
                let categories = match get_categories(conn) {
                    Ok(c) => c,
                    Err(_) => vec![],
                };
                self.status_message = None;
                let mut browser = RegisterBrowser::new(
                    data.rows,
                    data.total,
                    "all transactions".to_string(),
                    categories,
                );
                browser.scroll_to_today();
                DashboardScreen::Browse(browser)
            }
            Err(e) => {
                self.status_message = Some(format!("Could not load register: {e}"));
                DashboardScreen::Home
            }
        }
    }

    fn enter_review(&mut self, conn: &rusqlite::Connection) -> DashboardScreen {
        let flagged = match get_flagged_transactions(conn) {
            Ok(f) => f,
            Err(e) => {
                self.status_message = Some(format!("Could not load flagged transactions: {e}"));
                return DashboardScreen::Home;
            }
        };
        if flagged.is_empty() {
            self.status_message = Some("No flagged transactions to review.".to_string());
            return DashboardScreen::Home;
        }
        let categories = match get_categories(conn) {
            Ok(c) => c,
            Err(e) => {
                self.status_message = Some(format!("Could not load categories: {e}"));
                return DashboardScreen::Home;
            }
        };
        self.status_message = None;
        DashboardScreen::Review(TransactionReviewer::new(flagged, categories))
    }

    fn enter_report_view(&mut self, idx: usize, conn: &rusqlite::Connection) -> DashboardScreen {
        self.enter_report_view_with_date(idx, conn, None, None)
    }

    fn enter_report_view_with_date(
        &mut self,
        idx: usize,
        conn: &rusqlite::Connection,
        year: Option<i32>,
        month: Option<String>,
    ) -> DashboardScreen {
        // Register (idx 4) delegates to the interactive browser
        if idx == 4 {
            return self.enter_browse(conn);
        }
        let year = year.or_else(|| Some(chrono::Local::now().year()));
        let result = match idx {
            0 => super::report::view::build_pnl(month.clone(), year, None, None),
            1 => super::report::view::build_expenses(month.clone(), year),
            2 => super::report::view::build_tax(year),
            3 => super::report::view::build_cashflow(month.clone(), year),
            5 => super::report::view::build_flagged(),
            6 => super::report::view::build_balance(),
            7 => super::report::view::build_k1(year),
            _ => return DashboardScreen::Home,
        };
        match result {
            Ok(view) => {
                self.status_message = None;
                DashboardScreen::ReportView(view)
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {e}"));
                DashboardScreen::Home
            }
        }
    }
}

/// Pick nice round y-axis tick values (top and mid) given a max data value.
fn y_axis_ticks(max_val: f64) -> (f64, f64) {
    // Round steps: 1k, 2.5k, 5k, 10k, 25k, 50k, 100k, 250k, ...
    let steps = [
        1000.0, 2500.0, 5000.0, 10000.0, 25000.0, 50000.0, 100000.0, 250000.0, 500000.0,
        1000000.0, 2500000.0, 5000000.0, 10000000.0,
    ];
    let top = steps
        .iter()
        .copied()
        .find(|&s| s >= max_val)
        .unwrap_or(max_val);
    let mid = top / 2.0;
    (top, mid)
}

/// Format a dollar amount as compact "$Xk" or "$X.Xk" for thousands, "$XM" for millions.
fn format_k(val: f64) -> String {
    if val >= 1_000_000.0 {
        let m = val / 1_000_000.0;
        if m == m.floor() {
            format!("${}M", m as u64)
        } else {
            format!("${:.1}M", m)
        }
    } else if val >= 1000.0 {
        let k = val / 1000.0;
        if k == k.floor() {
            format!("${}k", k as u64)
        } else {
            format!("${:.1}k", k)
        }
    } else {
        format!("${}", val as u64)
    }
}

fn do_export(idx: usize, year: Option<i32>, month: Option<String>) -> Result<String> {
    #[cfg(not(feature = "pdf"))]
    {
        let _ = (idx, year, month);
        return Err(crate::error::NigelError::Other(
            "PDF export requires the 'pdf' feature".into(),
        ));
    }
    #[cfg(feature = "pdf")]
    {
        let year = year.or_else(|| Some(chrono::Local::now().year()));
        let path = match idx {
            0 => super::export::pnl(month.clone(), year, None, None, None)?,
            1 => super::export::expenses(month.clone(), year, None)?,
            2 => super::export::tax(year, None)?,
            3 => super::export::cashflow(month.clone(), year, None)?,
            4 => super::export::register(month.clone(), year, None, None, None, None)?,
            5 => super::export::flagged(None)?,
            6 => super::export::balance(None)?,
            7 => super::export::k1(year, None)?,
            8 => return super::export::all(year, None),
            _ => return Ok(String::new()),
        };
        Ok(format!("Exported {path}"))
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<()> {
    // Returning users: show splash screen before dashboard
    let is_first_run = !settings_file_exists();
    if !is_first_run {
        super::splash::run()?;
    }

    // First-run: show onboarding, then ensure data dir + DB exist
    let mut post_setup_action = None;
    let mut onboarding_company = None;
    if is_first_run {
        if let Some(result) = super::onboarding::run()? {
            let mut settings = load_settings();
            if !result.user_name.is_empty() {
                settings.user_name = result.user_name;
            }
            save_settings(&settings)?;

            if !result.company_name.is_empty() {
                onboarding_company = Some(result.company_name);
            }
            post_setup_action = Some(result.action);
        }
    }

    // Ensure data dir and database exist (like `nigel init`)
    let settings = load_settings();
    let data_dir = std::path::PathBuf::from(&settings.data_dir);
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(data_dir.join("exports"))?;
    std::fs::create_dir_all(data_dir.join("snapshots"))?;
    std::fs::create_dir_all(data_dir.join("backups"))?;
    let conn = crate::db::get_connection(&data_dir.join("nigel.db"))?;
    crate::db::init_db(&conn)?;

    // Save company_name from onboarding to DB metadata
    if let Some(company) = onboarding_company {
        crate::db::set_metadata(&conn, "company_name", &company)?;
    }

    // Migrate legacy company_name from settings.json → DB metadata
    if crate::db::get_metadata(&conn, "company_name").is_none() {
        if let Some(company) = crate::settings::migrate_company_name() {
            crate::db::set_metadata(&conn, "company_name", &company)?;
        }
    }

    drop(conn);

    // Handle post-setup action from onboarding
    if let Some(action) = post_setup_action {
        match action {
            super::onboarding::PostSetupAction::Demo => {
                super::demo::setup_demo()?;
            }
            super::onboarding::PostSetupAction::Import => {
                // User chose "Load existing" during onboarding — prompt for path
                print!("Current data directory: {}\nPath to data directory: ", get_data_dir().display());
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                let path = input.trim();
                if !path.is_empty() {
                    super::load::run(path)?;
                }
            }
            super::onboarding::PostSetupAction::StartFresh => {
                // Nothing extra — DB is already initialized above
            }
        }
    }

    let user_name = if settings.user_name.is_empty() {
        None
    } else {
        Some(settings.user_name.clone())
    };

    loop {
        let conn = get_connection(&get_data_dir().join("nigel.db"))?;
        let mut dashboard = Dashboard::new(user_name.clone());
        dashboard.load_data(&conn)?;

        let mut terminal = ratatui::init();

        let exit: std::result::Result<bool, crate::error::NigelError> = loop {
            if let Err(e) = terminal.draw(|frame| dashboard.draw(frame)) {
                break Err(e.into());
            }

            if let DashboardScreen::Snake(ref mut game) = dashboard.screen {
                let timeout = game.tick_rate();
                match crossterm::event::poll(timeout) {
                    Ok(true) => {
                        // Key is available, fall through to event::read() below
                    }
                    Ok(false) => {
                        // No input within timeout — advance game tick
                        game.do_tick();
                        continue;
                    }
                    Err(e) => break Err(e.into()),
                }
            }

            match event::read() {
                Err(e) => break Err(e.into()),
                Ok(Event::Key(key)) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break Ok(true);
                    }

                    let mut return_home = false;
                    let mut pending_reload: Option<(usize, Option<i32>, Option<String>)> = None;
                    let should_quit = match &mut dashboard.screen {
                        DashboardScreen::Home => {
                            if key.code == KeyCode::F(5) {
                                let _ = dashboard.load_data(&conn);
                                false
                            } else {
                                dashboard.handle_home_key(key.code, &conn)
                            }
                        }
                        DashboardScreen::Import(ref mut import) => {
                            match import.handle_key(key.code, &conn) {
                                ImportAction::Close => {
                                    return_home = true;
                                }
                                ImportAction::Continue => {}
                            }
                            false
                        }
                        DashboardScreen::Browse(browser) => {
                            match browser.handle_key_event(key.code) {
                                BrowseAction::Close => {
                                    return_home = true;
                                }
                                BrowseAction::Continue => {}
                                BrowseAction::CommitEdit => {
                                    if let Err(e) = browser.commit_edit(&conn) {
                                        browser.set_status(format!("Edit failed: {e}"));
                                    }
                                }
                                BrowseAction::ToggleFlag => {
                                    if let Err(e) = browser.toggle_flag(&conn) {
                                        browser.set_status(format!("Flag toggle failed: {e}"));
                                    }
                                }
                            }
                            false
                        }
                        DashboardScreen::Review(reviewer) => {
                            match reviewer.handle_key(key.code) {
                                HandleResult::Continue => {}
                                HandleResult::CommitAndAdvance => {
                                    if let Err(e) = reviewer.commit_review(&conn) {
                                        break Err(e);
                                    }
                                    if reviewer.is_done() {
                                        return_home = true;
                                    }
                                }
                                HandleResult::UndoPrevious => {
                                    if let Err(e) = reviewer.undo_previous(&conn) {
                                        break Err(e);
                                    }
                                }
                                HandleResult::Done => {
                                    return_home = true;
                                }
                            }
                            false
                        }
                        DashboardScreen::Accounts(ref mut manager) => {
                            match manager.handle_key(key.code, &conn) {
                                AccountAction::Close => {
                                    return_home = true;
                                }
                                AccountAction::Continue => {}
                            }
                            false
                        }
                        DashboardScreen::Rules(ref mut rules) => {
                            match rules.handle_key(key.code, &conn) {
                                RulesAction::Close => {
                                    return_home = true;
                                }
                                RulesAction::Continue => {}
                            }
                            false
                        }
                        DashboardScreen::Reconcile(ref mut reconcile) => {
                            match reconcile.handle_key(key.code, &conn) {
                                ReconcileAction::Close => {
                                    return_home = true;
                                }
                                ReconcileAction::Continue => {}
                            }
                            false
                        }
                        DashboardScreen::Load(ref mut load) => {
                            match load.handle_key(key.code) {
                                LoadAction::Close => {
                                    return_home = true;
                                }
                                LoadAction::Reload => {
                                    dashboard.needs_reload = true;
                                }
                                LoadAction::Continue => {}
                            }
                            false
                        }
                        DashboardScreen::ReportView(ref mut view) => {
                            let action = view.handle_key(key.code);
                            match action {
                                ReportViewAction::Close => {
                                    dashboard.current_report_idx = None;
                                    return_home = true;
                                }
                                ReportViewAction::Continue => {}
                                ReportViewAction::Reload => {
                                    // Stash reload info; handled below after borrow ends
                                    if let Some(idx) = dashboard.current_report_idx {
                                        let (year, month) = view.date_params();
                                        pending_reload = Some((idx, year, month));
                                    }
                                }
                            }
                            false
                        }
                        DashboardScreen::ReportPicker { selection, mode } => {
                            let max_idx = match mode {
                                ReportPickerMode::View => REPORT_TYPES.len() - 1,
                                ReportPickerMode::Export => EXPORT_TYPES.len() - 1,
                            };
                            match key.code {
                                KeyCode::Up => *selection = selection.saturating_sub(1),
                                KeyCode::Down => {
                                    *selection = (*selection + 1).min(max_idx)
                                }
                                KeyCode::Esc => return_home = true,
                                KeyCode::Enter => {
                                    match mode {
                                        ReportPickerMode::View => {
                                            dashboard.pending_report_view = Some(*selection);
                                        }
                                        ReportPickerMode::Export => {
                                            dashboard.pending_export = Some(*selection);
                                        }
                                    }
                                }
                                _ => {}
                            }
                            key.code == KeyCode::Char('q')
                        }
                        DashboardScreen::Snake(ref mut game) => {
                            match game.handle_key(key.code) {
                                SnakeAction::Quit => {
                                    return_home = true;
                                }
                                SnakeAction::Continue => {}
                            }
                            false
                        }
                    };

                    if let Some((idx, year, month)) = pending_reload {
                        dashboard.screen = dashboard.enter_report_view_with_date(
                            idx, &conn, year, month,
                        );
                    }

                    if return_home {
                        dashboard.screen = DashboardScreen::Home;
                        let _ = dashboard.load_data(&conn);
                    }

                    if let Some(idx) = dashboard.pending_report_view.take() {
                        dashboard.current_report_idx = Some(idx);
                        dashboard.screen = dashboard.enter_report_view(idx, &conn);
                    }

                    if let Some(idx) = dashboard.pending_export.take() {
                        // Use the current report view's date params if we have
                        // one active, otherwise default to current year
                        let (year, month) = if let DashboardScreen::ReportView(ref view) = dashboard.screen {
                            view.date_params()
                        } else {
                            (None, None)
                        };
                        match do_export(idx, year, month) {
                            Ok(msg) => dashboard.status_message = Some(msg),
                            Err(e) => dashboard.status_message = Some(format!("Export failed: {e}")),
                        }
                        dashboard.screen = DashboardScreen::Home;
                    }

                    if dashboard.needs_reload {
                        break Ok(false); // reload
                    }

                    if should_quit {
                        break Ok(true); // quit
                    }
                }
                _ => {}
            }
        };

        drop(terminal);
        ratatui::restore();

        match exit {
            Err(e) => return Err(e),
            Ok(true) => return Ok(()),  // quit
            Ok(false) => continue,       // reload (data directory changed)
        }
    }
}

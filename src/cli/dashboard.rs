use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph},
    Frame,
};

use crate::browser::RegisterBrowser;
use crate::cli::review::{HandleResult, TransactionReviewer};
use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reports;
use crate::reviewer::{get_categories, get_flagged_transactions};
use crate::settings::get_data_dir;
use crate::tui::{money_span, FOOTER_STYLE, HEADER_STYLE};

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
    "You're looking well. The books, less so.",
    "Ah, there you are.",
    "The spreadsheets send their regards.",
    "Right then, where were we?",
];

const MENU_ITEMS: &[&str] = &[
    "Browse the register",
    "Import a statement",
    "Review flagged transactions",
    "Reconcile an account",
    "View or edit categorization rules",
    "View a report",
    "Export a report",
];

/// Number of menu items in the left column; remainder goes in the right column.
const MENU_LEFT_COUNT: usize = 4;

enum DashboardScreen {
    Home,
    Browse(RegisterBrowser),
    Review(TransactionReviewer),
    Stub(&'static str),
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
}

impl Dashboard {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let greeting = GREETINGS
            .choose(&mut rng)
            .unwrap_or(&"Hello.")
            .to_string();
        Self {
            screen: DashboardScreen::Home,
            greeting,
            menu_selection: 0,
            home_data: None,
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
            .map(|m| m.inflows as u64)
            .collect();

        let cashflow_expenses: Vec<u64> = cashflow
            .months
            .iter()
            .map(|m| m.outflows.abs() as u64)
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
        if let DashboardScreen::Review(ref reviewer) = self.screen {
            reviewer.draw(frame);
            return;
        }
        if let DashboardScreen::Stub(label) = self.screen {
            self.draw_stub(frame, label);
            return;
        }
        self.draw_home(frame);
    }

    fn draw_home(&self, frame: &mut Frame) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        // Menu rows = max(left_count, right_count) + 1 for title
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

        // Header — branding + greeting
        frame.render_widget(
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        // Thick separator lines
        let sep_line = "━".repeat(area.width as usize);
        let sep_widget = Paragraph::new(sep_line.as_str()).style(border_style);
        frame.render_widget(sep_widget.clone(), sep1);
        frame.render_widget(sep_widget.clone(), sep2);
        frame.render_widget(sep_widget.clone(), sep3);

        if let Some(data) = &self.home_data {
            // Stats + Balances side by side
            let [left_area, right_area] = Layout::horizontal([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .areas(stats_area);

            // YTD summary
            let stats_lines = vec![
                Line::from(vec![
                    Span::raw("  YTD Income    "),
                    money_span(data.total_income),
                ]),
                Line::from(vec![
                    Span::raw("  YTD Expenses "),
                    money_span(data.total_expenses),
                ]),
                Line::from(vec![
                    Span::raw("  Net Profit    "),
                    money_span(data.net),
                ]),
                Line::from(format!(
                    "  Transactions  {}",
                    money(data.txn_count as f64)
                )),
                Line::from(format!("  Flagged       {}", data.flagged_count)),
            ];
            frame.render_widget(Paragraph::new(stats_lines), left_area);

            // Account balances
            let mut balance_lines = vec![Line::from(Span::styled(
                "  Account Balances",
                Style::default().add_modifier(Modifier::BOLD),
            ))];
            for (name, bal) in &data.balances {
                balance_lines.push(Line::from(vec![
                    Span::raw(format!("  {:<20}", name)),
                    money_span(*bal),
                ]));
            }
            frame.render_widget(Paragraph::new(balance_lines), right_area);

            // Charts side by side: bar chart (left) + top expenses (right)
            let [chart_left, chart_right] = Layout::horizontal([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .areas(charts_area);

            // Monthly Cash Flow bar chart
            if !data.cashflow_labels.is_empty() {
                let income_style = Style::default().fg(Color::Green);
                let expense_style = Style::default().fg(Color::Red);

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
                    .title(" Monthly Cash Flow ")
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
                frame.render_widget(chart, chart_left);
            }

            // Top Expenses by Category — horizontal bar chart
            if !data.top_expenses.is_empty() {
                let bar_data: Vec<Bar> = data
                    .top_expenses
                    .iter()
                    .map(|(name, val)| {
                        Bar::default()
                            .value(*val as u64)
                            .label(Line::from(name.as_str()))
                            .style(Style::default().fg(Color::Red))
                            .value_style(Style::default().fg(Color::White))
                            .text_value(money(*val))
                    })
                    .collect();

                let group = BarGroup::default().bars(&bar_data);

                let block = Block::default()
                    .title(" Top Expenses ")
                    .title_style(Style::default().add_modifier(Modifier::BOLD))
                    .borders(Borders::NONE);

                let chart = BarChart::default()
                    .block(block)
                    .direction(Direction::Horizontal)
                    .data(group)
                    .bar_width(1)
                    .bar_gap(0);
                frame.render_widget(chart, chart_right);
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
                "  What would you like to do?",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            menu_title_area,
        );

        let [menu_left, menu_right] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .areas(menu_cols_area);

        // Left column: items 0..MENU_LEFT_COUNT
        let left_lines: Vec<Line> = (0..MENU_LEFT_COUNT)
            .map(|i| self.menu_item_line(i, flagged_count))
            .collect();
        frame.render_widget(Paragraph::new(left_lines), menu_left);

        // Right column: items MENU_LEFT_COUNT..
        let right_lines: Vec<Line> = (MENU_LEFT_COUNT..MENU_ITEMS.len())
            .map(|i| self.menu_item_line(i, flagged_count))
            .collect();
        frame.render_widget(Paragraph::new(right_lines), menu_right);

        // Hints
        frame.render_widget(
            Paragraph::new(" Up/Down=navigate  Enter=select  r=refresh  q=quit")
                .style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn menu_item_line(&self, i: usize, flagged_count: usize) -> Line<'static> {
        let marker = if i == self.menu_selection { ">" } else { " " };
        let item = MENU_ITEMS[i];
        let label = if i == 2 {
            format!("  {marker} {item} ({flagged_count})")
        } else {
            format!("  {marker} {item}")
        };
        let style = if i == self.menu_selection {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(Span::styled(label, style))
    }

    fn draw_stub(&self, frame: &mut Frame, label: &str) {
        let area = frame.area();
        let [header_area, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );
        frame.render_widget(
            Paragraph::new(format!("  {label} — coming soon. Press Esc to go back.")),
            content_area,
        );
        frame.render_widget(
            Paragraph::new(" Esc=back  q=quit").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn handle_home_key(&mut self, code: KeyCode, conn: &rusqlite::Connection) -> bool {
        match code {
            KeyCode::Up => {
                self.menu_selection = self.menu_selection.saturating_sub(1);
            }
            KeyCode::Down => {
                self.menu_selection = (self.menu_selection + 1).min(MENU_ITEMS.len() - 1);
            }
            KeyCode::Char('q') => return true,
            KeyCode::Enter => {
                self.screen = match self.menu_selection {
                    0 => self.enter_browse(conn),
                    1 => DashboardScreen::Stub("Import a statement"),
                    2 => self.enter_review(conn),
                    3 => DashboardScreen::Stub("Reconcile an account"),
                    4 => DashboardScreen::Stub("View or edit categorization rules"),
                    5 => DashboardScreen::Stub("View a report"),
                    6 => DashboardScreen::Stub("Export a report"),
                    _ => DashboardScreen::Home,
                };
            }
            _ => {}
        }
        false
    }

    fn enter_browse(&self, conn: &rusqlite::Connection) -> DashboardScreen {
        let year = chrono::Local::now().year();
        match reports::get_register(conn, Some(year), None, None, None, None) {
            Ok(data) => {
                let browser =
                    RegisterBrowser::new(data.rows, data.total, format!("year: {year}"));
                DashboardScreen::Browse(browser)
            }
            Err(_) => DashboardScreen::Home,
        }
    }

    fn enter_review(&self, conn: &rusqlite::Connection) -> DashboardScreen {
        let flagged = match get_flagged_transactions(conn) {
            Ok(f) => f,
            Err(_) => return DashboardScreen::Home,
        };
        if flagged.is_empty() {
            return DashboardScreen::Home;
        }
        let categories = match get_categories(conn) {
            Ok(c) => c,
            Err(_) => return DashboardScreen::Home,
        };
        DashboardScreen::Review(TransactionReviewer::new(flagged, categories))
    }

    fn handle_stub_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.screen = DashboardScreen::Home;
            }
            KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }
}

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut dashboard = Dashboard::new();
    dashboard.load_data(&conn)?;

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let mut terminal = ratatui::init();

    let result = loop {
        if let Err(e) = terminal.draw(|frame| dashboard.draw(frame)) {
            break Err(e.into());
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
                    break Ok(());
                }

                let mut return_home = false;
                let should_quit = match &mut dashboard.screen {
                    DashboardScreen::Home => {
                        if key.code == KeyCode::Char('r') {
                            let _ = dashboard.load_data(&conn);
                            false
                        } else {
                            dashboard.handle_home_key(key.code, &conn)
                        }
                    }
                    DashboardScreen::Browse(browser) => {
                        return_home = browser.handle_key_event(key.code);
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
                    DashboardScreen::Stub(_) => dashboard.handle_stub_key(key.code),
                };
                if return_home {
                    dashboard.screen = DashboardScreen::Home;
                    let _ = dashboard.load_data(&conn);
                }

                if should_quit {
                    break Ok(());
                }
            }
            _ => {}
        }
    };

    ratatui::restore();
    result
}

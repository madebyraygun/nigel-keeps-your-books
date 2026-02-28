use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Paragraph},
    Frame,
};

use crate::browser::RegisterBrowser;
use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reports;
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

enum DashboardScreen {
    Home,
    Browse(RegisterBrowser),
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
                // Extract month abbreviation from "YYYY-MM" format
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

        self.home_data = Some(HomeData {
            total_income: pnl.total_income,
            total_expenses: pnl.total_expenses,
            net: pnl.net,
            txn_count,
            flagged_count: flagged.len(),
            balances,
            cashflow_labels,
            cashflow_income,
        });
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        if let DashboardScreen::Browse(ref mut browser) = self.screen {
            browser.draw_frame(frame);
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

        let [header_area, stats_area, chart_area, menu_area, hints_area] =
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(area);

        // Header — branding + greeting
        frame.render_widget(
            Paragraph::new(format!("Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

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
                    Span::raw(format!("  {:<16}", name)),
                    money_span(*bal),
                ]));
            }
            frame.render_widget(Paragraph::new(balance_lines), right_area);

            // Bar chart — income per month
            let bar_data: Vec<(&str, u64)> = data
                .cashflow_labels
                .iter()
                .zip(data.cashflow_income.iter())
                .map(|(label, val)| (label.as_str(), *val))
                .collect();
            if !bar_data.is_empty() {
                let chart = BarChart::default()
                    .data(&bar_data)
                    .bar_width(3)
                    .bar_gap(1)
                    .bar_style(Style::default().fg(Color::Green));
                frame.render_widget(chart, chart_area);
            }
        }

        // Command menu
        let flagged_count = self
            .home_data
            .as_ref()
            .map(|d| d.flagged_count)
            .unwrap_or(0);

        let mut menu_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  What would you like to do?",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        for (i, item) in MENU_ITEMS.iter().enumerate() {
            let marker = if i == self.menu_selection { ">" } else { " " };
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
            menu_lines.push(Line::from(Span::styled(label, style)));
        }
        frame.render_widget(Paragraph::new(menu_lines), menu_area);

        // Hints
        frame.render_widget(
            Paragraph::new(" Up/Down=navigate  Enter=select  r=refresh  q=quit")
                .style(FOOTER_STYLE),
            hints_area,
        );
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
            Paragraph::new(format!("Nigel: {}", self.greeting)).style(HEADER_STYLE),
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
                    2 => DashboardScreen::Stub("Review flagged transactions"),
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

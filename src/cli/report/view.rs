use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crossterm::event::KeyCode;

use crate::cli::{parse_month_opt, ReportCommands};
use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reports;
use crate::settings::get_data_dir;
use crate::tui::{
    money_span, ReportView, ReportViewAction, run_report_view, FOOTER_STYLE, HEADER_STYLE,
    AMOUNT_NEG_STYLE, AMOUNT_POS_STYLE,
};

// ---------------------------------------------------------------------------
// Date granularity support for report navigation
// ---------------------------------------------------------------------------

/// What date navigation granularities a report supports.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DateGranularity {
    /// Supports both month and year navigation (P&L, Expenses, Cash Flow)
    MonthAndYear,
    /// Supports only year navigation (Tax, K-1)
    YearOnly,
    /// No date navigation (Flagged, Balance)
    None,
}

/// Whether the view is currently showing a month or a full year.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PeriodMode {
    Year,
    Month,
}

const MONTH_NAMES: &[&str] = &[
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Dispatch a report command to an interactive ratatui view.
pub fn dispatch(cmd: ReportCommands) -> Result<()> {
    // Register delegates to the interactive RegisterBrowser (standalone)
    if matches!(cmd, ReportCommands::Register { .. }) {
        return register_standalone(cmd);
    }
    if matches!(cmd, ReportCommands::All { .. }) {
        return Err(crate::error::NigelError::Other(
            "`report all` is export-only — use `--mode export`".into(),
        ));
    }
    let mut view = build_view(&cmd)?;
    run_report_view(view.as_mut())
}

/// Build a report view from a command. Used by both CLI dispatch and dashboard.
/// Does NOT handle Register (which uses RegisterBrowser) or All (export-only).
pub(crate) fn build_view(cmd: &ReportCommands) -> Result<Box<dyn ReportView>> {
    match cmd {
        ReportCommands::Pnl { month, year, from_date, to_date, .. } => {
            build_pnl(month.clone(), *year, from_date.clone(), to_date.clone())
        }
        ReportCommands::Expenses { month, year, .. } => build_expenses(month.clone(), *year),
        ReportCommands::Tax { year, .. } => build_tax(*year),
        ReportCommands::Cashflow { month, year, .. } => build_cashflow(month.clone(), *year),
        ReportCommands::Flagged { .. } => build_flagged(),
        ReportCommands::Balance { .. } => build_balance(),
        ReportCommands::K1 { year, .. } => build_k1(*year),
        _ => Err(crate::error::NigelError::Other("Unsupported report for view mode".into())),
    }
}

// ---------------------------------------------------------------------------
// Table-based report view (shared by all report types)
// ---------------------------------------------------------------------------

const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
const SECTION_STYLE: Style = Style::new()
    .fg(Color::Yellow)
    .add_modifier(Modifier::BOLD);
const HEADER_ROW_STYLE: Style = Style::new()
    .fg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub(crate) struct TableReportView {
    title: String,
    header: Row<'static>,
    rows: Vec<Row<'static>>,
    widths: Vec<Constraint>,
    offset: usize,
    visible_count: usize,
    // Date navigation state
    granularity: DateGranularity,
    period_mode: PeriodMode,
    year: i32,
    month: u32, // 1-12
}

impl TableReportView {
    fn new(
        title: impl Into<String>,
        header: Row<'static>,
        rows: Vec<Row<'static>>,
        widths: Vec<Constraint>,
    ) -> Self {
        let now = chrono::Local::now();
        Self {
            title: title.into(),
            header,
            rows,
            widths,
            offset: 0,
            visible_count: 20,
            granularity: DateGranularity::None,
            period_mode: PeriodMode::Year,
            year: chrono::Datelike::year(&now),
            month: chrono::Datelike::month(&now),
        }
    }

    fn with_date(mut self, granularity: DateGranularity, year: i32, month: Option<u32>) -> Self {
        self.granularity = granularity;
        self.year = year;
        if let Some(m) = month {
            self.month = m;
            self.period_mode = PeriodMode::Month;
        } else {
            self.period_mode = PeriodMode::Year;
        }
        self
    }

    /// Returns the current date parameters: (year, optional month string like "2026-03").
    pub(crate) fn date_params(&self) -> (Option<i32>, Option<String>) {
        match self.granularity {
            DateGranularity::None => (None, None),
            DateGranularity::YearOnly => (Some(self.year), None),
            DateGranularity::MonthAndYear => match self.period_mode {
                PeriodMode::Year => (Some(self.year), None),
                PeriodMode::Month => (
                    Some(self.year),
                    Some(format!("{}-{:02}", self.year, self.month)),
                ),
            },
        }
    }

    fn period_label(&self) -> String {
        match self.granularity {
            DateGranularity::None => String::new(),
            DateGranularity::YearOnly => format!(" \u{2014} FY {}", self.year),
            DateGranularity::MonthAndYear => match self.period_mode {
                PeriodMode::Year => format!(" \u{2014} FY {}", self.year),
                PeriodMode::Month => {
                    let name = MONTH_NAMES
                        .get((self.month - 1) as usize)
                        .unwrap_or(&"???");
                    format!(" \u{2014} {} {}", name, self.year)
                }
            },
        }
    }

    fn navigate_period(&mut self, delta: i32) {
        match self.granularity {
            DateGranularity::None => {}
            DateGranularity::YearOnly => {
                self.year = (self.year + delta).max(2000);
            }
            DateGranularity::MonthAndYear => match self.period_mode {
                PeriodMode::Year => {
                    self.year = (self.year + delta).max(2000);
                }
                PeriodMode::Month => {
                    let mut m = self.month as i32 + delta;
                    let mut y = self.year;
                    while m < 1 {
                        m += 12;
                        y -= 1;
                    }
                    while m > 12 {
                        m -= 12;
                        y += 1;
                    }
                    self.month = m as u32;
                    self.year = y.max(2000);
                }
            },
        }
    }

    fn toggle_period_mode(&mut self) {
        if self.granularity == DateGranularity::MonthAndYear {
            self.period_mode = match self.period_mode {
                PeriodMode::Year => PeriodMode::Month,
                PeriodMode::Month => PeriodMode::Year,
            };
        }
    }
}

impl ReportView for TableReportView {
    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let [header_area, sep_area, content_area, footer_area] =
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(area);

        // Header (with period label for date-navigable reports)
        frame.render_widget(
            Paragraph::new(format!(" {}{}", self.title, self.period_label())).style(HEADER_STYLE),
            header_area,
        );

        // Separator
        frame.render_widget(
            Paragraph::new("━".repeat(area.width as usize)).style(FOOTER_STYLE),
            sep_area,
        );

        // Compute visible rows (header takes ~2 lines: header + bottom_margin)
        let header_overhead = 2u16;
        let visible = content_area.height.saturating_sub(header_overhead) as usize;
        self.visible_count = visible.max(1);

        let visible_rows: Vec<Row> = self
            .rows
            .iter()
            .skip(self.offset)
            .take(visible)
            .cloned()
            .collect();

        let table = Table::new(visible_rows, self.widths.clone())
            .header(self.header.clone())
            .column_spacing(2);

        frame.render_widget(table, content_area);

        // Footer
        let max = self.rows.len().saturating_sub(visible);
        let pos_info = if max > 0 {
            format!(
                "  line {}/{}",
                self.offset + 1,
                self.rows.len()
            )
        } else {
            String::new()
        };
        let nav_hint = match self.granularity {
            DateGranularity::MonthAndYear => "\u{2190}/\u{2192}=period  m=month/year  ",
            DateGranularity::YearOnly => "\u{2190}/\u{2192}=year  ",
            DateGranularity::None => "",
        };
        frame.render_widget(
            Paragraph::new(format!(
                " {nav_hint}\u{2191}/\u{2193}=scroll  q/Esc=close{pos_info}"
            ))
            .style(FOOTER_STYLE),
            footer_area,
        );
    }

    fn handle_key(&mut self, code: KeyCode) -> ReportViewAction {
        let page = self.visible_count;
        let max = self.rows.len().saturating_sub(page);
        match code {
            KeyCode::Char('q') | KeyCode::Esc => ReportViewAction::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.offset = self.offset.saturating_sub(1);
                ReportViewAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.offset = (self.offset + 1).min(max);
                ReportViewAction::Continue
            }
            KeyCode::PageUp => {
                self.offset = self.offset.saturating_sub(page);
                ReportViewAction::Continue
            }
            KeyCode::PageDown => {
                self.offset = (self.offset + page).min(max);
                ReportViewAction::Continue
            }
            KeyCode::Home => {
                self.offset = 0;
                ReportViewAction::Continue
            }
            KeyCode::End => {
                self.offset = max;
                ReportViewAction::Continue
            }
            KeyCode::Left if self.granularity != DateGranularity::None => {
                self.navigate_period(-1);
                self.offset = 0;
                ReportViewAction::Reload
            }
            KeyCode::Right if self.granularity != DateGranularity::None => {
                self.navigate_period(1);
                self.offset = 0;
                ReportViewAction::Reload
            }
            KeyCode::Char('m') if self.granularity == DateGranularity::MonthAndYear => {
                self.toggle_period_mode();
                self.offset = 0;
                ReportViewAction::Reload
            }
            _ => ReportViewAction::Continue,
        }
    }

    fn date_params(&self) -> (Option<i32>, Option<String>) {
        self.date_params()
    }
}

// ---------------------------------------------------------------------------
// Helper: create cells with consistent styling
// ---------------------------------------------------------------------------

fn money_cell(amount: f64) -> Cell<'static> {
    Cell::from(money_span(amount))
}

fn text_cell(s: impl Into<String>) -> Cell<'static> {
    Cell::from(s.into())
}

fn bold_cell(s: impl Into<String>) -> Cell<'static> {
    Cell::from(Span::styled(s.into(), BOLD))
}

fn section_row(label: &str, num_cols: usize) -> Row<'static> {
    let mut cells: Vec<Cell> = vec![Cell::from(Span::styled(
        label.to_string(),
        SECTION_STYLE,
    ))];
    for _ in 1..num_cols {
        cells.push(Cell::from(""));
    }
    Row::new(cells)
}

fn blank_row(num_cols: usize) -> Row<'static> {
    Row::new(vec![Cell::from(""); num_cols])
}

// ---------------------------------------------------------------------------
// Report builders
// ---------------------------------------------------------------------------

pub(crate) fn build_pnl(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_pnl(
        &conn,
        year.or(my),
        mm,
        from_date.as_deref(),
        to_date.as_deref(),
    )?;

    let widths = vec![Constraint::Fill(1), Constraint::Length(14)];
    let header = Row::new(["Category", "Amount"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    if !data.income.is_empty() {
        rows.push(section_row("INCOME", 2));
        for item in &data.income {
            rows.push(Row::new([
                text_cell(format!("  {}", item.name)),
                money_cell(item.total),
            ]));
        }
        rows.push(Row::new([
            bold_cell("  Total Income"),
            money_cell(data.total_income),
        ]));
        rows.push(blank_row(2));
    }

    if !data.expenses.is_empty() {
        rows.push(section_row("EXPENSES", 2));
        for item in &data.expenses {
            rows.push(Row::new([
                text_cell(format!("  {}", item.name)),
                money_cell(-item.total.abs()),
            ]));
        }
        rows.push(Row::new([
            bold_cell("  Total Expenses"),
            money_cell(-data.total_expenses.abs()),
        ]));
        rows.push(blank_row(2));
    }

    rows.push(Row::new([
        bold_cell("NET"),
        money_cell(data.net),
    ]));

    let effective_year = year.or(my).unwrap_or_else(|| chrono::Datelike::year(&chrono::Local::now()));
    Ok(Box::new(TableReportView::new(
        "Profit & Loss",
        header,
        rows,
        widths,
    ).with_date(DateGranularity::MonthAndYear, effective_year, mm.map(|m| m as u32))))
}

pub(crate) fn build_expenses(
    month: Option<String>,
    year: Option<i32>,
) -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_expense_breakdown(&conn, year.or(my), mm)?;

    let widths = vec![
        Constraint::Fill(1),
        Constraint::Length(14),
        Constraint::Length(8),
        Constraint::Length(8),
    ];
    let header = Row::new(["Category", "Amount", "%", "Count"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    for item in &data.categories {
        rows.push(Row::new([
            text_cell(&item.name),
            money_cell(-item.total.abs()),
            text_cell(format!("{:.1}%", item.pct)),
            text_cell(item.count.to_string()),
        ]));
    }

    rows.push(blank_row(4));
    rows.push(Row::new([
        bold_cell("Total"),
        money_cell(-data.total.abs()),
        Cell::from(""),
        Cell::from(""),
    ]));

    if !data.top_vendors.is_empty() {
        rows.push(blank_row(4));
        rows.push(section_row("TOP VENDORS", 4));
        rows.push(blank_row(4));
        // Sub-header for vendor section
        rows.push(
            Row::new([
                Cell::from(Span::styled("Vendor", HEADER_ROW_STYLE)),
                Cell::from(Span::styled("Amount", HEADER_ROW_STYLE)),
                Cell::from(Span::styled("Count", HEADER_ROW_STYLE)),
                Cell::from(""),
            ])
        );
        for v in &data.top_vendors {
            rows.push(Row::new([
                text_cell(&v.vendor),
                money_cell(-v.total.abs()),
                text_cell(v.count.to_string()),
                Cell::from(""),
            ]));
        }
    }

    let effective_year = year.or(my).unwrap_or_else(|| chrono::Datelike::year(&chrono::Local::now()));
    Ok(Box::new(TableReportView::new(
        "Expense Breakdown",
        header,
        rows,
        widths,
    ).with_date(DateGranularity::MonthAndYear, effective_year, mm.map(|m| m as u32))))
}

pub(crate) fn build_tax(year: Option<i32>) -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_tax_summary(&conn, year)?;

    let widths = vec![
        Constraint::Fill(1),
        Constraint::Length(20),
        Constraint::Length(12),
        Constraint::Length(14),
    ];
    let header = Row::new(["Category", "Tax Line", "Type", "Amount"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    for item in &data.line_items {
        let style = if item.category_type == "income" {
            AMOUNT_POS_STYLE
        } else {
            AMOUNT_NEG_STYLE
        };
        rows.push(Row::new([
            text_cell(&item.name),
            text_cell(item.tax_line.as_deref().unwrap_or("")),
            text_cell(&item.category_type),
            Cell::from(Span::styled(money(item.total.abs()), style)),
        ]));
    }

    let effective_year = year.unwrap_or_else(|| chrono::Datelike::year(&chrono::Local::now()));
    Ok(Box::new(TableReportView::new(
        "Tax Summary",
        header,
        rows,
        widths,
    ).with_date(DateGranularity::YearOnly, effective_year, None)))
}

pub(crate) fn build_cashflow(
    month: Option<String>,
    year: Option<i32>,
) -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_cashflow(&conn, year.or(my), mm)?;

    let widths = vec![
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let header = Row::new(["Month", "Inflows", "Outflows", "Net", "Running"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    for m in &data.months {
        rows.push(Row::new([
            text_cell(&m.month),
            Cell::from(Span::styled(money(m.inflows), AMOUNT_POS_STYLE)),
            Cell::from(Span::styled(money(m.outflows.abs()), AMOUNT_NEG_STYLE)),
            money_cell(m.net),
            text_cell(money(m.running_balance)),
        ]));
    }

    let effective_year = year.or(my).unwrap_or_else(|| chrono::Datelike::year(&chrono::Local::now()));
    Ok(Box::new(TableReportView::new(
        "Cash Flow",
        header,
        rows,
        widths,
    ).with_date(DateGranularity::MonthAndYear, effective_year, mm.map(|m| m as u32))))
}

pub(crate) fn build_flagged() -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_flagged(&conn)?;

    let widths = vec![
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Fill(1),
        Constraint::Length(12),
        Constraint::Length(20),
    ];
    let header = Row::new(["ID", "Date", "Description", "Amount", "Account"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    if data.is_empty() {
        rows.push(Row::new([
            Cell::from(""),
            Cell::from(""),
            text_cell("No flagged transactions."),
            Cell::from(""),
            Cell::from(""),
        ]));
    } else {
        for r in &data {
            rows.push(Row::new([
                text_cell(r.id.to_string()),
                text_cell(&r.date),
                text_cell(truncate(&r.description, 50)),
                money_cell(r.amount),
                text_cell(&r.account_name),
            ]));
        }
        rows.push(blank_row(5));
        rows.push(Row::new([
            bold_cell(format!("Total: {}", data.len())),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]));
    }

    Ok(Box::new(TableReportView::new(
        format!("Flagged Transactions ({})", data.len()),
        header,
        rows,
        widths,
    )))
}

pub(crate) fn build_balance() -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_balance(&conn)?;

    let widths = vec![
        Constraint::Fill(1),
        Constraint::Length(20),
        Constraint::Length(14),
    ];
    let header = Row::new(["Account", "Type", "Balance"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    for a in &data.accounts {
        rows.push(Row::new([
            text_cell(&a.name),
            text_cell(&a.account_type),
            money_cell(a.balance),
        ]));
    }

    rows.push(blank_row(3));
    rows.push(Row::new([
        bold_cell("Total"),
        Cell::from(""),
        money_cell(data.total),
    ]));
    rows.push(blank_row(3));
    rows.push(Row::new([
        bold_cell("YTD Net Income"),
        Cell::from(""),
        money_cell(data.ytd_net_income),
    ]));

    Ok(Box::new(TableReportView::new(
        "Cash Position",
        header,
        rows,
        widths,
    )))
}

pub(crate) fn build_k1(year: Option<i32>) -> Result<Box<dyn ReportView>> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_k1_prep(&conn, year)?;

    let widths = vec![
        Constraint::Fill(1),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let header = Row::new(["Item", "Detail", "Amount"])
        .style(HEADER_ROW_STYLE)
        .bottom_margin(1);

    let mut rows = Vec::new();

    // Income Summary
    rows.push(section_row("INCOME SUMMARY", 3));
    rows.push(Row::new([
        text_cell("  Gross Receipts"),
        Cell::from(""),
        money_cell(data.gross_receipts),
    ]));
    rows.push(Row::new([
        text_cell("  Other Income"),
        Cell::from(""),
        money_cell(data.other_income),
    ]));
    rows.push(Row::new([
        text_cell("  Total Deductions"),
        Cell::from(""),
        money_cell(data.total_deductions),
    ]));

    let obi_label = if data.ordinary_business_income >= 0.0 {
        "  Ordinary Business Income"
    } else {
        "  Ordinary Business Loss"
    };
    rows.push(Row::new([
        bold_cell(obi_label),
        Cell::from(""),
        money_cell(data.ordinary_business_income),
    ]));

    // Deductions by Line
    if !data.deduction_lines.is_empty() {
        rows.push(blank_row(3));
        rows.push(section_row("DEDUCTIONS BY LINE", 3));
        rows.push(Row::new([
            Cell::from(Span::styled("Line", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Category", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Amount", HEADER_ROW_STYLE)),
        ]));
        for item in &data.deduction_lines {
            rows.push(Row::new([
                text_cell(&item.form_line),
                text_cell(&item.category_name),
                money_cell(-item.total),
            ]));
        }
    }

    // Schedule K Items
    if !data.schedule_k_items.is_empty() {
        rows.push(blank_row(3));
        rows.push(section_row("SCHEDULE K", 3));
        rows.push(Row::new([
            Cell::from(Span::styled("Line", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Item", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Amount", HEADER_ROW_STYLE)),
        ]));
        for item in &data.schedule_k_items {
            rows.push(Row::new([
                text_cell(&item.form_line),
                text_cell(&item.category_name),
                money_cell(item.total.abs()),
            ]));
        }
    }

    // Line 19 — Other Deductions
    if !data.other_deductions.is_empty() {
        rows.push(blank_row(3));
        rows.push(section_row("LINE 19 \u{2014} OTHER DEDUCTIONS", 3));
        rows.push(Row::new([
            Cell::from(Span::styled("Category", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Full Amount", HEADER_ROW_STYLE)),
            Cell::from(Span::styled("Deductible", HEADER_ROW_STYLE)),
        ]));
        for item in &data.other_deductions {
            let note = if item.deductible < item.total {
                " (50%)"
            } else {
                ""
            };
            rows.push(Row::new([
                text_cell(format!("{}{}", item.category_name, note)),
                text_cell(money(item.total)),
                text_cell(money(item.deductible)),
            ]));
        }
        rows.push(Row::new([
            bold_cell("Total Other Deductions"),
            Cell::from(""),
            text_cell(money(data.other_deductions_total)),
        ]));
    }

    // Validation warnings
    if data.validation.uncategorized_count > 0 {
        rows.push(blank_row(3));
        let warn_style = Style::default().fg(Color::Yellow);
        rows.push(Row::new([
            Cell::from(Span::styled(
                format!(
                    "Warning: {} uncategorized transactions — run `nigel review`",
                    data.validation.uncategorized_count
                ),
                warn_style,
            )),
            Cell::from(""),
            Cell::from(""),
        ]));
    }
    if let Some(ratio) = data.validation.comp_dist_ratio {
        if ratio < 1.0 {
            let warn_style = Style::default().fg(Color::Yellow);
            rows.push(Row::new([
                Cell::from(Span::styled(
                    format!(
                        "Warning: Officer comp ({}) < distributions ({}) — review reasonable comp",
                        money(data.validation.officer_comp),
                        money(data.validation.distributions)
                    ),
                    warn_style,
                )),
                Cell::from(""),
                Cell::from(""),
            ]));
        }
    }

    let effective_year = year.unwrap_or_else(|| chrono::Datelike::year(&chrono::Local::now()));
    Ok(Box::new(TableReportView::new(
        "K-1 Preparation Worksheet (Form 1120-S)",
        header,
        rows,
        widths,
    ).with_date(DateGranularity::YearOnly, effective_year, None)))
}

// ---------------------------------------------------------------------------
// Register (standalone — delegates to RegisterBrowser)
// ---------------------------------------------------------------------------

fn register_standalone(cmd: ReportCommands) -> Result<()> {
    let ReportCommands::Register {
        month,
        year,
        from_date,
        to_date,
        account,
        ..
    } = cmd
    else {
        return Ok(());
    };

    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let data = reports::get_register(
        &conn,
        y,
        mm,
        from_date.as_deref(),
        to_date.as_deref(),
        account.as_deref(),
    )?;

    if data.rows.is_empty() {
        println!("No transactions found.");
        return Ok(());
    }

    let categories = crate::reviewer::get_categories(&conn).unwrap_or_default();
    let filter_desc = if let Some(ref a) = account {
        format!("account: {a}")
    } else if let Some(y) = y {
        format!("year: {y}")
    } else {
        "all".to_string()
    };

    let mut browser =
        crate::browser::RegisterBrowser::new(data.rows, data.total, filter_desc, categories);

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let mut terminal = ratatui::init();

    let result: Result<()> = loop {
        if let Err(e) = terminal.draw(|frame| browser.draw_frame(frame)) {
            break Err(e.into());
        }
        match crossterm::event::read() {
            Err(e) => break Err(e.into()),
            Ok(crossterm::event::Event::Key(key)) => {
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue;
                }
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    break Ok(());
                }
                use crate::browser::BrowseAction;
                match browser.handle_key_event(key.code) {
                    BrowseAction::Close => break Ok(()),
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
            }
            _ => {}
        }
    };

    drop(terminal);
    ratatui::restore();
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}\u{2026}")
    }
}

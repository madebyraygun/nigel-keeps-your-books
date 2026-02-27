use std::io::BufWriter;

use printpdf::*;

use crate::error::{NigelError, Result};
use crate::fmt::money;
use crate::reports::*;

// US Letter dimensions (mm)
const PAGE_W: f32 = 215.9;
const PAGE_H: f32 = 279.4;
const MARGIN_TOP: f32 = 25.4;
const MARGIN_BOTTOM: f32 = 25.4;
const MARGIN_LEFT: f32 = 19.05;
const MARGIN_RIGHT: f32 = 19.05;
const ROW_H: f32 = 5.0;
const FONT_SIZE: f32 = 10.0;
const TITLE_SIZE: f32 = 16.0;
const SUBTITLE_SIZE: f32 = 10.0;

fn approx_text_width(text: &str, size: f32) -> f32 {
    text.len() as f32 * size * 0.18
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

struct Col {
    width: f32,
    align: Align,
}

struct PdfWriter {
    doc: PdfDocumentReference,
    font: IndirectFontRef,
    font_bold: IndirectFontRef,
    current_page: PdfPageIndex,
    current_layer: PdfLayerIndex,
    y: f32,
}

impl PdfWriter {
    fn new(title: &str) -> Result<Self> {
        let (doc, page, layer) =
            PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        Ok(Self {
            doc,
            font,
            font_bold,
            current_page: page,
            current_layer: layer,
            y: MARGIN_TOP,
        })
    }

    fn pdf_y(&self) -> f32 {
        PAGE_H - self.y
    }

    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer");
        self.current_page = page;
        self.current_layer = layer;
        self.y = MARGIN_TOP;
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y + needed > PAGE_H - MARGIN_BOTTOM {
            self.new_page();
        }
    }

    fn text(&self, s: &str, x: f32, size: f32, bold: bool) {
        let font = if bold {
            self.font_bold.clone()
        } else {
            self.font.clone()
        };
        let layer = self
            .doc
            .get_page(self.current_page)
            .get_layer(self.current_layer);
        layer.use_text(s, size, Mm(x), Mm(self.pdf_y()), &font);
    }

    fn hline(&self, x1: f32, x2: f32) {
        let layer = self
            .doc
            .get_page(self.current_page)
            .get_layer(self.current_layer);
        layer.set_outline_thickness(0.5);
        let line = Line {
            points: vec![
                (Point::new(Mm(x1), Mm(self.pdf_y())), false),
                (Point::new(Mm(x2), Mm(self.pdf_y())), false),
            ],
            is_closed: false,
        };
        layer.add_line(line);
    }

    fn header(&mut self, title: &str, company: &str, date_range: &str) {
        self.text(title, MARGIN_LEFT, TITLE_SIZE, true);
        self.y += 7.0;
        if !company.is_empty() {
            self.text(company, MARGIN_LEFT, SUBTITLE_SIZE, false);
            self.y += 5.0;
        }
        self.text(date_range, MARGIN_LEFT, SUBTITLE_SIZE, false);
        self.y += 5.0;
        let ts = chrono::Local::now()
            .format("Generated %Y-%m-%d %H:%M")
            .to_string();
        self.text(&ts, MARGIN_LEFT, 8.0, false);
        self.y += 5.0;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 5.0;
    }

    fn table_header(&mut self, cols: &[Col], headers: &[&str]) {
        self.ensure_space(ROW_H * 2.0);
        let mut x = MARGIN_LEFT;
        for (i, col) in cols.iter().enumerate() {
            if i < headers.len() {
                match col.align {
                    Align::Left => self.text(headers[i], x, FONT_SIZE, true),
                    Align::Right => {
                        let tw = approx_text_width(headers[i], FONT_SIZE);
                        self.text(headers[i], x + col.width - tw, FONT_SIZE, true);
                    }
                }
            }
            x += col.width;
        }
        self.y += ROW_H;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 2.0;
    }

    fn table_row(&mut self, cols: &[Col], values: &[&str], bold: bool) {
        self.ensure_space(ROW_H);
        let mut x = MARGIN_LEFT;
        for (i, col) in cols.iter().enumerate() {
            if i < values.len() {
                match col.align {
                    Align::Left => self.text(values[i], x, FONT_SIZE, bold),
                    Align::Right => {
                        let tw = approx_text_width(values[i], FONT_SIZE);
                        self.text(values[i], x + col.width - tw, FONT_SIZE, bold);
                    }
                }
            }
            x += col.width;
        }
        self.y += ROW_H;
    }

    fn section_label(&mut self, label: &str) {
        self.ensure_space(ROW_H);
        self.text(label, MARGIN_LEFT, FONT_SIZE, true);
        self.y += ROW_H;
    }

    fn blank_row(&mut self) {
        self.y += ROW_H;
    }

    fn separator(&mut self) {
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 2.0;
    }

    fn to_bytes(self) -> Result<Vec<u8>> {
        let mut buf = BufWriter::new(Vec::new());
        self.doc
            .save(&mut buf)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        Ok(buf.into_inner().map_err(|e| NigelError::Pdf(e.to_string()))?)
    }
}

// ---------------------------------------------------------------------------
// Render functions
// ---------------------------------------------------------------------------

pub fn render_pnl(report: &PnlReport, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Profit & Loss")?;
    pdf.header("Profit & Loss", company, date_range);

    let cols = &[
        Col { width: 130.0, align: Align::Left },
        Col { width: 47.8, align: Align::Right },
    ];
    pdf.table_header(cols, &["Category", "Amount"]);

    if !report.income.is_empty() {
        pdf.section_label("INCOME");
        for item in &report.income {
            let amt = money(item.total);
            pdf.table_row(cols, &[&item.name, &amt], false);
        }
        let total = money(report.total_income);
        pdf.table_row(cols, &["Total Income", &total], true);
        pdf.blank_row();
    }

    if !report.expenses.is_empty() {
        pdf.section_label("EXPENSES");
        for item in &report.expenses {
            let amt = money(item.total.abs());
            pdf.table_row(cols, &[&item.name, &amt], false);
        }
        let total = money(report.total_expenses.abs());
        pdf.table_row(cols, &["Total Expenses", &total], true);
        pdf.blank_row();
    }

    pdf.separator();
    let label = if report.net >= 0.0 { "NET INCOME" } else { "NET LOSS" };
    let net = money(report.net);
    pdf.table_row(cols, &[label, &net], true);

    pdf.to_bytes()
}

pub fn render_expenses(
    report: &ExpenseBreakdown,
    company: &str,
    date_range: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Expense Breakdown")?;
    pdf.header("Expense Breakdown", company, date_range);

    let cols = &[
        Col { width: 90.0, align: Align::Left },
        Col { width: 40.0, align: Align::Right },
        Col { width: 27.8, align: Align::Right },
        Col { width: 20.0, align: Align::Right },
    ];
    pdf.table_header(cols, &["Category", "Amount", "%", "Count"]);

    for item in &report.categories {
        let amt = money(item.total.abs());
        let pct = format!("{:.1}%", item.pct);
        let cnt = item.count.to_string();
        pdf.table_row(cols, &[&item.name, &amt, &pct, &cnt], false);
    }
    let total = money(report.total.abs());
    pdf.separator();
    pdf.table_row(cols, &["Total", &total, "", ""], true);

    if !report.top_vendors.is_empty() {
        pdf.blank_row();
        pdf.section_label("Top Vendors");
        let vcols = &[
            Col { width: 90.0, align: Align::Left },
            Col { width: 40.0, align: Align::Right },
            Col { width: 47.8, align: Align::Right },
        ];
        pdf.table_header(vcols, &["Vendor", "Amount", "Count"]);
        for v in &report.top_vendors {
            let amt = money(v.total.abs());
            let cnt = v.count.to_string();
            pdf.table_row(vcols, &[&v.vendor, &amt, &cnt], false);
        }
    }

    pdf.to_bytes()
}

pub fn render_tax(report: &TaxSummary, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Tax Summary")?;
    pdf.header("Tax Summary", company, date_range);

    let cols = &[
        Col { width: 70.0, align: Align::Left },
        Col { width: 40.0, align: Align::Left },
        Col { width: 30.0, align: Align::Left },
        Col { width: 37.8, align: Align::Right },
    ];
    pdf.table_header(cols, &["Category", "Tax Line", "Type", "Amount"]);

    for item in &report.line_items {
        let tl = item.tax_line.as_deref().unwrap_or("");
        let amt = money(item.total.abs());
        pdf.table_row(cols, &[&item.name, tl, &item.category_type, &amt], false);
    }

    pdf.to_bytes()
}

pub fn render_cashflow(
    report: &CashflowReport,
    company: &str,
    date_range: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Cash Flow")?;
    pdf.header("Cash Flow", company, date_range);

    let cols = &[
        Col { width: 35.0, align: Align::Left },
        Col { width: 37.0, align: Align::Right },
        Col { width: 37.0, align: Align::Right },
        Col { width: 37.0, align: Align::Right },
        Col { width: 31.8, align: Align::Right },
    ];
    pdf.table_header(cols, &["Month", "Inflows", "Outflows", "Net", "Running"]);

    for m in &report.months {
        let inf = money(m.inflows);
        let out = money(m.outflows.abs());
        let net = money(m.net);
        let run = money(m.running_balance);
        pdf.table_row(cols, &[&m.month, &inf, &out, &net, &run], false);
    }

    pdf.to_bytes()
}

pub fn render_flagged(rows: &[FlaggedTransaction], company: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Flagged Transactions")?;
    pdf.header(
        "Flagged Transactions",
        company,
        &format!("{} items", rows.len()),
    );

    let cols = &[
        Col { width: 15.0, align: Align::Left },
        Col { width: 27.0, align: Align::Left },
        Col { width: 80.0, align: Align::Left },
        Col { width: 30.0, align: Align::Right },
        Col { width: 25.8, align: Align::Left },
    ];
    pdf.table_header(cols, &["ID", "Date", "Description", "Amount", "Account"]);

    for r in rows {
        let id = r.id.to_string();
        let amt = money(r.amount.abs());
        pdf.table_row(cols, &[&id, &r.date, &r.description, &amt, &r.account_name], false);
    }

    pdf.to_bytes()
}

pub fn render_balance(report: &BalanceReport, company: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Cash Position")?;
    pdf.header("Cash Position", company, "As of today");

    let cols = &[
        Col { width: 80.0, align: Align::Left },
        Col { width: 50.0, align: Align::Left },
        Col { width: 47.8, align: Align::Right },
    ];
    pdf.table_header(cols, &["Account", "Type", "Balance"]);

    for a in &report.accounts {
        let bal = money(a.balance);
        pdf.table_row(cols, &[&a.name, &a.account_type, &bal], false);
    }

    pdf.separator();
    let total = money(report.total);
    pdf.table_row(cols, &["Total", "", &total], true);

    pdf.blank_row();
    let ytd = money(report.ytd_net_income);
    let ytd_label = format!("YTD Net Income: {ytd}");
    pdf.text(&ytd_label, MARGIN_LEFT, FONT_SIZE, false);

    pdf.to_bytes()
}

pub fn render_k1(report: &K1PrepReport, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("K-1 Preparation Worksheet")?;
    pdf.header("K-1 Preparation Worksheet (Form 1120-S)", company, date_range);

    // Income Summary
    let summary_cols = &[
        Col { width: 130.0, align: Align::Left },
        Col { width: 47.8, align: Align::Right },
    ];
    pdf.section_label("Income Summary");
    pdf.table_header(summary_cols, &["Item", "Amount"]);
    let gr = money(report.gross_receipts);
    pdf.table_row(summary_cols, &["Gross Receipts", &gr], false);
    let oi = money(report.other_income);
    pdf.table_row(summary_cols, &["Other Income", &oi], false);
    let td = money(report.total_deductions);
    pdf.table_row(summary_cols, &["Total Deductions", &td], false);
    pdf.separator();
    let label = if report.ordinary_business_income >= 0.0 {
        "Ordinary Business Income"
    } else {
        "Ordinary Business Loss"
    };
    let obi = money(report.ordinary_business_income);
    pdf.table_row(summary_cols, &[label, &obi], true);
    pdf.blank_row();

    // Deductions by Line
    if !report.deduction_lines.is_empty() {
        let ded_cols = &[
            Col { width: 30.0, align: Align::Left },
            Col { width: 100.0, align: Align::Left },
            Col { width: 47.8, align: Align::Right },
        ];
        pdf.section_label("Deductions by Line");
        pdf.table_header(ded_cols, &["Line", "Category", "Amount"]);
        for item in &report.deduction_lines {
            let amt = money(item.total);
            pdf.table_row(ded_cols, &[&item.form_line, &item.category_name, &amt], false);
        }
        pdf.blank_row();
    }

    // Schedule K Items
    if !report.schedule_k_items.is_empty() {
        let sk_cols = &[
            Col { width: 30.0, align: Align::Left },
            Col { width: 100.0, align: Align::Left },
            Col { width: 47.8, align: Align::Right },
        ];
        pdf.section_label("Schedule K");
        pdf.table_header(sk_cols, &["Line", "Item", "Amount"]);
        for item in &report.schedule_k_items {
            let amt = money(item.total.abs());
            pdf.table_row(sk_cols, &[&item.form_line, &item.category_name, &amt], false);
        }
        pdf.blank_row();
    }

    // Line 19 Other Deductions detail
    if !report.other_deductions.is_empty() {
        let od_cols = &[
            Col { width: 80.0, align: Align::Left },
            Col { width: 48.9, align: Align::Right },
            Col { width: 48.9, align: Align::Right },
        ];
        pdf.section_label("Line 19 — Other Deductions");
        pdf.table_header(od_cols, &["Category", "Full Amount", "Deductible"]);
        for item in &report.other_deductions {
            let label = if item.deductible < item.total {
                format!("{} (50%)", item.category_name)
            } else {
                item.category_name.clone()
            };
            let full = money(item.total);
            let ded = money(item.deductible);
            pdf.table_row(od_cols, &[&label, &full, &ded], false);
        }
        pdf.separator();
        let odt = money(report.other_deductions_total);
        pdf.table_row(od_cols, &["Total Other Deductions", "", &odt], true);
    }

    // Validation notes
    if report.validation.uncategorized_count > 0 {
        pdf.blank_row();
        let warning = format!(
            "Warning: {} uncategorized transactions",
            report.validation.uncategorized_count
        );
        pdf.text(&warning, MARGIN_LEFT, FONT_SIZE, true);
        pdf.y += ROW_H;
    }

    pdf.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn seed(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let income_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expense_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Client payment', 1000.0, ?2)",
            rusqlite::params![acct, income_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-20', 'Adobe CC', -50.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
    }

    #[test]
    fn test_render_pnl_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        let bytes = render_pnl(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_expenses_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        let bytes = render_expenses(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_tax_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_tax_summary(&conn, Some(2025)).unwrap();
        let bytes = render_tax(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_cashflow_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_cashflow(&conn, Some(2025), None).unwrap();
        let bytes = render_cashflow(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_flagged_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let rows = get_flagged(&conn).unwrap();
        let bytes = render_flagged(&rows, "Test Corp").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_balance_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_balance(&conn).unwrap();
        let bytes = render_balance(&report, "Test Corp").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_k1_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        let bytes = render_k1(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}

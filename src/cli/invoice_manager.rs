use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::error::Result;
use crate::fmt::money;
use crate::invoicing::clients::get_client;
use crate::invoicing::invoices::{
    get_invoice, line_items, list_invoices, paid_amount, payments, InvoiceListRow,
};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};
use crate::tui::{FOOTER_STYLE, GREEN, HEADER_STYLE};

pub enum InvoiceAction {
    Continue,
    Close,
    /// The screen has entered a blocking state and needs the controller to
    /// paint it before the work runs.
    Perform,
}

enum Screen {
    List,
    Detail,
}

/// Everything the detail view shows, loaded on entry and reloaded after every
/// mutation.
struct Detail {
    invoice: Invoice,
    client: Client,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64,
}

impl Detail {
    fn load(conn: &Connection, invoice_id: i64) -> Result<Self> {
        let invoice = get_invoice(conn, invoice_id)?;
        let client = get_client(conn, invoice.client_id)?;
        Ok(Self {
            items: line_items(conn, invoice.id)?,
            payments: payments(conn, invoice.id)?,
            paid: paid_amount(conn, invoice.id)?,
            invoice,
            client,
        })
    }

    fn balance(&self) -> f64 {
        self.invoice.total - self.paid
    }
}

pub struct InvoiceManager {
    rows: Vec<InvoiceListRow>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    detail: Option<Detail>,
    detail_scroll: usize,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl InvoiceManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        Self {
            rows: list_invoices(conn).unwrap_or_default(),
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            detail: None,
            detail_scroll: 0,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload_list(&mut self, conn: &Connection) {
        self.rows = list_invoices(conn).unwrap_or_default();
        if self.rows.is_empty() {
            self.selection = 0;
        } else {
            self.selection = self.selection.min(self.rows.len() - 1);
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

    /// Load (or reload) the detail for one invoice.
    fn load_detail(&mut self, conn: &Connection, invoice_id: i64) -> Result<()> {
        self.detail = Some(Detail::load(conn, invoice_id)?);
        Ok(())
    }

    fn selected_id(&self) -> Option<i64> {
        self.rows.get(self.selection).map(|r| r.id)
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.screen {
            Screen::List => self.draw_list(frame),
            Screen::Detail => self.draw_detail(frame),
        }
    }

    fn draw_detail(&self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };
        let invoice = &detail.invoice;

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!(" Invoice #{}   ", invoice.number),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(invoice.status.clone(), status_style(&invoice.status)),
            ]),
            Line::from(""),
            Line::from(format!("   Client    {}", detail.client.name)),
            Line::from(format!(
                "   Email     {}",
                optional_display(detail.client.email.as_deref())
            )),
            Line::from(format!(
                "   Issued    {:<16} Due  {:<16} Currency  {}",
                invoice.issue_date,
                optional_display(invoice.due_date.as_deref()),
                invoice.currency,
            )),
        ];
        if let Some(voided_at) = &invoice.voided_at {
            lines.push(Line::from(format!("   Voided    {voided_at}")));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "   {:<40} {:>8} {:>11} {:>12}",
                "Description", "Qty", "Unit", "Amount"
            ),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        for item in &detail.items {
            lines.push(Line::from(format!(
                "   {:<40} {:>8.2} {:>11.2} {:>12.2}",
                truncate(&item.description, 38),
                item.quantity,
                item.unit_amount,
                item.line_total,
            )));
        }
        lines.push(total_line("Subtotal", invoice.subtotal));
        lines.push(total_line("Tax", invoice.tax));
        lines.push(total_line("Total", invoice.total));

        // No empty table: the section only exists once there is a payment.
        if !detail.payments.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "   Payments",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for payment in &detail.payments {
                lines.push(Line::from(format!(
                    "   {}   {:<20} {:>26}",
                    payment.paid_date,
                    payment.method,
                    money(payment.amount)
                )));
            }
        }
        lines.push(total_line("Paid", detail.paid));
        lines.push(total_line("Balance", detail.balance()));

        if let Some(url) = &invoice.stripe_payment_link_url {
            lines.push(Line::from(""));
            lines.push(Line::from(format!("   Pay link  {url}")));
        }

        let visible = (content_area.height as usize).max(1);
        let start = self.detail_scroll.min(lines.len().saturating_sub(1));
        let end = (start + visible).min(lines.len());
        frame.render_widget(Paragraph::new(lines[start..end].to_vec()), content_area);

        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else {
            let hint = if is_void(invoice) {
                " Up/Down=scroll  Esc=back  q=quit"
            } else {
                " s=send  p=record payment  v=void  Up/Down=scroll  Esc=back  q=quit"
            };
            frame.render_widget(Paragraph::new(hint).style(FOOTER_STYLE), hints_area);
        }
    }

    /// Header, separator, content, footer — the four-row frame every manager
    /// screen draws into.
    fn draw_chrome(&self, frame: &mut Frame) -> (Rect, Rect) {
        let area = frame.area();
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
        frame.render_widget(
            Paragraph::new(sep_line.as_str()).style(Style::default().fg(Color::DarkGray)),
            sep,
        );
        (content_area, hints_area)
    }

    fn draw_list(&mut self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);

        // 3 lines of title area + 1 column header = 4 lines of overhead.
        let data_rows = (content_area.height as usize).saturating_sub(4);
        self.last_visible_rows = data_rows;

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Invoices ({})", self.rows.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.rows.is_empty() {
            lines.push(Line::from(
                "   No invoices yet. Draft one with `nigel invoice new` \u{2014} the dashboard",
            ));
            lines.push(Line::from("   cannot create invoices yet."));
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "   {:<6} {:<8} {:<24} {:>12} {:>12} {}",
                    "#", "Status", "Client", "Total", "Balance", "Due"
                ),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));

            let end = (self.scroll_offset + data_rows).min(self.rows.len());
            for i in self.scroll_offset..end {
                let row = &self.rows[i];
                let marker = if i == self.selection { " > " } else { "   " };
                let base = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                // Only the status cell carries colour; the figures are all
                // positive receivables, so a sign-derived colour would be noise.
                lines.push(Line::from(vec![
                    Span::styled(format!("{marker}{:<6} ", row.number), base),
                    Span::styled(
                        format!("{:<8} ", truncate(&row.status, 8)),
                        status_style(&row.status).patch(base),
                    ),
                    Span::styled(
                        format!(
                            "{:<24} {:>12} {:>12} {}",
                            truncate(&row.client_name, 22),
                            money(row.total),
                            money(balance(row)),
                            row.due_date.as_deref().unwrap_or("\u{2014}"),
                        ),
                        base,
                    ),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(" Enter=open  Esc=back  q=quit").style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        match &self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::Detail => self.handle_detail_key(code, conn),
        }
    }

    fn handle_detail_key(&mut self, code: KeyCode, _conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Up => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            KeyCode::Down => self.detail_scroll += 1,
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
            KeyCode::PageDown => self.detail_scroll += 10,
            KeyCode::Esc => self.close_detail(),
            KeyCode::Char('q') => return InvoiceAction::Close,
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn open_detail(&mut self, conn: &Connection) {
        let Some(id) = self.selected_id() else {
            return;
        };
        match self.load_detail(conn, id) {
            Ok(()) => {
                self.detail_scroll = 0;
                self.screen = Screen::Detail;
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn close_detail(&mut self) {
        self.screen = Screen::List;
        self.detail = None;
    }

    fn handle_list_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        if self.rows.is_empty() {
            return match code {
                KeyCode::Char('q') | KeyCode::Esc => InvoiceAction::Close,
                _ => InvoiceAction::Continue,
            };
        }
        let last = self.rows.len() - 1;
        let page = self.last_visible_rows.max(1);
        match code {
            KeyCode::Up => self.selection = self.selection.saturating_sub(1),
            KeyCode::Down => self.selection = (self.selection + 1).min(last),
            KeyCode::PageUp => self.selection = self.selection.saturating_sub(page),
            KeyCode::PageDown => self.selection = (self.selection + page).min(last),
            KeyCode::Home => self.selection = 0,
            KeyCode::End => self.selection = last,
            KeyCode::Enter => {
                self.open_detail(conn);
                return InvoiceAction::Continue;
            }
            KeyCode::Char('q') | KeyCode::Esc => return InvoiceAction::Close,
            _ => return InvoiceAction::Continue,
        }
        self.ensure_visible(self.last_visible_rows);
        InvoiceAction::Continue
    }
}

/// One right-aligned label/amount row under the line items.
fn total_line(label: &str, amount: f64) -> Line<'static> {
    Line::from(format!("   {:>44} {:>15}", label, money(amount)))
}

/// `voided_at` is the fact and `status` is derived from it, the same reading
/// `cli::invoice` takes.
fn is_void(invoice: &Invoice) -> bool {
    invoice.voided_at.is_some() || invoice.status == "void"
}

/// An absent value reads as an em dash, never as an invented blank.
fn optional_display(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "\u{2014}".to_string(),
    }
}

/// What is still owed on an invoice.
fn balance(row: &InvoiceListRow) -> f64 {
    row.total - row.paid
}

/// Colour carries status, since every figure on this screen is a positive
/// receivable. The column is TEXT, so an unrecognized value renders plain
/// rather than panicking.
fn status_style(status: &str) -> Style {
    match status {
        "draft" | "void" => Style::default().fg(Color::DarkGray),
        "sent" => Style::default().fg(Color::Cyan),
        "partial" => Style::default().fg(Color::Yellow),
        "paid" => Style::default().fg(GREEN),
        "overdue" => Style::default().fg(Color::Red),
        _ => Style::default(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}\u{2026}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices::{
        create_invoice, record_payment, set_payment_link, void_invoice, NewLineItem,
    };
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn manager(conn: &Connection) -> InvoiceManager {
        InvoiceManager::new(conn, "Hello, Dalton.")
    }

    fn is_close(action: InvoiceAction) -> bool {
        matches!(action, InvoiceAction::Close)
    }

    /// A client and one invoice of `amount`, returning the invoice row id.
    fn seed_invoice(conn: &Connection, client: &str, amount: f64) -> i64 {
        let cid = add_client(conn, client, Some("ops@cedar.test"), None, None).unwrap();
        seed_invoice_for(conn, cid, amount)
    }

    fn seed_invoice_for(conn: &Connection, client_id: i64, amount: f64) -> i64 {
        let items = vec![NewLineItem {
            description: "Strategy workshop".into(),
            quantity: 1.0,
            unit_amount: amount,
        }];
        create_invoice(
            conn,
            client_id,
            "2026-07-16",
            Some("2026-08-15"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap()
    }

    fn seed_three(conn: &Connection) -> Vec<i64> {
        let cid = add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        [100.0, 200.0, 300.0]
            .into_iter()
            .map(|amount| seed_invoice_for(conn, cid, amount))
            .collect()
    }

    #[test]
    fn new_loads_invoices_newest_first() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
        let mgr = manager(&conn);

        let numbers: Vec<i64> = mgr.rows.iter().map(|r| r.number).collect();
        assert_eq!(numbers, [1250, 1249, 1248]);
    }

    #[test]
    fn new_on_an_empty_book_does_not_panic() {
        let (_d, conn) = test_conn();
        let mgr = manager(&conn);
        assert!(mgr.rows.is_empty());
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn navigation_clamps_at_both_ends() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::End, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::PageDown, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::Home, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::PageUp, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::PageDown, &conn);
        assert_eq!(
            mgr.selection, 2,
            "a page longer than the list lands on the end"
        );
    }

    #[test]
    fn esc_and_q_close_from_the_list() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
        let mut mgr = manager(&conn);
        assert!(is_close(mgr.handle_key(KeyCode::Esc, &conn)));
        assert!(is_close(mgr.handle_key(KeyCode::Char('q'), &conn)));
    }

    #[test]
    fn keys_on_an_empty_list_do_not_panic() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        for code in [
            KeyCode::Down,
            KeyCode::End,
            KeyCode::PageDown,
            KeyCode::Enter,
        ] {
            assert!(!is_close(mgr.handle_key(code, &conn)));
        }
        assert!(is_close(mgr.handle_key(KeyCode::Esc, &conn)));
    }

    #[test]
    fn status_style_maps_every_invoice_status() {
        let expected = [
            ("draft", Color::DarkGray),
            ("sent", Color::Cyan),
            ("partial", Color::Yellow),
            ("paid", GREEN),
            ("overdue", Color::Red),
            ("void", Color::DarkGray),
        ];
        for (status, colour) in expected {
            assert_eq!(status_style(status).fg, Some(colour), "status {status}");
        }
        // The column is TEXT, not an enum.
        assert_eq!(status_style("something-else"), Style::default());
    }

    #[test]
    fn balance_is_total_minus_paid() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();

        let mgr = manager(&conn);
        let row = &mgr.rows[0];
        assert_eq!(balance(row), 750.0);
    }

    fn detail_of(mgr: &InvoiceManager) -> &Detail {
        mgr.detail.as_ref().expect("no detail loaded")
    }

    #[test]
    fn enter_loads_the_detail_for_the_selected_invoice() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        let detail = detail_of(&mgr);
        assert_eq!(detail.invoice.number, 1248);
        assert_eq!(detail.client.name, "Cedar Systems");
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.payments.len(), 1);
        assert_eq!(detail.paid, 1_250.0);
        assert_eq!(detail.balance(), 750.0);
    }

    #[test]
    fn esc_from_detail_returns_to_the_list_without_closing_the_screen() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(!is_close(mgr.handle_key(KeyCode::Esc, &conn)));
        assert!(matches!(mgr.screen, Screen::List));
        assert!(mgr.detail.is_none());
    }

    #[test]
    fn q_from_detail_leaves_the_screen() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(is_close(mgr.handle_key(KeyCode::Char('q'), &conn)));
    }

    #[test]
    fn enter_on_an_empty_list_does_nothing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::List));
        assert!(mgr.detail.is_none());
    }

    #[test]
    fn a_load_failure_reports_and_stays_on_the_list() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        // The client vanishes out from under the invoice.
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute("DELETE FROM clients", []).unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::List));
        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("Client not found"), "got: {message}");
    }

    #[test]
    fn detail_scroll_clamps_at_zero() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.detail_scroll, 0);
        mgr.handle_key(KeyCode::PageUp, &conn);
        assert_eq!(mgr.detail_scroll, 0);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.detail_scroll, 1);
    }

    #[test]
    fn the_detail_renders_the_invoice_its_payments_and_the_action_keys() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoice #1248"), "{screen}");
        assert!(screen.contains("Cedar Systems"), "{screen}");
        assert!(screen.contains("Strategy workshop"), "{screen}");
        assert!(screen.contains("Payments"), "{screen}");
        assert!(screen.contains("2026-08-01"), "{screen}");
        assert!(screen.contains("Balance"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("Pay link  https://pay/x"), "{screen}");
        assert!(
            screen.contains("s=send  p=record payment  v=void"),
            "{screen}"
        );
        for row in screen.lines() {
            assert!(row.chars().count() <= 80, "row overflows: {row:?}");
        }
    }

    #[test]
    fn an_unpaid_invoice_has_no_payments_section_and_no_pay_link() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 1_250.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(!screen.contains("Payments"), "{screen}");
        assert!(!screen.contains("Pay link"), "{screen}");
        assert!(screen.contains("Paid"), "{screen}");
    }

    #[test]
    fn a_void_invoice_shows_its_void_date_and_drops_the_action_keys() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-07").unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Voided    2026-08-07"), "{screen}");
        assert!(!screen.contains("s=send"), "{screen}");
        assert!(
            screen.contains("Up/Down=scroll  Esc=back  q=quit"),
            "{screen}"
        );
    }

    /// The screen as an 80x24 terminal renders it, one string per row.
    fn rendered(mgr: &mut InvoiceManager) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| mgr.draw(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_list_renders_its_columns_inside_eighty_columns() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoices (1)"), "{screen}");
        for column in ["#", "Status", "Client", "Total", "Balance", "Due"] {
            assert!(screen.contains(column), "{column} missing:\n{screen}");
        }
        assert!(screen.contains("> 1248"), "{screen}");
        assert!(screen.contains("$2,000.00"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("2026-08-15"), "{screen}");
        assert!(screen.contains("Enter=open  Esc=back  q=quit"), "{screen}");
        for row in screen.lines() {
            assert!(row.chars().count() <= 80, "row overflows: {row:?}");
        }
    }

    #[test]
    fn the_empty_list_points_at_the_command_that_creates_one() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoices (0)"), "{screen}");
        assert!(screen.contains("nigel invoice new"), "{screen}");
    }

    #[test]
    fn a_missing_due_date_renders_as_an_em_dash() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Retainer".into(),
            quantity: 1.0,
            unit_amount: 1_250.0,
        }];
        create_invoice(&conn, cid, "2026-08-06", None, "USD", &items, None, None).unwrap();
        let mut mgr = manager(&conn);

        assert!(rendered(&mut mgr).contains('\u{2014}'));
    }
}

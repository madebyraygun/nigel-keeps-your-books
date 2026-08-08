use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::invoice::{build_clients, ensure_not_void, payment_amount};
use crate::error::Result;
use crate::fmt::money;
use crate::invoicing::clients::get_client;
use crate::invoicing::invoices::{
    ensure_voidable, get_invoice, line_items, list_invoices, paid_amount, payments, record_payment,
    validate_date, void_invoice, InvoiceListRow,
};
use crate::invoicing::render_html::load_template;
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};
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
    PayForm(PayForm),
    ConfirmVoid,
    ConfirmSend,
    Sending,
}

/// The four methods `invoice_payments.method` allows. A fifth option would be
/// a CHECK-constraint failure at insert time, not a compile error.
const METHODS: &[&str] = &["direct_deposit", "ach", "stripe", "other"];

struct PayForm {
    amount: String,
    date: String,
    method: usize,
    focused: usize,
}

impl PayForm {
    /// Prefilled with the outstanding balance and today; a settled invoice
    /// opens with an empty amount rather than a zero to delete.
    fn new(balance: f64, today: &str) -> Self {
        Self {
            amount: if balance < 0.005 {
                String::new()
            } else {
                format!("{balance:.2}")
            },
            date: today.to_string(),
            method: 0,
            focused: 0,
        }
    }

    fn method(&self) -> &'static str {
        METHODS[self.method]
    }

    fn push(&mut self, c: char) {
        match self.focused {
            AMOUNT_IDX if c.is_ascii_digit() || c == '.' || c == ',' => self.amount.push(c),
            DATE_IDX if c.is_ascii_digit() || c == '-' => self.date.push(c),
            _ => {}
        }
    }

    fn backspace(&mut self) {
        match self.focused {
            AMOUNT_IDX => {
                self.amount.pop();
            }
            DATE_IDX => {
                self.date.pop();
            }
            _ => {}
        }
    }
}

const AMOUNT_IDX: usize = 0;
const DATE_IDX: usize = 1;
const METHOD_IDX: usize = 2;
const PAY_FIELDS: usize = 3;

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
            Screen::Detail | Screen::ConfirmVoid | Screen::ConfirmSend => self.draw_detail(frame),
            Screen::PayForm(form) => self.draw_pay_form(frame, form),
            Screen::Sending => self.draw_sending(frame),
        }
    }

    /// S7. The terminal really is unresponsive for the duration of the send,
    /// so the frame says so rather than animating a spinner it cannot advance.
    fn draw_sending(&self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };
        let email = optional_display(detail.client.email.as_deref());

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Sending invoice #{}", detail.invoice.number),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("   Creating the Stripe payment link, publishing the page and PDF, and"),
            Line::from(format!("   emailing {email}.")),
            Line::from(""),
            Line::from("   This can take a few seconds. Nigel is not reading keys until it"),
            Line::from("   finishes."),
        ];
        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Working\u{2026}").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn draw_pay_form(&self, frame: &mut Frame, form: &PayForm) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    " Record a Payment \u{2014} invoice #{}",
                    detail.invoice.number
                ),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("   Client     {}", detail.client.name)),
            Line::from(format!(
                "   Total      {:<14} Paid  {:<14} Balance  {}",
                money(detail.invoice.total),
                money(detail.paid),
                money(detail.balance()),
            )),
            Line::from(""),
        ];

        for (idx, label, value) in [
            (AMOUNT_IDX, "Amount", format!("$ {}", form.amount)),
            (DATE_IDX, "Date", format!("  {}", form.date)),
            (
                METHOD_IDX,
                "Method",
                if form.focused == METHOD_IDX {
                    format!("< {} >", form.method())
                } else {
                    format!("  {}  ", form.method())
                },
            ),
        ] {
            let focused = form.focused == idx;
            let (label_style, value_style, cursor) = if focused {
                (
                    Style::default().add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Cyan),
                    if idx == METHOD_IDX { "" } else { "_" },
                )
            } else {
                (Style::default(), Style::default(), "")
            };
            lines.push(Line::from(vec![
                Span::styled(format!("   {label:<10} "), label_style),
                Span::styled(format!("{value}{cursor}"), value_style),
            ]));
        }

        if let Some(msg) = &self.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Tab=next field  Left/Right=method  Enter=record  Esc=cancel")
                .style(FOOTER_STYLE),
            hints_area,
        );
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

        if let Screen::ConfirmSend = &self.screen {
            lines.push(Line::from(""));
            for line in send_confirmation(detail) {
                lines.push(Line::from(Span::styled(
                    format!("   {line}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        // The invoice stays on screen while the confirmation is answered.
        if let Screen::ConfirmVoid = &self.screen {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "   Void invoice #{} for {} ({})?",
                    invoice.number,
                    detail.client.name,
                    money(invoice.total)
                ),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "   Void is permanent. A void invoice can never be sent or paid.",
                Style::default().fg(Color::Yellow),
            )));
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
        } else if let Screen::ConfirmVoid = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=void  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else if let Screen::ConfirmSend = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=send  n=cancel").style(FOOTER_STYLE),
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
            Screen::PayForm(_) => self.handle_pay_key(code, conn),
            Screen::ConfirmVoid => self.handle_void_key(code, conn),
            Screen::ConfirmSend => self.handle_confirm_send_key(code),
            // The screen is painted and then blocks; no key is read until the
            // send returns.
            Screen::Sending => InvoiceAction::Continue,
        }
    }

    /// `s` on the detail view. Every guard runs before the dialog opens, so it
    /// never offers something that is going to fail: the void check, the
    /// client's email, the invoice template, and the invoicing config — in the
    /// order `nigel invoice send` runs them, and none of them touches the
    /// network.
    pub(crate) fn begin_send(
        &mut self,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) -> InvoiceAction {
        let Some(detail) = &self.detail else {
            return InvoiceAction::Continue;
        };
        if let Err(e) = ensure_not_void(&detail.invoice, "sent") {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        if detail.client.email.is_none() {
            // send.rs's own wording, so the two front ends cannot disagree.
            let name = detail.client.name.clone();
            self.set_status(format!("client '{name}' has no email"));
            return InvoiceAction::Continue;
        }
        if let Err(e) = load_template(data_dir) {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        if let Err(e) = build_clients(cfg) {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        self.screen = Screen::ConfirmSend;
        InvoiceAction::Continue
    }

    fn handle_confirm_send_key(&mut self, code: KeyCode) -> InvoiceAction {
        match code {
            KeyCode::Char('y') => {
                self.screen = Screen::Sending;
                // The controller paints S7 before running the send, so the
                // frozen frame is the one that says it is frozen.
                InvoiceAction::Perform
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::Detail;
                InvoiceAction::Continue
            }
            _ => InvoiceAction::Continue,
        }
    }

    /// `v` on the detail view, pre-flighted through the data layer's own guard
    /// so the dialog is never offered for an invoice that would refuse it.
    fn open_void_confirmation(&mut self, conn: &Connection) {
        let Some(detail) = &self.detail else {
            return;
        };
        match ensure_voidable(conn, &detail.invoice) {
            Ok(()) => self.screen = Screen::ConfirmVoid,
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn handle_void_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Char('y') => self.do_void(conn, &crate::cli::today()),
            KeyCode::Char('n') | KeyCode::Esc => self.screen = Screen::Detail,
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn do_void(&mut self, conn: &Connection, today: &str) {
        let Some(detail) = &self.detail else {
            return;
        };
        let (invoice_id, number) = (detail.invoice.id, detail.invoice.number);
        // The date is the fact refresh_status derives `void` from, so it is
        // today's, never an empty string.
        match void_invoice(conn, invoice_id, today) {
            Ok(()) => {
                self.after_mutation(conn, invoice_id);
                self.set_status(format!("Voided invoice #{number}."));
            }
            Err(e) => {
                self.screen = Screen::Detail;
                self.set_status(e.to_string());
            }
        }
    }

    /// `p` on the detail view: refused outright for a void invoice, in
    /// `cli::invoice`'s own words, before the form is ever offered.
    fn open_pay_form(&mut self, today: &str) {
        let Some(detail) = &self.detail else {
            return;
        };
        if let Err(e) = ensure_not_void(&detail.invoice, "paid") {
            self.set_status(e.to_string());
            return;
        }
        self.screen = Screen::PayForm(PayForm::new(detail.balance(), today));
    }

    fn handle_pay_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        let Screen::PayForm(form) = &mut self.screen else {
            return InvoiceAction::Continue;
        };
        match code {
            KeyCode::Esc => self.screen = Screen::Detail,
            KeyCode::Tab | KeyCode::Down => form.focused = (form.focused + 1) % PAY_FIELDS,
            KeyCode::BackTab | KeyCode::Up => {
                form.focused = if form.focused == 0 {
                    PAY_FIELDS - 1
                } else {
                    form.focused - 1
                };
            }
            KeyCode::Left => {
                if form.focused == METHOD_IDX {
                    form.method = if form.method == 0 {
                        METHODS.len() - 1
                    } else {
                        form.method - 1
                    };
                }
            }
            KeyCode::Right => {
                if form.focused == METHOD_IDX {
                    form.method = (form.method + 1) % METHODS.len();
                }
            }
            KeyCode::Char(c) => form.push(c),
            KeyCode::Backspace => form.backspace(),
            KeyCode::Enter => self.record_pay_form(conn),
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn record_pay_form(&mut self, conn: &Connection) {
        let (Screen::PayForm(form), Some(detail)) = (&self.screen, &self.detail) else {
            return;
        };
        let raw = form.amount.trim().replace(',', "");
        let date = form.date.trim().to_string();
        let method = form.method();

        if raw.is_empty() {
            self.set_status("Amount is required".into());
            return;
        }
        let Ok(typed) = raw.parse::<f64>() else {
            self.set_status("Amount must be a number".into());
            return;
        };
        // The CLI's own rule, so an overpayment stays allowed and only junk is
        // refused; its message names --amount, which this form does not have.
        let amount = match payment_amount(&detail.invoice, detail.paid, Some(typed)) {
            Ok(amount) => amount,
            Err(e) => {
                self.set_status(field_wording(e.to_string()));
                return;
            }
        };
        if date.is_empty() {
            self.set_status("Date is required (YYYY-MM-DD)".into());
            return;
        }
        // A malformed date poisons refresh_status and ar_aging, so it is checked
        // through the data layer's own rule rather than one invented here.
        if let Err(e) = validate_date(&date, "payment") {
            self.set_status(e.to_string());
            return;
        }

        let invoice_id = detail.invoice.id;
        let number = detail.invoice.number;
        if let Err(e) = record_payment(conn, invoice_id, amount, &date, method, None) {
            self.set_status(e.to_string());
            return;
        }
        self.after_mutation(conn, invoice_id);
        let status = self
            .detail
            .as_ref()
            .map(|d| d.invoice.status.clone())
            .unwrap_or_default();
        self.set_status(format!(
            "Recorded {} against invoice #{number} ({status}).",
            money(amount)
        ));
    }

    /// Reload both the row list and the open detail after a write.
    fn after_mutation(&mut self, conn: &Connection, invoice_id: i64) {
        self.reload_list(conn);
        match self.load_detail(conn, invoice_id) {
            Ok(()) => self.screen = Screen::Detail,
            Err(e) => {
                self.detail = None;
                self.screen = Screen::List;
                self.set_status(e.to_string());
            }
        }
    }

    fn handle_detail_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Up => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            KeyCode::Down => self.detail_scroll += 1,
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
            KeyCode::PageDown => self.detail_scroll += 10,
            KeyCode::Char('p') => self.open_pay_form(&crate::cli::today()),
            KeyCode::Char('v') => self.open_void_confirmation(conn),
            KeyCode::Char('s') => return self.begin_send(invoicing_config(), &get_data_dir()),
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

/// The two lines S6 puts under the invoice, worded for a first send or a
/// re-send.
fn send_confirmation(detail: &Detail) -> Vec<String> {
    let invoice = &detail.invoice;
    let email = optional_display(detail.client.email.as_deref());
    match &invoice.published_at {
        Some(published) => vec![
            format!("Re-send invoice #{} to {email}?", invoice.number),
            format!("Published {published}. The existing payment link is reused; the page and"),
            "PDF are republished and the client is emailed again.".to_string(),
        ],
        None => vec![
            format!("Send invoice #{} to {email}?", invoice.number),
            format!(
                "{} \u{b7} {}. Creates a Stripe payment link, publishes the",
                detail.client.name,
                money(invoice.total)
            ),
            "page and PDF, then emails the client.".to_string(),
        ],
    }
}

/// The CLI names the flag it wants; a form names the field that was typed in.
fn field_wording(message: String) -> String {
    match message.strip_prefix("--amount ") {
        Some(rest) => format!("Amount {rest}"),
        None => message,
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
        create_invoice, mark_published, record_payment, set_payment_link, void_invoice, NewLineItem,
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

    fn pay_form(mgr: &InvoiceManager) -> &PayForm {
        match &mgr.screen {
            Screen::PayForm(form) => form,
            _ => panic!("not on the payment form"),
        }
    }

    /// Open the detail for the only invoice and press `p`.
    fn open_pay(mgr: &mut InvoiceManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.handle_key(KeyCode::Char('p'), conn);
    }

    fn type_str(mgr: &mut InvoiceManager, conn: &Connection, text: &str) {
        for ch in text.chars() {
            mgr.handle_key(KeyCode::Char(ch), conn);
        }
    }

    fn clear_field(mgr: &mut InvoiceManager, conn: &Connection) {
        for _ in 0..40 {
            mgr.handle_key(KeyCode::Backspace, conn);
        }
    }

    fn payment_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn p_on_a_void_invoice_is_refused_before_the_form_opens() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-07").unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be paid.")
        );
    }

    #[test]
    fn p_prefills_the_amount_with_the_outstanding_balance_and_today() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        let form = pay_form(&mgr);
        assert_eq!(form.amount, "750.00");
        assert_eq!(form.date, crate::cli::today());
    }

    #[test]
    fn p_on_a_settled_invoice_prefills_an_empty_amount() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        record_payment(&conn, id, 100.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        assert_eq!(pay_form(&mgr).amount, "");
    }

    #[test]
    fn method_options_are_exactly_the_four_the_schema_allows() {
        assert_eq!(METHODS, ["direct_deposit", "ach", "stripe", "other"]);
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        assert_eq!(pay_form(&mgr).method(), "direct_deposit");
    }

    #[test]
    fn every_method_option_is_actually_insertable() {
        // invoice_payments.method carries a CHECK constraint; a fifth option
        // would fail at insert time, not at compile time.
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 1_000.0);
        for method in METHODS {
            record_payment(&conn, id, 1.0, "2026-08-01", method, None)
                .unwrap_or_else(|e| panic!("method {method} is not insertable: {e}"));
        }
    }

    #[test]
    fn left_and_right_cycle_the_method() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        for _ in 0..METHOD_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        mgr.handle_key(KeyCode::Tab, &conn);
        mgr.handle_key(KeyCode::Tab, &conn);
        while pay_form(&mgr).focused != METHOD_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }

        mgr.handle_key(KeyCode::Right, &conn);
        assert_eq!(pay_form(&mgr).method(), "ach");
        mgr.handle_key(KeyCode::Left, &conn);
        assert_eq!(pay_form(&mgr).method(), "direct_deposit");
        mgr.handle_key(KeyCode::Left, &conn);
        assert_eq!(pay_form(&mgr).method(), "other", "the selector wraps");
    }

    /// Type an amount and a date into a freshly opened form, then Enter.
    fn submit_payment(mgr: &mut InvoiceManager, conn: &Connection, amount: &str, date: &str) {
        clear_field(mgr, conn);
        type_str(mgr, conn, amount);
        mgr.handle_key(KeyCode::Tab, conn);
        clear_field(mgr, conn);
        type_str(mgr, conn, date);
        mgr.handle_key(KeyCode::Enter, conn);
    }

    #[test]
    fn the_validation_table_refuses_and_writes_nothing() {
        // Only what the fields actually accept: they take digits, `.` and `,`
        // for the amount and digits and `-` for the date, so a letter never
        // reaches validation.
        let cases: [(&str, &str, &str); 6] = [
            ("", "2026-08-07", "Amount is required"),
            (".", "2026-08-07", "Amount must be a number"),
            ("0", "2026-08-07", "Amount must be a finite number"),
            ("0.00", "2026-08-07", "Amount must be a finite number"),
            ("100", "", "Date is required (YYYY-MM-DD)"),
            (
                "100",
                "2026-13-45",
                "Invalid payment date: 2026-13-45 (expected YYYY-MM-DD)",
            ),
        ];
        for (amount, date, expected) in cases {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 2_000.0);
            let mut mgr = manager(&conn);
            open_pay(&mut mgr, &conn);
            submit_payment(&mut mgr, &conn, amount, date);

            let message = mgr.status_message.clone().unwrap_or_default();
            assert!(
                message.contains(expected),
                "amount {amount:?} date {date:?}: expected {expected:?}, got {message:?}"
            );
            assert!(matches!(mgr.screen, Screen::PayForm(_)), "form stayed open");
            assert_eq!(payment_rows(&conn), 0, "amount {amount:?} date {date:?}");
        }
    }

    #[test]
    fn a_malformed_date_is_refused_in_the_data_layers_wording() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "100", "2026-13-45");

        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invalid payment date: 2026-13-45 (expected YYYY-MM-DD)")
        );
        assert_eq!(payment_rows(&conn), 0);
    }

    #[test]
    fn a_negative_or_non_finite_amount_is_refused_in_the_cli_s_words() {
        // Unreachable by typing — the field takes no `-` and no letters — but
        // the rule is the CLI's, so the screen cannot disagree about it.
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let invoice = get_invoice(&conn, list_invoices(&conn).unwrap()[0].id).unwrap();

        for amount in [-25.0, f64::NAN, f64::INFINITY] {
            let message = field_wording(
                payment_amount(&invoice, 0.0, Some(amount))
                    .unwrap_err()
                    .to_string(),
            );
            assert!(
                message.starts_with("Amount must be a finite number greater than zero"),
                "got: {message}"
            );
        }
    }

    #[test]
    fn the_amount_refusal_names_the_field_not_the_cli_flag() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "0", "2026-08-07");

        let message = mgr.status_message.clone().unwrap();
        assert!(!message.contains("--amount"), "got: {message}");
        assert!(message.starts_with("Amount must be"), "got: {message}");
    }

    #[test]
    fn a_valid_payment_is_recorded_and_returns_to_detail() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "750.00", "2026-08-20");

        assert!(matches!(mgr.screen, Screen::Detail));
        let (amount, date, method): (f64, String, String) = conn
            .query_row(
                "SELECT amount, paid_date, method FROM invoice_payments ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (amount, date.as_str(), method.as_str()),
            (750.0, "2026-08-20", "direct_deposit")
        );

        let detail = detail_of(&mgr);
        assert_eq!(detail.paid, 2_000.0);
        assert_eq!(detail.balance(), 0.0);
        assert_eq!(detail.invoice.status, "paid");
        assert_eq!(mgr.rows[0].paid, 2_000.0, "the list reloaded too");
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Recorded $750.00 against invoice #1248 (paid).")
        );
    }

    #[test]
    fn an_overpayment_is_allowed() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "250", "2026-08-07");

        assert_eq!(payment_rows(&conn), 1);
        assert_eq!(detail_of(&mgr).invoice.status, "paid");
    }

    #[test]
    fn commas_are_stripped_from_the_amount() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 2_000.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "1,250.00", "2026-08-07");

        assert_eq!(detail_of(&mgr).paid, 1_250.0);
    }

    #[test]
    fn the_amount_field_refuses_letters_and_the_date_field_refuses_slashes() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "1a2");
        mgr.handle_key(KeyCode::Tab, &conn);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "2026/08/07");

        assert_eq!(pay_form(&mgr).amount, "12");
        assert_eq!(pay_form(&mgr).date, "20260807");
    }

    #[test]
    fn esc_cancels_the_payment_without_writing() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(payment_rows(&conn), 0);
    }

    #[test]
    fn the_pay_form_renders_the_balance_line_and_the_fields() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Record a Payment"), "{screen}");
        assert!(screen.contains("#1248"), "{screen}");
        assert!(screen.contains("$1,250.00"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("direct_deposit"), "{screen}");
        assert!(
            screen.contains("Tab=next field  Left/Right=method  Enter=record  Esc=cancel"),
            "{screen}"
        );
        for row in screen.lines() {
            assert!(row.chars().count() <= 80, "row overflows: {row:?}");
        }
    }

    fn open_void(mgr: &mut InvoiceManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.handle_key(KeyCode::Char('v'), conn);
    }

    fn voided_at(conn: &Connection) -> Option<String> {
        conn.query_row("SELECT voided_at FROM invoices LIMIT 1", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn v_opens_the_confirmation_naming_the_invoice_client_and_total() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 1_250.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::ConfirmVoid));
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Void invoice #1248 for Acme Co ($1,250.00)?"),
            "{screen}"
        );
        assert!(screen.contains("Void is permanent."), "{screen}");
        assert!(screen.contains("y=void  n=cancel"), "{screen}");
    }

    #[test]
    fn n_and_esc_cancel_without_writing() {
        for key in [KeyCode::Char('n'), KeyCode::Esc] {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Acme Co", 100.0);
            let mut mgr = manager(&conn);
            open_void(&mut mgr, &conn);
            mgr.handle_key(key, &conn);

            assert!(matches!(mgr.screen, Screen::Detail), "{key:?}");
            assert_eq!(voided_at(&conn), None, "{key:?}");
        }
    }

    #[test]
    fn y_voids_and_reloads_the_detail() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Char('y'), &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(detail_of(&mgr).invoice.status, "void");
        assert_eq!(mgr.rows[0].status, "void", "the list reloaded too");
        assert_eq!(mgr.status_message.as_deref(), Some("Voided invoice #1248."));

        let screen = rendered(&mut mgr);
        assert!(
            !screen.contains("s=send"),
            "the actions are gone:\n{screen}"
        );
    }

    #[test]
    fn void_writes_todays_date_as_voided_at() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Char('y'), &conn);

        // refresh_status derives `void` from this column, so a wrong or empty
        // date produces an invoice that will not stay void.
        assert_eq!(voided_at(&conn), Some(crate::cli::today()));
    }

    #[test]
    fn v_on_an_already_void_invoice_is_refused_before_the_dialog() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Acme Co", 100.0);
        void_invoice(&conn, id, "2026-08-06").unwrap();
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is already void.")
        );
    }

    #[test]
    fn v_on_an_invoice_with_payments_is_refused_before_the_dialog() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 has 1250.00 in recorded payments and cannot be voided.")
        );
        assert_eq!(voided_at(&conn), None);
    }

    #[test]
    fn a_void_invoice_cannot_then_be_paid() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Char('y'), &conn);

        mgr.handle_key(KeyCode::Char('p'), &conn);
        assert!(
            matches!(mgr.screen, Screen::Detail),
            "no payment form opened"
        );
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be paid.")
        );
        assert_eq!(payment_rows(&conn), 0);
    }

    fn no_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
        }
    }

    fn full_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            mailgun_api_key: Some("key".into()),
            mailgun_domain: Some("mail.example.test".into()),
            from_email: Some("billing@example.test".into()),
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.test/i".into()),
        }
    }

    /// Open the detail for the only invoice and press `s`, with an injected
    /// config and data directory so no test reads the developer's settings.
    fn begin_send(
        mgr: &mut InvoiceManager,
        conn: &Connection,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) -> InvoiceAction {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.begin_send(cfg, data_dir)
    }

    #[test]
    fn s_on_a_void_invoice_is_refused_before_the_dialog() {
        let (dir, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-06").unwrap();
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be sent.")
        );
    }

    #[test]
    fn s_on_a_client_with_no_email_is_refused_before_the_dialog() {
        let (dir, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        seed_invoice_for(&conn, cid, 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("client 'Acme Co' has no email")
        );
    }

    #[test]
    fn s_with_missing_invoicing_config_names_the_first_absent_key() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, no_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("stripe_secret_key"), "got: {message}");
    }

    #[test]
    fn s_with_a_broken_template_reports_it_and_stays_on_the_detail() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let template = dir.path().join("templates");
        std::fs::create_dir_all(&template).unwrap();
        std::fs::write(template.join("invoice.html"), "<p>no placeholders</p>").unwrap();

        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail), "no dialog opened");
        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("invoice.html"), "got: {message}");
    }

    #[test]
    fn the_confirmation_names_the_recipient_and_total() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 2_000.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::ConfirmSend));
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Send invoice #1248 to ops@cedar.test?"),
            "{screen}"
        );
        assert!(screen.contains("Cedar Systems"), "{screen}");
        assert!(screen.contains("$2,000.00"), "{screen}");
        assert!(screen.contains("y=send  n=cancel"), "{screen}");
    }

    #[test]
    fn a_published_invoice_gets_the_resend_wording() {
        let (dir, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        mark_published(&conn, id, "2026-07-16").unwrap();
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Re-send invoice #1248"), "{screen}");
        assert!(screen.contains("Published 2026-07-16"), "{screen}");
        assert!(screen.contains("payment link is reused"), "{screen}");
    }

    #[test]
    fn n_and_esc_cancel_the_send() {
        for key in [KeyCode::Char('n'), KeyCode::Esc] {
            let (dir, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            begin_send(&mut mgr, &conn, full_config(), dir.path());
            mgr.handle_key(key, &conn);

            assert!(matches!(mgr.screen, Screen::Detail), "{key:?}");
            assert!(
                get_invoice(&conn, 1).unwrap().published_at.is_none(),
                "{key:?}"
            );
        }
    }

    #[test]
    fn y_moves_to_sending_and_returns_perform() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        let action = mgr.handle_key(KeyCode::Char('y'), &conn);
        assert!(matches!(action, InvoiceAction::Perform));
        assert!(matches!(mgr.screen, Screen::Sending));
    }

    #[test]
    fn the_sending_frame_says_the_terminal_is_frozen() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());
        mgr.handle_key(KeyCode::Char('y'), &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Sending invoice #1248"), "{screen}");
        assert!(screen.contains("ops@cedar.test"), "{screen}");
        assert!(screen.contains("not reading keys"), "{screen}");
        assert!(screen.contains("Working"), "{screen}");
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

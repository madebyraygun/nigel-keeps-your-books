use rusqlite::Connection;

use crate::error::Result;
use crate::invoicing::invoices::line_items;
use crate::invoicing::render_html::{render_invoice_html, PayButton};
use crate::models::{Client, Invoice, InvoiceLineItem};

/// Everything `invoice send` publishes for one invoice.
pub struct RenderedInvoice {
    pub html: String,
    /// `None` only in a build without the `pdf` feature; each caller decides
    /// whether that is fatal (`send`) or a notice (`preview`).
    pub pdf: Option<Vec<u8>>,
}

/// Render an invoice exactly as `send` publishes it. Reads the database, makes
/// no network call, and writes nothing — the one seam `send` and `preview`
/// share, so a preview cannot disagree with what a client receives.
pub fn render_invoice(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    pay: PayButton<'_>,
    contact_email: &str,
) -> Result<RenderedInvoice> {
    // Loaded here rather than passed in, so both callers get the same rows in
    // the same order.
    let items = line_items(conn, invoice.id)?;
    let html = render_invoice_html(invoice, client, &items, pay, contact_email);
    let pdf = render_pdf(invoice, client, &items)?;
    Ok(RenderedInvoice { html, pdf })
}

#[cfg(feature = "pdf")]
fn render_pdf(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
) -> Result<Option<Vec<u8>>> {
    crate::pdf::render_invoice_pdf(invoice, client, items).map(Some)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(
    _invoice: &Invoice,
    _client: &Client,
    _items: &[InvoiceLineItem],
) -> Result<Option<Vec<u8>>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, get_client};
    use crate::invoicing::invoices::{create_invoice, get_invoice, NewLineItem};
    use crate::invoicing::render::render_invoice;
    use crate::invoicing::render_html::PayButton;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn seed(conn: &rusqlite::Connection, items: &[NewLineItem]) -> i64 {
        let cid = add_client(conn, "Acme", Some("ap@acme.test"), None, None).unwrap();
        create_invoice(conn, cid, "2026-08-04", None, "USD", items, None, None).unwrap()
    }

    fn one_item() -> Vec<NewLineItem> {
        vec![NewLineItem {
            description: "Consulting".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }]
    }

    #[test]
    fn renders_the_html_send_would_publish() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(
            &conn,
            &invoice,
            &client,
            PayButton::Placeholder,
            "ap@acme.test",
        )
        .unwrap();

        assert!(out.html.contains("Invoice #1248"), "got: {}", out.html);
        assert!(out.html.contains("Contact ap@acme.test"));
        assert!(out.html.contains("100.00"));
    }

    #[test]
    fn line_items_come_from_the_database_in_position_order() {
        let (_d, conn) = test_conn();
        let items = vec![
            NewLineItem {
                description: "First".into(),
                quantity: 1.0,
                unit_amount: 10.0,
            },
            NewLineItem {
                description: "Second".into(),
                quantity: 1.0,
                unit_amount: 20.0,
            },
            NewLineItem {
                description: "Third".into(),
                quantity: 1.0,
                unit_amount: 30.0,
            },
        ];
        let id = seed(&conn, &items);
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, "b@e.test").unwrap();

        let at = |needle: &str| out.html.find(needle).expect("line item missing from html");
        assert!(at("First") < at("Second"));
        assert!(at("Second") < at("Third"));
    }

    #[test]
    fn rendering_writes_nothing_to_the_invoice() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        render_invoice(&conn, &invoice, &client, PayButton::Placeholder, "b@e.test").unwrap();

        let after = get_invoice(&conn, id).unwrap();
        assert_eq!(after.status, "draft");
        assert!(after.published_at.is_none());
        assert!(after.stripe_payment_link_url.is_none());
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_is_rendered_when_the_feature_is_on() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, "b@e.test").unwrap();
        assert!(out.pdf.unwrap().starts_with(b"%PDF"));
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn pdf_is_none_without_the_feature_and_html_still_renders() {
        let (_d, conn) = test_conn();
        let id = seed(&conn, &one_item());
        let invoice = get_invoice(&conn, id).unwrap();
        let client = get_client(&conn, invoice.client_id).unwrap();

        let out = render_invoice(&conn, &invoice, &client, PayButton::Omitted, "b@e.test").unwrap();
        assert!(out.pdf.is_none());
        assert!(out.html.contains("Invoice #1248"));
    }
}

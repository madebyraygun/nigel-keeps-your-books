use crate::models::{Client, Invoice, InvoiceLineItem};

const TEMPLATE: &str = include_str!("templates/invoice.html");

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Expands `{{KEY}}` placeholders in a single left-to-right pass, so substituted
/// values are never re-scanned for further placeholders. Unknown placeholders are
/// emitted verbatim.
fn expand(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find("{{") {
        let after_open = &rest[open + 2..];
        match after_open.find("}}").and_then(|close| {
            vars.iter()
                .find(|(k, _)| *k == &after_open[..close])
                .map(|(_, v)| (close, *v))
        }) {
            Some((close, value)) => {
                out.push_str(&rest[..open]);
                out.push_str(value);
                rest = &after_open[close + 2..];
            }
            None => {
                out.push_str(&rest[..open + 2]);
                rest = after_open;
            }
        }
    }

    out.push_str(rest);
    out
}

#[allow(dead_code)]
pub fn render_invoice_html(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    pay_url: Option<&str>,
) -> String {
    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                esc(&i.description),
                i.quantity,
                i.unit_amount,
                i.line_total
            )
        })
        .collect();

    let pay = match pay_url {
        Some(url) => format!("<a class=\"pay\" href=\"{}\">Pay online</a>", esc(url)),
        None => String::new(),
    };
    let due = invoice
        .due_date
        .as_deref()
        .map(|d| format!("<br>Due: {}", esc(d)))
        .unwrap_or_default();

    expand(
        TEMPLATE,
        &[
            ("NUMBER", &invoice.number.to_string()),
            ("CLIENT", &esc(&client.name)),
            ("ISSUE", &esc(&invoice.issue_date)),
            ("DUE", &due),
            ("ROWS", &rows),
            ("CURRENCY", &esc(&invoice.currency)),
            ("TOTAL", &format!("{:.2}", invoice.total)),
            ("PAY", &pay),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    fn sample() -> (Invoice, Client, Vec<InvoiceLineItem>) {
        let inv = Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()),
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: 250.0,
            tax: 0.0,
            total: 250.0,
            notes: None,
            terms: None,
            token: "abc123".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: Some("2026-08-04".into()),
        };
        let client = Client {
            id: 1,
            name: "Acme <Co>".into(),
            email: Some("a@b.test".into()),
            billing_address: None,
            notes: None,
        };
        let items = vec![InvoiceLineItem {
            id: None,
            invoice_id: Some(1),
            description: "Design".into(),
            quantity: 2.0,
            unit_amount: 100.0,
            line_total: 200.0,
            position: 0,
        }];
        (inv, client, items)
    }

    #[test]
    fn renders_number_total_items_and_pay_button() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(&inv, &client, &items, Some("https://pay.stripe.test/x"));
        assert!(html.contains("1248"));
        assert!(html.contains("Design"));
        assert!(html.contains("250.00"));
        assert!(html.contains("https://pay.stripe.test/x"));
        assert!(html.contains("Direct deposit"));
        assert!(html.contains("Acme &lt;Co&gt;")); // escaped
    }

    #[test]
    fn client_name_containing_a_placeholder_stays_literal_text() {
        let (inv, mut client, items) = sample();
        client.name = "Acme {{ROWS}} {{PAY}} Co".into();
        let html = render_invoice_html(&inv, &client, &items, Some("https://pay.stripe.test/x"));
        assert!(html.contains("Acme {{ROWS}} {{PAY}} Co"));
        assert_eq!(html.matches("Design").count(), 1);
        assert_eq!(html.matches("Pay online").count(), 1);
    }

    #[test]
    fn pay_url_cannot_break_out_of_the_href_attribute() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &inv,
            &client,
            &items,
            Some("https://pay.stripe.test/x\"onmouseover=\"alert(1)"),
        );
        assert!(html.contains("&quot;onmouseover"));
        assert!(!html.contains("\"onmouseover"));
    }

    #[test]
    fn omits_pay_button_when_no_url() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(&inv, &client, &items, None);
        assert!(!html.contains("Pay online"));
        assert!(html.contains("Direct deposit"));
    }
}

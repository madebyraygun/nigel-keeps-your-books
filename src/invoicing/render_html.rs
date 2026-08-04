use crate::models::{Client, Invoice, InvoiceLineItem};

const TEMPLATE: &str = include_str!("templates/invoice.html");

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

    TEMPLATE
        .replace("{{NUMBER}}", &invoice.number.to_string())
        .replace("{{ISSUE}}", &esc(&invoice.issue_date))
        .replace("{{DUE}}", &due)
        .replace("{{CURRENCY}}", &esc(&invoice.currency))
        .replace("{{TOTAL}}", &format!("{:.2}", invoice.total))
        .replace("{{CLIENT}}", &esc(&client.name))
        .replace("{{ROWS}}", &rows)
        .replace("{{PAY}}", &pay)
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
    fn omits_pay_button_when_no_url() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(&inv, &client, &items, None);
        assert!(!html.contains("Pay online"));
        assert!(html.contains("Direct deposit"));
    }
}

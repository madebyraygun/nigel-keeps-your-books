use rusqlite::Connection;

use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{get_invoice, line_items, mark_published, set_payment_link};
use crate::invoicing::render_html::render_invoice_html;

pub fn send_invoice<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    contact_email: &str,
    gateway: &G,
    publisher: &P,
    mailer: &M,
) -> Result<String> {
    let mut invoice = get_invoice(conn, invoice_id)?;
    let client = get_client(conn, invoice.client_id)?;
    let email = client
        .email
        .clone()
        .ok_or_else(|| NigelError::Other(format!("client '{}' has no email", client.name)))?;

    // Create the Stripe link once; reuse on resend.
    if invoice.stripe_payment_link_url.is_none() {
        let link = gateway.create_payment_link(&invoice, &client)?;
        set_payment_link(conn, invoice_id, &link.id, &link.url)?;
        invoice = get_invoice(conn, invoice_id)?;
    }
    let pay_url = invoice.stripe_payment_link_url.clone();

    let items = line_items(conn, invoice_id)?;
    let html = render_invoice_html(&invoice, &client, &items, pay_url.as_deref(), contact_email);
    let pdf = render_pdf(&invoice, &client, &items)?;

    let public_url = publisher.publish(&invoice.token, html.as_bytes(), &pdf)?;

    let subject = format!("Invoice #{} from Raygun", invoice.number);
    mailer.send_invoice(&email, &subject, &html, &pdf)?;

    mark_published(conn, invoice_id, today)?;
    Ok(public_url)
}

#[cfg(feature = "pdf")]
fn render_pdf(
    invoice: &crate::models::Invoice,
    client: &crate::models::Client,
    items: &[crate::models::InvoiceLineItem],
) -> Result<Vec<u8>> {
    crate::pdf::render_invoice_pdf(invoice, client, items)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(
    _invoice: &crate::models::Invoice,
    _client: &crate::models::Client,
    _items: &[crate::models::InvoiceLineItem],
) -> Result<Vec<u8>> {
    Err(NigelError::Other(
        "PDF support not compiled in (build with --features pdf)".into(),
    ))
}

// Sending needs a real PDF to publish and attach, so these exercise the orchestration
// only in the `pdf` build; without the feature `render_pdf` above always errors.
#[cfg(all(test, feature = "pdf"))]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::{NigelError, Result};
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{
        AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink,
    };
    use crate::invoicing::invoices::{create_invoice, get_invoice, NewLineItem};
    use crate::migrations::run_migrations;
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    struct FakeGw {
        create_calls: RefCell<u32>,
    }
    impl PaymentGateway for FakeGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            *self.create_calls.borrow_mut() += 1;
            Ok(PaymentLink {
                id: "pl_1".into(),
                url: "https://pay/x".into(),
            })
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(vec![])
        }
    }
    struct FakePub;
    impl AssetPublisher for FakePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.rygn.io/i/{token}/"))
        }
    }
    struct CapturePub {
        html: RefCell<String>,
    }
    impl AssetPublisher for CapturePub {
        fn publish(&self, token: &str, h: &[u8], _p: &[u8]) -> Result<String> {
            *self.html.borrow_mut() = String::from_utf8(h.to_vec()).unwrap();
            Ok(format!("https://billing.example.test/i/{token}/"))
        }
    }
    struct FailPub;
    impl AssetPublisher for FailPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Err(NigelError::Other("upload down".into()))
        }
    }
    struct FakeMail {
        sent: RefCell<u32>,
    }
    impl Mailer for FakeMail {
        fn send_invoice(&self, _to: &str, _s: &str, _h: &str, _p: &[u8]) -> Result<()> {
            *self.sent.borrow_mut() += 1;
            Ok(())
        }
    }

    fn seed(conn: &rusqlite::Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    #[test]
    fn happy_path_publishes_emails_and_marks_sent() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail {
            sent: RefCell::new(0),
        };
        let url = send_invoice(
            &conn,
            id,
            "2026-08-04",
            "billing@example.test",
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert!(url.starts_with("https://billing.rygn.io/i/"));
        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.status, "sent");
        assert_eq!(inv.stripe_payment_link_id.as_deref(), Some("pl_1"));
        assert_eq!(*mail.sent.borrow(), 1);
    }

    #[test]
    fn publish_failure_leaves_draft_and_sends_no_email() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail {
            sent: RefCell::new(0),
        };
        let err = send_invoice(
            &conn,
            id,
            "2026-08-04",
            "billing@example.test",
            &gw,
            &FailPub,
            &mail,
        );
        assert!(err.is_err());
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
        assert_eq!(*mail.sent.borrow(), 0);
    }

    #[test]
    fn published_html_carries_the_supplied_contact_email() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail {
            sent: RefCell::new(0),
        };
        let publisher = CapturePub {
            html: RefCell::new(String::new()),
        };
        send_invoice(
            &conn,
            id,
            "2026-08-04",
            "ap@acme.test",
            &gw,
            &publisher,
            &mail,
        )
        .unwrap();
        assert!(publisher.html.borrow().contains("Contact ap@acme.test"));
    }

    #[test]
    fn resend_reuses_existing_payment_link() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail {
            sent: RefCell::new(0),
        };
        send_invoice(
            &conn,
            id,
            "2026-08-04",
            "billing@example.test",
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        send_invoice(
            &conn,
            id,
            "2026-08-05",
            "billing@example.test",
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert_eq!(*gw.create_calls.borrow(), 1); // created once, reused second time
    }
}

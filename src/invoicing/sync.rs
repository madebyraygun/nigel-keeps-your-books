use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::PaymentGateway;
use crate::invoicing::invoices::{get_invoice, record_payment};

/// One invoice the gateway refused, named by the number a person reads off it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub number: i64,
    pub message: String,
}

/// What one reconciliation run did.
///
/// Per-invoice failures are data, not an error: a deleted Stripe payment link
/// 404s forever, and one of those must not stop the rest of the run or hide the
/// payments it did record.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub recorded: u32,
    pub invoices_checked: u32,
    pub failures: Vec<SyncFailure>,
}

pub fn sync_invoice<G: PaymentGateway>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    gateway: &G,
) -> Result<u32> {
    let invoice = get_invoice(conn, invoice_id)?;
    let link_id = match invoice.stripe_payment_link_id.as_deref() {
        Some(id) => id,
        None => return Ok(0),
    };
    let mut recorded = 0;
    for session in gateway.paid_sessions(link_id)? {
        let is_new = record_payment(
            conn,
            invoice_id,
            session.amount,
            today,
            "stripe",
            Some(&session.session_id),
        )?;
        if is_new {
            recorded += 1;
        }
    }
    Ok(recorded)
}

/// Sync every open invoice that has a payment link.
///
/// One invoice failing at the gateway (a deleted payment link 404s forever)
/// must not stop the rest: per-invoice failures are collected and the loop
/// continues. An error is returned only when every invoice failed, so a caller
/// seeing `Ok` knows at least one invoice was reconciled.
///
/// The failures are returned rather than printed, because a browser cannot read
/// the server's stderr — the CLI prints the same notices from the report.
pub fn sync_all_report<G: PaymentGateway>(
    conn: &Connection,
    today: &str,
    gateway: &G,
) -> Result<SyncReport> {
    let mut stmt = conn.prepare(
        "SELECT id, number FROM invoices
         WHERE stripe_payment_link_id IS NOT NULL AND status IN ('sent','partial','overdue')",
    )?;
    let invoices = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut report = SyncReport {
        invoices_checked: invoices.len() as u32,
        ..SyncReport::default()
    };
    for (id, number) in &invoices {
        match sync_invoice(conn, *id, today, gateway) {
            Ok(recorded) => report.recorded += recorded,
            Err(e) => report.failures.push(SyncFailure {
                number: *number,
                message: e.to_string(),
            }),
        }
    }

    if !report.failures.is_empty() && report.failures.len() == invoices.len() {
        let detail = report
            .failures
            .iter()
            .map(|f| format!("#{}: {}", f.number, f.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(NigelError::Other(format!(
            "all {} invoice(s) failed to sync — {detail}",
            report.failures.len()
        )));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::Result;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{PaidSession, PaymentGateway, PaymentLink};
    use crate::invoicing::invoices::{
        create_invoice, get_invoice, paid_amount, set_payment_link, NewLineItem,
    };
    use crate::migrations::run_migrations;
    use crate::models::{Client, Invoice};

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    struct Gw(Vec<PaidSession>);
    impl PaymentGateway for Gw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            unreachable!()
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(self.0.clone())
        }
    }

    /// Pays every link except the ones named in `broken`, which fail the way a
    /// deleted Stripe payment link does — permanently.
    struct FlakyGw {
        broken: Vec<&'static str>,
    }
    impl PaymentGateway for FlakyGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            unreachable!()
        }
        fn paid_sessions(&self, id: &str) -> Result<Vec<PaidSession>> {
            if self.broken.contains(&id) {
                return Err(crate::error::NigelError::Other(format!(
                    "stripe: no such payment link {id}"
                )));
            }
            Ok(vec![PaidSession {
                session_id: format!("cs_{id}"),
                amount: 100.0,
            }])
        }
    }

    struct PerLinkGw;
    impl PaymentGateway for PerLinkGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            unreachable!()
        }
        fn paid_sessions(&self, id: &str) -> Result<Vec<PaidSession>> {
            Ok(vec![PaidSession {
                session_id: format!("cs_{id}"),
                amount: 100.0,
            }])
        }
    }

    #[test]
    fn sync_records_once_and_marks_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        let gw = Gw(vec![PaidSession {
            session_id: "cs_1".into(),
            amount: 100.0,
        }]);
        assert_eq!(sync_invoice(&conn, id, "2026-08-10", &gw).unwrap(), 1);
        assert_eq!(sync_invoice(&conn, id, "2026-08-11", &gw).unwrap(), 0); // idempotent
        assert_eq!(paid_amount(&conn, id).unwrap(), 100.0);
        assert_eq!(get_invoice(&conn, id).unwrap().status, "paid");
    }

    #[test]
    fn sync_invoice_without_link_is_a_noop() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        let gw = Gw(vec![PaidSession {
            session_id: "cs_1".into(),
            amount: 100.0,
        }]);
        assert_eq!(sync_invoice(&conn, id, "2026-08-10", &gw).unwrap(), 0);
        assert_eq!(paid_amount(&conn, id).unwrap(), 0.0);
    }

    #[test]
    fn sync_all_skips_drafts_and_settled_invoices() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];

        let owing =
            create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        set_payment_link(&conn, owing, "pl_1", "https://pay/x").unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [owing],
        )
        .unwrap();

        let draft =
            create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        set_payment_link(&conn, draft, "pl_2", "https://pay/y").unwrap();

        let settled =
            create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        set_payment_link(&conn, settled, "pl_3", "https://pay/z").unwrap();
        conn.execute("UPDATE invoices SET status='paid' WHERE id=?1", [settled])
            .unwrap();

        let report = sync_all_report(&conn, "2026-08-10", &PerLinkGw).unwrap();
        assert_eq!(report.recorded, 1);
        assert_eq!(
            report.invoices_checked, 1,
            "only the open one was looked at"
        );
        assert!(report.failures.is_empty());
        assert_eq!(paid_amount(&conn, owing).unwrap(), 100.0);
        assert_eq!(paid_amount(&conn, draft).unwrap(), 0.0);
        assert_eq!(paid_amount(&conn, settled).unwrap(), 0.0);
    }

    fn open_invoice(conn: &rusqlite::Connection, client_id: i64, link: &str) -> i64 {
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            conn,
            client_id,
            "2026-08-04",
            None,
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        set_payment_link(conn, id, link, "https://pay/x").unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();
        id
    }

    #[test]
    fn sync_all_keeps_going_after_a_failing_invoice() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let broken = open_invoice(&conn, cid, "pl_bad");
        let healthy = open_invoice(&conn, cid, "pl_good");

        let gw = FlakyGw {
            broken: vec!["pl_bad"],
        };
        assert_eq!(
            sync_all_report(&conn, "2026-08-10", &gw).unwrap().recorded,
            1
        );
        assert_eq!(paid_amount(&conn, healthy).unwrap(), 100.0);
        assert_eq!(paid_amount(&conn, broken).unwrap(), 0.0);
        assert_eq!(get_invoice(&conn, healthy).unwrap().status, "paid");
    }

    /// The failures are returned, not printed: a browser cannot read the
    /// server's stderr, and the CLI prints them from here.
    #[test]
    fn the_report_names_the_invoices_that_failed() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let broken = open_invoice(&conn, cid, "pl_bad");
        open_invoice(&conn, cid, "pl_good");
        let broken_number = get_invoice(&conn, broken).unwrap().number;

        let gw = FlakyGw {
            broken: vec!["pl_bad"],
        };
        let report = sync_all_report(&conn, "2026-08-10", &gw).unwrap();

        assert_eq!(report.recorded, 1);
        assert_eq!(report.invoices_checked, 2);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].number, broken_number);
        assert!(
            report.failures[0].message.contains("no such payment link"),
            "got: {}",
            report.failures[0].message
        );
    }

    #[test]
    fn sync_all_reports_an_error_when_every_invoice_fails() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        open_invoice(&conn, cid, "pl_bad");
        open_invoice(&conn, cid, "pl_worse");

        let gw = FlakyGw {
            broken: vec!["pl_bad", "pl_worse"],
        };
        let err = sync_all_report(&conn, "2026-08-10", &gw)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no such payment link pl_bad"), "got: {err}");
        assert!(err.contains("no such payment link pl_worse"), "got: {err}");
    }

    #[test]
    fn a_run_with_nothing_open_is_an_empty_report_not_an_error() {
        let (_d, conn) = test_conn();
        let report = sync_all_report(&conn, "2026-08-10", &PerLinkGw).unwrap();
        assert_eq!(report.recorded, 0);
        assert_eq!(report.invoices_checked, 0);
        assert!(report.failures.is_empty());
    }
}

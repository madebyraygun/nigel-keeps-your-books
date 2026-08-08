use chrono::NaiveDate;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::Connection;

use crate::db::{get_metadata, set_metadata};
use crate::error::Result;
use crate::models::{Invoice, InvoiceLineItem, InvoiceStatus};

const NEXT_NUMBER_KEY: &str = "next_invoice_number";
const NEXT_NUMBER_DEFAULT: i64 = 1248;

pub struct NewLineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
}

pub fn gen_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub fn next_number(conn: &Connection) -> Result<i64> {
    let n = get_metadata(conn, NEXT_NUMBER_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(NEXT_NUMBER_DEFAULT);
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
pub fn create_invoice(
    conn: &Connection,
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[NewLineItem],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;

    let number = next_number(&tx)?;
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax = 0.0;
    let total = subtotal + tax;
    let token = gen_token();

    tx.execute(
        "INSERT INTO invoices
            (number, client_id, issue_date, due_date, status, currency, subtotal, tax, total, notes, terms, token)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            number, client_id, issue_date, due_date, currency, subtotal, tax, total, notes, terms, token
        ],
    )?;
    let invoice_id = tx.last_insert_rowid();

    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        tx.execute(
            "INSERT INTO invoice_line_items
                (invoice_id, description, quantity, unit_amount, line_total, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                invoice_id,
                item.description,
                item.quantity,
                item.unit_amount,
                line_total,
                idx as i64
            ],
        )?;
    }

    set_metadata(&tx, NEXT_NUMBER_KEY, &(number + 1).to_string())?;
    tx.commit()?;
    Ok(invoice_id)
}

fn row_to_invoice(r: &rusqlite::Row) -> rusqlite::Result<Invoice> {
    Ok(Invoice {
        id: r.get(0)?,
        number: r.get(1)?,
        client_id: r.get(2)?,
        issue_date: r.get(3)?,
        due_date: r.get(4)?,
        status: r.get(5)?,
        currency: r.get(6)?,
        subtotal: r.get(7)?,
        tax: r.get(8)?,
        total: r.get(9)?,
        notes: r.get(10)?,
        terms: r.get(11)?,
        token: r.get(12)?,
        stripe_payment_link_id: r.get(13)?,
        stripe_payment_link_url: r.get(14)?,
        published_at: r.get(15)?,
    })
}

const INVOICE_COLS: &str = "id, number, client_id, issue_date, due_date, status, currency,
    subtotal, tax, total, notes, terms, token, stripe_payment_link_id,
    stripe_payment_link_url, published_at";

pub fn get_invoice(conn: &Connection, id: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE id = ?1"),
        [id],
        row_to_invoice,
    )?)
}

pub fn get_invoice_by_number(conn: &Connection, number: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE number = ?1"),
        [number],
        row_to_invoice,
    )?)
}

pub fn paid_amount(conn: &Connection, invoice_id: i64) -> Result<f64> {
    let sum: Option<f64> = conn.query_row(
        "SELECT SUM(amount) FROM invoice_payments WHERE invoice_id = ?1",
        [invoice_id],
        |r| r.get(0),
    )?;
    Ok(sum.unwrap_or(0.0))
}

pub fn record_payment(
    conn: &Connection,
    invoice_id: i64,
    amount: f64,
    paid_date: &str,
    method: &str,
    stripe_session: Option<&str>,
) -> Result<bool> {
    if let Some(sid) = stripe_session {
        let seen: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_payments WHERE stripe_checkout_session_id = ?1)",
            [sid],
            |r| r.get(0),
        )?;
        if seen {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method, stripe_checkout_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![invoice_id, amount, paid_date, method, stripe_session],
    )?;
    // The payment date, not the wall clock, is the reference day so the derived
    // status is deterministic regardless of when the payment is entered.
    refresh_status(conn, invoice_id, paid_date)?;
    Ok(true)
}

pub fn refresh_status(conn: &Connection, invoice_id: i64, today: &str) -> Result<String> {
    let inv = get_invoice(conn, invoice_id)?;
    if inv.status == InvoiceStatus::Void.as_str() {
        return Ok(inv.status);
    }
    let paid = paid_amount(conn, invoice_id)?;
    let published = inv.published_at.is_some();
    let owing = inv.total - paid;

    // Half a cent of slack: payments that should sum to the total can land a hair under
    // it in binary floating point, and no real balance is ever settled to finer than 1c.
    let status = if paid >= inv.total - 0.005 && inv.total > 0.0 {
        InvoiceStatus::Paid
    } else if !published {
        InvoiceStatus::Draft
    } else if is_overdue(inv.due_date.as_deref(), today) && owing > 0.0 {
        InvoiceStatus::Overdue
    } else if paid > 0.0 {
        InvoiceStatus::Partial
    } else {
        InvoiceStatus::Sent
    };

    conn.execute(
        "UPDATE invoices SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), invoice_id],
    )?;
    Ok(status.as_str().to_string())
}

fn is_overdue(due_date: Option<&str>, today: &str) -> bool {
    // ISO YYYY-MM-DD dates compare correctly as strings.
    match due_date {
        Some(d) => today > d,
        None => false,
    }
}

pub fn set_payment_link(conn: &Connection, id: i64, link_id: &str, url: &str) -> Result<()> {
    conn.execute(
        "UPDATE invoices SET stripe_payment_link_id = ?1, stripe_payment_link_url = ?2 WHERE id = ?3",
        rusqlite::params![link_id, url, id],
    )?;
    Ok(())
}

pub fn mark_published(conn: &Connection, id: i64, published_at: &str) -> Result<()> {
    conn.execute(
        "UPDATE invoices SET published_at = ?1 WHERE id = ?2",
        rusqlite::params![published_at, id],
    )?;
    refresh_status(conn, id, published_at)?;
    Ok(())
}

pub fn line_items(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoiceLineItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, invoice_id, description, quantity, unit_amount, line_total, position
         FROM invoice_line_items WHERE invoice_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            Ok(InvoiceLineItem {
                id: r.get(0)?,
                invoice_id: r.get(1)?,
                description: r.get(2)?,
                quantity: r.get(3)?,
                unit_amount: r.get(4)?,
                line_total: r.get(5)?,
                position: r.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub struct AgingBucket {
    pub label: &'static str,
    pub total: f64,
}

pub fn ar_aging(conn: &Connection, today: &str) -> Result<Vec<AgingBucket>> {
    let today = NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map_err(|e| crate::error::NigelError::Other(format!("bad date {today}: {e}")))?;

    let mut buckets = [
        AgingBucket {
            label: "current",
            total: 0.0,
        },
        AgingBucket {
            label: "1-30",
            total: 0.0,
        },
        AgingBucket {
            label: "31-60",
            total: 0.0,
        },
        AgingBucket {
            label: "61-90",
            total: 0.0,
        },
        AgingBucket {
            label: "90+",
            total: 0.0,
        },
    ];

    let mut stmt = conn.prepare(
        "SELECT id, total, COALESCE(due_date, issue_date) FROM invoices
         WHERE status IN ('sent','partial','overdue')",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (id, total, due) in rows {
        let owing = total - paid_amount(conn, id)?;
        if owing <= 0.0 {
            continue;
        }
        let due = NaiveDate::parse_from_str(&due, "%Y-%m-%d").unwrap_or(today);
        let days = (today - due).num_days();
        let idx = if days <= 0 {
            0
        } else if days <= 30 {
            1
        } else if days <= 60 {
            2
        } else if days <= 90 {
            3
        } else {
            4
        };
        buckets[idx].total += owing;
    }
    Ok(buckets.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn token_is_16_alphanumeric() {
        let t = gen_token();
        assert_eq!(t.len(), 16);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn first_number_defaults_to_1248_and_increments() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "Design".into(),
                quantity: 2.0,
                unit_amount: 100.0,
            },
            NewLineItem {
                description: "Dev".into(),
                quantity: 1.0,
                unit_amount: 50.0,
            },
        ];
        let id1 = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-09-03"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        let inv1 = get_invoice(&conn, id1).unwrap();
        assert_eq!(inv1.number, 1248);
        assert_eq!(inv1.subtotal, 250.0);
        assert_eq!(inv1.total, 250.0);
        assert_eq!(inv1.status, "draft");

        let id2 =
            create_invoice(&conn, cid, "2026-08-05", None, "USD", &items, None, None).unwrap();
        assert_eq!(get_invoice(&conn, id2).unwrap().number, 1249);
    }

    #[test]
    fn failed_create_rolls_back_and_leaves_numbering_usable() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Design".into(),
            quantity: 1.0,
            unit_amount: 10.0,
        }];

        conn.execute_batch(
            "CREATE TRIGGER fail_line_items BEFORE INSERT ON invoice_line_items
             BEGIN SELECT RAISE(ABORT, 'line item insert failed'); END;",
        )
        .unwrap();
        assert!(create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).is_err());

        let invoices: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap();
        assert_eq!(invoices, 0);
        assert_eq!(next_number(&conn).unwrap(), 1248);

        conn.execute_batch("DROP TRIGGER fail_line_items;").unwrap();
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().number, 1248);
    }

    #[test]
    fn recording_full_payment_marks_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 200.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-09-03"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();

        assert!(record_payment(&conn, id, 200.0, "2026-08-10", "direct_deposit", None).unwrap());
        assert_eq!(paid_amount(&conn, id).unwrap(), 200.0);
        assert_eq!(refresh_status(&conn, id, "2026-08-11").unwrap(), "paid");
    }

    #[test]
    fn partial_then_overdue_is_derived() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 200.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        record_payment(&conn, id, 50.0, "2026-08-10", "ach", None).unwrap();
        assert_eq!(refresh_status(&conn, id, "2026-08-15").unwrap(), "partial");
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "overdue");
    }

    #[test]
    fn stripe_session_is_idempotent() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        assert!(record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap());
        assert!(!record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap());
        assert_eq!(paid_amount(&conn, id).unwrap(), 100.0);
    }

    #[test]
    fn installments_summing_a_hair_short_still_mark_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.20,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        // 33.40 * 3 lands on 100.19999999999999 in binary floating point, a hair under
        // the 100.20 total. The invoice is settled in full and must read as paid.
        for _ in 0..3 {
            record_payment(&conn, id, 33.40, "2026-08-10", "ach", None).unwrap();
        }
        assert!(paid_amount(&conn, id).unwrap() < 100.20);
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "paid");
    }

    #[test]
    fn a_cent_short_is_not_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.00,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1",
            [id],
        )
        .unwrap();

        record_payment(&conn, id, 99.99, "2026-08-10", "ach", None).unwrap();
        assert_eq!(refresh_status(&conn, id, "2026-08-15").unwrap(), "partial");
    }

    #[test]
    fn void_is_never_downgraded() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(
            &conn,
            cid,
            "2026-08-04",
            Some("2026-08-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute("UPDATE invoices SET status='void' WHERE id=?1", [id])
            .unwrap();

        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "void");
        record_payment(&conn, id, 100.0, "2026-08-10", "other", None).unwrap();
        assert_eq!(get_invoice(&conn, id).unwrap().status, "void");
    }

    #[test]
    fn line_items_come_back_in_position_order() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", None, None, None).unwrap();
        let items = vec![
            NewLineItem {
                description: "Design".into(),
                quantity: 2.0,
                unit_amount: 100.0,
            },
            NewLineItem {
                description: "Dev".into(),
                quantity: 3.0,
                unit_amount: 50.0,
            },
        ];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        let rows = line_items(&conn, id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "Design");
        assert_eq!(rows[0].position, 0);
        assert_eq!(rows[0].line_total, 200.0);
        assert_eq!(rows[1].description, "Dev");
        assert_eq!(rows[1].position, 1);
        assert_eq!(rows[1].line_total, 150.0);
        assert_eq!(rows[1].invoice_id, Some(id));
    }

    #[test]
    fn aging_buckets_split_by_days_past_due() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        // due 5 days ago -> "1-30"
        let a = create_invoice(
            &conn,
            cid,
            "2026-07-01",
            Some("2026-07-30"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        // due 45 days ago -> "31-60"
        let b = create_invoice(
            &conn,
            cid,
            "2026-06-01",
            Some("2026-06-20"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE invoices SET status='sent', published_at='x' WHERE id IN (?1,?2)",
            [a, b],
        )
        .unwrap();

        let buckets = ar_aging(&conn, "2026-08-04").unwrap();
        let get = |label: &str| buckets.iter().find(|x| x.label == label).unwrap().total;
        assert_eq!(get("1-30"), 100.0);
        assert_eq!(get("31-60"), 100.0);
        assert_eq!(get("90+"), 0.0);
    }
}

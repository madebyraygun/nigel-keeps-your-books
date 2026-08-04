use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::Connection;

use crate::db::{get_metadata, set_metadata};
use crate::error::Result;
use crate::models::Invoice;

const NEXT_NUMBER_KEY: &str = "next_invoice_number";
const NEXT_NUMBER_DEFAULT: i64 = 1248;

#[allow(dead_code)]
pub struct NewLineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
}

#[allow(dead_code)]
pub fn gen_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

#[allow(dead_code)]
pub fn next_number(conn: &Connection) -> Result<i64> {
    let n = get_metadata(conn, NEXT_NUMBER_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(NEXT_NUMBER_DEFAULT);
    Ok(n)
}

#[allow(dead_code)]
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
    let number = next_number(conn)?;
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax = 0.0;
    let total = subtotal + tax;
    let token = gen_token();

    conn.execute(
        "INSERT INTO invoices
            (number, client_id, issue_date, due_date, status, currency, subtotal, tax, total, notes, terms, token)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            number, client_id, issue_date, due_date, currency, subtotal, tax, total, notes, terms, token
        ],
    )?;
    let invoice_id = conn.last_insert_rowid();

    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        conn.execute(
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

    set_metadata(conn, NEXT_NUMBER_KEY, &(number + 1).to_string())?;
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

#[allow(dead_code)]
pub fn get_invoice(conn: &Connection, id: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE id = ?1"),
        [id],
        row_to_invoice,
    )?)
}

#[allow(dead_code)]
pub fn get_invoice_by_number(conn: &Connection, number: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE number = ?1"),
        [number],
        row_to_invoice,
    )?)
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
}

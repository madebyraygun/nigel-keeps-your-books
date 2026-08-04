use std::path::Path;

use comfy_table::{Cell, Table};
use rusqlite::Connection;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::import_invoiceshelf::import as import_invoiceshelf;
use crate::invoicing::invoices::{
    ar_aging, create_invoice, get_invoice, get_invoice_by_number, line_items, paid_amount,
    record_payment, NewLineItem,
};
use crate::invoicing::mailgun::MailgunClient;
use crate::invoicing::r2::R2Publisher;
use crate::invoicing::send::send_invoice;
use crate::invoicing::stripe::StripeClient;
use crate::invoicing::sync::sync_all;
use crate::models::{Invoice, InvoiceStatus};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};

fn parse_item(s: &str) -> Result<NewLineItem> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(NigelError::Other(format!(
            "bad --item '{s}', want desc:qty:unit"
        )));
    }
    Ok(NewLineItem {
        description: parts[0].to_string(),
        quantity: parts[1].parse().map_err(|_| {
            NigelError::Other(format!("bad quantity '{}' in --item '{s}'", parts[1]))
        })?,
        unit_amount: parts[2].parse().map_err(|_| {
            NigelError::Other(format!("bad unit amount '{}' in --item '{s}'", parts[2]))
        })?,
    })
}

fn parse_items(items: &[String]) -> Result<Vec<NewLineItem>> {
    if items.is_empty() {
        return Err(NigelError::Other(
            "an invoice needs at least one --item \"desc:qty:unit\"".into(),
        ));
    }
    items.iter().map(|s| parse_item(s)).collect()
}

fn find_invoice(conn: &Connection, number: i64) -> Result<Invoice> {
    get_invoice_by_number(conn, number).map_err(|e| match e {
        NigelError::Db(rusqlite::Error::QueryReturnedNoRows) => NigelError::Other(format!(
            "No invoice #{number}. Run `nigel invoice list` to see invoice numbers."
        )),
        other => other,
    })
}

fn ensure_sendable(invoice: &Invoice) -> Result<()> {
    if invoice.status == InvoiceStatus::Void.as_str() {
        return Err(NigelError::Other(format!(
            "Invoice #{} is void and cannot be sent.",
            invoice.number
        )));
    }
    Ok(())
}

fn require(value: Option<String>, what: &str) -> Result<String> {
    value.ok_or_else(|| {
        NigelError::Other(format!(
            "missing invoicing config: {what} (set it in settings.json or the matching NIGEL_ env var)"
        ))
    })
}

fn build_gateway(cfg: &InvoicingConfig) -> Result<StripeClient> {
    Ok(StripeClient {
        secret_key: require(cfg.stripe_secret_key.clone(), "stripe_secret_key")?,
    })
}

fn build_clients(cfg: InvoicingConfig) -> Result<(StripeClient, R2Publisher, MailgunClient)> {
    let stripe = build_gateway(&cfg)?;
    let r2 = R2Publisher {
        account_id: require(cfg.r2_account_id, "r2_account_id")?,
        access_key: require(cfg.r2_access_key, "r2_access_key")?,
        secret_key: require(cfg.r2_secret_key, "r2_secret_key")?,
        bucket: require(cfg.r2_bucket, "r2_bucket")?,
        public_base_url: cfg.public_base_url,
    };
    let mail = MailgunClient {
        api_key: require(cfg.mailgun_api_key, "mailgun_api_key")?,
        domain: cfg.mailgun_domain,
        from: cfg.from_email,
    };
    Ok((stripe, r2, mail))
}

pub fn new(
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[String],
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let parsed = parse_items(items)?;
    let id = create_invoice(
        &conn, client_id, issue_date, due_date, currency, &parsed, None, None,
    )?;
    let invoice = get_invoice(&conn, id)?;
    println!(
        "Created draft invoice #{} for {:.2} {}",
        invoice.number, invoice.total, invoice.currency
    );
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare(
        "SELECT i.number, i.status, c.name, i.total, COALESCE(i.due_date, '')
         FROM invoices i JOIN clients c ON c.id = i.client_id
         ORDER BY i.number DESC",
    )?;
    let rows: Vec<(i64, String, String, f64, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut table = Table::new();
    table.set_header(vec!["#", "Status", "Client", "Total", "Due"]);
    for (number, status, client, total, due) in rows {
        table.add_row(vec![
            Cell::new(number),
            Cell::new(status),
            Cell::new(client),
            Cell::new(format!("{total:.2}")),
            Cell::new(due),
        ]);
    }
    println!("Invoices\n{table}");
    Ok(())
}

pub fn show(number: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;

    println!(
        "Invoice #{}  [{}]  {} {:.2}",
        invoice.number, invoice.status, invoice.currency, invoice.total
    );
    println!("Client:   {}", client.name);
    println!("Issued:   {}", invoice.issue_date);
    println!("Due:      {}", invoice.due_date.as_deref().unwrap_or("-"));

    let mut table = Table::new();
    table.set_header(vec!["Description", "Qty", "Unit", "Amount"]);
    for item in line_items(&conn, invoice.id)? {
        table.add_row(vec![
            Cell::new(item.description),
            Cell::new(format!("{:.2}", item.quantity)),
            Cell::new(format!("{:.2}", item.unit_amount)),
            Cell::new(format!("{:.2}", item.line_total)),
        ]);
    }
    println!("{table}");

    let paid = paid_amount(&conn, invoice.id)?;
    println!("Paid:     {paid:.2}");
    println!("Balance:  {:.2}", invoice.total - paid);
    if let Some(url) = invoice.stripe_payment_link_url {
        println!("Pay:      {url}");
    }
    Ok(())
}

pub fn send(number: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_sendable(&invoice)?;
    let (stripe, r2, mail) = build_clients(invoicing_config())?;
    let url = send_invoice(&conn, invoice.id, today, &stripe, &r2, &mail)?;
    println!("Sent invoice #{number}: {url}");
    Ok(())
}

pub fn sync(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let stripe = build_gateway(&invoicing_config())?;
    let recorded = sync_all(&conn, today, &stripe)?;
    println!("Recorded {recorded} new payment(s)");
    Ok(())
}

pub fn pay(number: i64, amount: Option<f64>, date: &str, method: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let paid = paid_amount(&conn, invoice.id)?;
    let amount = amount.unwrap_or(invoice.total - paid);
    record_payment(&conn, invoice.id, amount, date, method, None)?;
    let invoice = get_invoice(&conn, invoice.id)?;
    println!(
        "Recorded {amount:.2} against invoice #{number} ({})",
        invoice.status
    );
    Ok(())
}

pub fn aging(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    println!("A/R aging as of {today}");
    for bucket in ar_aging(&conn, today)? {
        println!("{:>8}: {:.2}", bucket.label, bucket.total);
    }
    Ok(())
}

pub fn import(db: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let summary = import_invoiceshelf(&conn, Path::new(db))?;
    println!(
        "Imported {} clients, {} invoices, {} payments. Next invoice number: {}",
        summary.clients, summary.invoices, summary.payments, summary.next_number
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices::create_invoice;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// Insert one 100.00 draft invoice (number 1248) and return its row id.
    fn seed_invoice(conn: &Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    #[test]
    fn parse_item_splits_desc_qty_unit() {
        let item = parse_item("Design:2:150.50").unwrap();
        assert_eq!(item.description, "Design");
        assert_eq!(item.quantity, 2.0);
        assert_eq!(item.unit_amount, 150.50);
    }

    #[test]
    fn parse_item_rejects_wrong_shape_and_bad_numbers() {
        assert!(parse_item("Design:2").is_err());
        assert!(parse_item("Design:two:150").is_err());
        assert!(parse_item("Design:2:free").is_err());
    }

    #[test]
    fn an_invoice_needs_at_least_one_item() {
        let err = parse_items(&[]).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("--item"), "got: {err}");
        assert_eq!(parse_items(&["W:1:10".to_string()]).unwrap().len(), 1);
    }

    #[test]
    fn missing_secret_names_the_setting() {
        let cfg = InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: "rygn.io".into(),
            from_email: "billing@rygn.io".into(),
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: "https://billing.rygn.io/i".into(),
        };
        let err = build_clients(cfg).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("stripe_secret_key"), "got: {err}");
    }

    #[test]
    fn unknown_invoice_number_gets_a_readable_error() {
        let (_d, conn) = test_conn();
        let err = find_invoice(&conn, 9999)
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("No invoice #9999"), "got: {err}");
    }

    #[test]
    fn find_invoice_returns_the_matching_invoice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);

        let invoice = find_invoice(&conn, 1248).unwrap();
        assert_eq!(invoice.number, 1248);
        assert_eq!(invoice.total, 100.0);
    }

    #[test]
    fn void_invoices_are_refused_before_any_network_call() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        conn.execute("UPDATE invoices SET status='void' WHERE id=?1", [id])
            .unwrap();

        let invoice = find_invoice(&conn, 1248).unwrap();
        let err = ensure_sendable(&invoice).unwrap_err().to_string();
        assert!(err.contains("void"), "got: {err}");
    }

    #[test]
    fn draft_invoices_are_sendable() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        assert!(ensure_sendable(&find_invoice(&conn, 1248).unwrap()).is_ok());
    }
}

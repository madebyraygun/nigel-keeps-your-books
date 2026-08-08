use rusqlite::Connection;

use crate::error::{NigelError, Result};
use crate::models::Client;

pub fn add_client(
    conn: &Connection,
    name: &str,
    email: Option<&str>,
    billing_address: Option<&str>,
    notes: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO clients (name, email, billing_address, notes) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, email, billing_address, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_client(conn: &Connection, id: i64) -> Result<Client> {
    conn.query_row(
        "SELECT id, name, email, billing_address, notes FROM clients WHERE id = ?1",
        [id],
        |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                billing_address: r.get(3)?,
                notes: r.get(4)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Client not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// Cheap existence probe for callers that only need the id to be real.
pub fn ensure_client_exists(conn: &Connection, id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clients WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(NigelError::NotFound(format!("Client not found: id {id}")))
    }
}

/// Fields to change on a client. `None` leaves a field alone; `Some(None)`
/// clears it — the convention `cli::rules::RuleUpdate` uses for `vendor`.
#[derive(Debug, Default, Clone)]
pub struct ClientUpdate {
    /// NOT NULL in the schema, so it can be renamed but never cleared.
    pub name: Option<String>,
    pub email: Option<Option<String>>,
    pub billing_address: Option<Option<String>>,
    pub notes: Option<Option<String>>,
}

impl ClientUpdate {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.email.is_none()
            && self.billing_address.is_none()
            && self.notes.is_none()
    }
}

/// Apply a partial update to a client.
pub fn update_client(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()> {
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }
    if let Some(ref name) = update.name {
        if name.trim().is_empty() {
            return Err(NigelError::Invalid("Name is required".into()));
        }
    }

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = update.name {
        params.push(Box::new(name.trim().to_string()));
        updates.push(format!("name = ?{}", params.len()));
    }
    if let Some(ref email) = update.email {
        params.push(Box::new(email.clone()));
        updates.push(format!("email = ?{}", params.len()));
    }
    if let Some(ref address) = update.billing_address {
        params.push(Box::new(address.clone()));
        updates.push(format!("billing_address = ?{}", params.len()));
    }
    if let Some(ref notes) = update.notes {
        params.push(Box::new(notes.clone()));
        updates.push(format!("notes = ?{}", params.len()));
    }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE clients SET {} WHERE id = ?{}",
        updates.join(", "),
        params.len()
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    if conn.execute(&sql, param_refs.as_slice())? == 0 {
        return Err(NigelError::NotFound(format!("Client not found: id {id}")));
    }
    Ok(())
}

pub fn list_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt =
        conn.prepare("SELECT id, name, email, billing_address, notes FROM clients ORDER BY name")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                billing_address: r.get(3)?,
                notes: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// One row of a client's invoice history, for `client show`.
#[derive(Debug, Clone)]
pub struct ClientInvoiceRow {
    pub number: i64,
    pub status: String,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub total: f64,
    pub paid: f64,
}

/// A client plus everything `client show` prints, in one round trip.
#[derive(Debug, Clone)]
pub struct ClientSummary {
    pub client: Client,
    /// Newest invoice number first.
    pub invoices: Vec<ClientInvoiceRow>,
    /// Open invoices only, so a paid or voided one contributes nothing.
    pub outstanding: f64,
}

pub fn client_summary(conn: &Connection, id: i64) -> Result<ClientSummary> {
    let client = get_client(conn, id)?;

    let mut stmt = conn.prepare(
        "SELECT i.number, i.status, i.issue_date, i.due_date, i.total,
                COALESCE((SELECT SUM(p.amount) FROM invoice_payments p
                          WHERE p.invoice_id = i.id), 0)
         FROM invoices i WHERE i.client_id = ?1 ORDER BY i.number DESC",
    )?;
    let invoices = stmt
        .query_map([id], |r| {
            Ok(ClientInvoiceRow {
                number: r.get(0)?,
                status: r.get(1)?,
                issue_date: r.get(2)?,
                due_date: r.get(3)?,
                total: r.get(4)?,
                paid: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // The same open-status filter `ar_aging` uses, clamped per row so an
    // overpayment on one invoice cannot pay down another's balance.
    let outstanding = invoices
        .iter()
        .filter(|i| matches!(i.status.as_str(), "sent" | "partial" | "overdue"))
        .map(|i| (i.total - i.paid).max(0.0))
        .sum();

    Ok(ClientSummary {
        client,
        invoices,
        outstanding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::NigelError;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn unknown_client_id_is_not_found() {
        let (_d, conn) = test_conn();
        let err = get_client(&conn, 99).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn ensure_client_exists_passes_for_a_real_client_and_fails_otherwise() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        assert!(ensure_client_exists(&conn, id).is_ok());

        let err = ensure_client_exists(&conn, 99).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn add_and_get_client() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(list_clients(&conn).unwrap().len(), 1);
    }

    fn seed_client(conn: &Connection) -> i64 {
        add_client(
            conn,
            "Acme Co",
            Some("ap@acme.test"),
            Some("123 Main St"),
            Some("pays late"),
        )
        .unwrap()
    }

    #[test]
    fn updating_one_field_leaves_the_others_alone() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(Some("billing@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();

        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.email.as_deref(), Some("billing@acme.test"));
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.billing_address.as_deref(), Some("123 Main St"));
        assert_eq!(c.notes.as_deref(), Some("pays late"));
    }

    #[test]
    fn some_none_clears_a_nullable_field() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(None),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(get_client(&conn, id).unwrap().email, None);
    }

    #[test]
    fn an_empty_client_update_is_rejected() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let err = update_client(&conn, id, &ClientUpdate::default()).unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            "Nothing to update — provide at least one flag"
        );
    }

    #[test]
    fn a_blank_client_name_is_rejected() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let err = update_client(
            &conn,
            id,
            &ClientUpdate {
                name: Some("   ".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Name is required");
        assert_eq!(get_client(&conn, id).unwrap().name, "Acme Co");
    }

    /// One invoice for `client_id` at `total`, left as a draft.
    fn seed_invoice(conn: &Connection, client_id: i64, issue_date: &str, total: f64) -> i64 {
        let items = vec![crate::invoicing::invoices::NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: total,
        }];
        crate::invoicing::invoices::create_invoice(
            conn, client_id, issue_date, None, "USD", &items, None, None,
        )
        .unwrap()
    }

    fn publish(conn: &Connection, invoice_id: i64, on: &str) {
        crate::invoicing::invoices::mark_published(conn, invoice_id, on).unwrap();
    }

    #[test]
    fn summary_lists_a_clients_invoices_newest_first() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);
        seed_invoice(&conn, id, "2026-06-01", 100.0);
        seed_invoice(&conn, id, "2026-07-01", 200.0);
        seed_invoice(&conn, id, "2026-08-01", 300.0);

        let summary = client_summary(&conn, id).unwrap();
        let numbers: Vec<i64> = summary.invoices.iter().map(|i| i.number).collect();
        assert_eq!(numbers, vec![1250, 1249, 1248]);
        assert_eq!(summary.client.name, "Acme Co");
    }

    #[test]
    fn summary_outstanding_counts_only_open_invoices() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let open = seed_invoice(&conn, id, "2026-06-01", 100.0);
        publish(&conn, open, "2026-06-01");
        crate::invoicing::invoices::record_payment(&conn, open, 30.0, "2026-06-10", "ach", None)
            .unwrap();

        let settled = seed_invoice(&conn, id, "2026-07-01", 200.0);
        publish(&conn, settled, "2026-07-01");
        crate::invoicing::invoices::record_payment(
            &conn,
            settled,
            200.0,
            "2026-07-10",
            "ach",
            None,
        )
        .unwrap();

        let cancelled = seed_invoice(&conn, id, "2026-08-01", 500.0);
        publish(&conn, cancelled, "2026-08-01");
        conn.execute(
            "UPDATE invoices SET voided_at = '2026-08-02' WHERE id = ?1",
            [cancelled],
        )
        .unwrap();
        crate::invoicing::invoices::refresh_status(&conn, cancelled, "2026-08-02").unwrap();

        let summary = client_summary(&conn, id).unwrap();
        assert_eq!(summary.outstanding, 70.0);
        assert_eq!(summary.invoices.len(), 3);
    }

    #[test]
    fn summary_for_a_client_with_no_invoices_is_empty_not_an_error() {
        let (_d, conn) = test_conn();
        let id = seed_client(&conn);

        let summary = client_summary(&conn, id).unwrap();
        assert!(summary.invoices.is_empty());
        assert_eq!(summary.outstanding, 0.0);
    }

    #[test]
    fn summary_for_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = client_summary(&conn, 99).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }

    #[test]
    fn updating_a_missing_client_is_not_found() {
        let (_d, conn) = test_conn();
        let err = update_client(
            &conn,
            99,
            &ClientUpdate {
                name: Some("Ghost".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert_eq!(err.to_string(), "Client not found: id 99");
    }
}

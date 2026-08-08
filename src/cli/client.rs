use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::Result;
use crate::invoicing::clients::{
    add_client, client_summary, get_client, list_clients, update_client, ClientUpdate,
};
use crate::models::Client;
use crate::settings::get_data_dir;

pub fn add(name: &str, email: Option<&str>, address: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let id = add_client(&conn, name, email, address, None)?;
    println!("Added client {id}: {name}");
    Ok(())
}

pub fn show(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let summary = client_summary(&conn, id)?;
    let client = &summary.client;

    println!("Client #{}  {}", client.id, client.name);
    println!("Email:    {}", client.email.as_deref().unwrap_or("-"));
    println!(
        "Address:  {}",
        client.billing_address.as_deref().unwrap_or("-")
    );
    println!("Notes:    {}", client.notes.as_deref().unwrap_or("-"));

    if summary.invoices.is_empty() {
        println!("No invoices.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["#", "Status", "Issued", "Total", "Paid"]);
    for row in &summary.invoices {
        table.add_row(vec![
            Cell::new(row.number),
            Cell::new(&row.status),
            Cell::new(&row.issue_date),
            Cell::new(format!("{:.2}", row.total)),
            Cell::new(format!("{:.2}", row.paid)),
        ]);
    }
    println!("{table}");
    println!("Outstanding: {:.2}", summary.outstanding);
    Ok(())
}

pub fn edit(
    id: i64,
    name: Option<String>,
    email: Option<String>,
    address: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let update = ClientUpdate {
        name,
        email: email.map(Some),
        billing_address: address.map(Some),
        notes: notes.map(Some),
    };
    update_client(&conn, id, &update)?;
    let client = get_client(&conn, id)?;
    println!("Updated client {id}: {}", client.name);
    Ok(())
}

/// `nigel client list`, as text. Pure, so the parity fixtures can call it
/// without a terminal — the same shape `cli/report/text.rs` uses.
pub fn format_client_list(clients: &[Client]) -> String {
    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Email"]);
    for c in clients {
        table.add_row(vec![
            Cell::new(c.id),
            Cell::new(&c.name),
            // A client with no email reads as an em dash, never an empty cell —
            // the missing address is the reason a send will refuse.
            Cell::new(c.email.as_deref().unwrap_or("\u{2014}")),
        ]);
    }
    format!("Clients\n{table}")
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    println!("{}", format_client_list(&list_clients(&conn)?));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(id: i64, name: &str, email: Option<&str>) -> Client {
        Client {
            id,
            name: name.into(),
            email: email.map(str::to_string),
            billing_address: None,
            notes: None,
        }
    }

    /// Byte-for-byte what `nigel client list` prints.
    #[test]
    fn format_client_list_prints_the_columns_it_always_has() {
        let out = format_client_list(&[
            client(1, "Acme Co", Some("ap@acme.test")),
            client(2, "Globex", None),
        ]);
        assert_eq!(
            out,
            concat!(
                "Clients\n",
                "+----+---------+--------------+\n",
                "| ID | Name    | Email        |\n",
                "+=============================+\n",
                "| 1  | Acme Co | ap@acme.test |\n",
                "|----+---------+--------------|\n",
                "| 2  | Globex  | \u{2014}            |\n",
                "+----+---------+--------------+",
            )
        );
    }

    #[test]
    fn format_client_list_prints_an_em_dash_for_a_client_with_no_email() {
        let out = format_client_list(&[client(2, "Globex", None)]);
        assert!(out.contains('\u{2014}'), "want an em dash, got:\n{out}");

        let out = format_client_list(&[client(1, "Acme Co", Some("ap@acme.test"))]);
        assert!(
            !out.contains('\u{2014}'),
            "a client with an email gets no dash, got:\n{out}"
        );
    }

    #[test]
    fn format_client_list_with_no_clients_is_the_bare_heading_and_header() {
        let out = format_client_list(&[]);
        assert!(out.starts_with("Clients\n"), "got: {out}");
        assert!(out.contains("Email"), "got: {out}");
    }
}

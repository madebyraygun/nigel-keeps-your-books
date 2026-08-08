use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::Result;
use crate::invoicing::clients::{
    add_client, client_summary, get_client, list_clients, update_client, ClientUpdate,
};
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

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Email"]);
    for c in list_clients(&conn)? {
        table.add_row(vec![
            Cell::new(c.id),
            Cell::new(c.name),
            Cell::new(c.email.unwrap_or_default()),
        ]);
    }
    println!("Clients\n{table}");
    Ok(())
}

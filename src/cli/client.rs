use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::Result;
use crate::invoicing::clients::{add_client, list_clients};
use crate::settings::get_data_dir;

pub fn add(name: &str, email: Option<&str>, address: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let id = add_client(&conn, name, email, address, None)?;
    println!("Added client {id}: {name}");
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

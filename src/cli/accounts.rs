use comfy_table::{Table, Cell};

use crate::db::get_connection;
use crate::error::Result;
use crate::settings::get_data_dir;

pub fn add(name: &str, account_type: &str, institution: Option<&str>, last_four: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, account_type, institution, last_four],
    )?;
    println!("Added account: {name}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare("SELECT id, name, account_type, institution, last_four FROM accounts")?;
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Type", "Institution", "Last Four"]);
    for (id, name, acct_type, inst, last) in rows {
        table.add_row(vec![
            Cell::new(id),
            Cell::new(name),
            Cell::new(acct_type),
            Cell::new(inst.unwrap_or_default()),
            Cell::new(last.unwrap_or_default()),
        ]);
    }
    println!("Accounts\n{table}");
    Ok(())
}

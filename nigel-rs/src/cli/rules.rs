use comfy_table::{Table, Cell};

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::settings::get_data_dir;

pub fn add(
    pattern: &str,
    category: &str,
    vendor: Option<&str>,
    match_type: &str,
    priority: i64,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let cat_id: i64 = conn
        .query_row("SELECT id FROM categories WHERE name = ?1", [category], |row| {
            row.get(0)
        })
        .map_err(|_| NigelError::UnknownCategory(category.to_string()))?;

    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![pattern, match_type, vendor, cat_id, priority],
    )?;
    println!("Added rule: '{pattern}' \u{2192} {category}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name as category, r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.is_active = 1 ORDER BY r.priority DESC",
    )?;
    let rows: Vec<(i64, String, String, Option<String>, String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut table = Table::new();
    table.set_header(vec!["ID", "Pattern", "Type", "Vendor", "Category", "Priority", "Hits"]);
    for (id, pattern, match_type, vendor, category, priority, hits) in rows {
        table.add_row(vec![
            Cell::new(id),
            Cell::new(pattern),
            Cell::new(match_type),
            Cell::new(vendor.unwrap_or_default()),
            Cell::new(category),
            Cell::new(priority),
            Cell::new(hits),
        ]);
    }
    println!("Rules\n{table}");
    Ok(())
}

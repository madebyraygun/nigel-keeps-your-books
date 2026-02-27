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

pub fn update(
    id: i64,
    pattern: Option<String>,
    category: Option<String>,
    vendor: Option<String>,
    match_type: Option<String>,
    priority: Option<i64>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    // Verify rule exists and is active
    let exists: bool = conn
        .query_row(
            "SELECT is_active FROM rules WHERE id = ?1",
            [id],
            |row| row.get::<_, i32>(0),
        )
        .map(|a| a == 1)
        .map_err(|_| NigelError::Other(format!("No rule with ID {id}")))?;

    if !exists {
        return Err(NigelError::Other(format!("Rule {id} is inactive")));
    }

    if pattern.is_none()
        && category.is_none()
        && vendor.is_none()
        && match_type.is_none()
        && priority.is_none()
    {
        return Err(NigelError::Other(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref p) = pattern {
        params.push(Box::new(p.clone()));
        updates.push(format!("pattern = ?{}", params.len()));
    }
    if let Some(ref mt) = match_type {
        params.push(Box::new(mt.clone()));
        updates.push(format!("match_type = ?{}", params.len()));
    }
    if let Some(ref v) = vendor {
        params.push(Box::new(v.clone()));
        updates.push(format!("vendor = ?{}", params.len()));
    }
    if let Some(ref cat) = category {
        let cat_id: i64 = conn
            .query_row("SELECT id FROM categories WHERE name = ?1", [cat], |row| {
                row.get(0)
            })
            .map_err(|_| NigelError::UnknownCategory(cat.clone()))?;
        params.push(Box::new(cat_id));
        updates.push(format!("category_id = ?{}", params.len()));
    }
    if let Some(pri) = priority {
        params.push(Box::new(pri));
        updates.push(format!("priority = ?{}", params.len()));
    }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE rules SET {} WHERE id = ?{}",
        updates.join(", "),
        params.len()
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    println!("Updated rule {id}");
    Ok(())
}

pub fn delete(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let row: std::result::Result<(String, String, i32), _> = conn.query_row(
        "SELECT r.pattern, c.name, r.is_active FROM rules r JOIN categories c ON r.category_id = c.id WHERE r.id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );

    match row {
        Err(_) => Err(NigelError::Other(format!("No rule with ID {id}"))),
        Ok((_, _, 0)) => Err(NigelError::Other(format!("Rule {id} is already inactive"))),
        Ok((pattern, category, _)) => {
            conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])?;
            println!("Deleted rule {id}: '{pattern}' \u{2192} {category}");
            Ok(())
        }
    }
}

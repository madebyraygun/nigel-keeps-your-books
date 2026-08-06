use std::collections::HashMap;

use comfy_table::{Cell, Table};
use regex::Regex;
use rusqlite::Connection;
use serde::Serialize;

use crate::categorizer::matches as rule_matches;
use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::settings::get_data_dir;

/// An active categorization rule joined to the category it assigns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: i64,
    pub pattern: String,
    pub match_type: String,
    pub vendor: Option<String>,
    pub category: String,
    pub category_id: i64,
    pub priority: i64,
    pub hit_count: i64,
}

/// Active rules in the order the categorizer applies them: highest priority
/// first, ties broken by insertion order so the sequence is stable.
pub fn list_rules(conn: &Connection) -> Result<Vec<RuleRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id, \
                r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.is_active = 1 \
         ORDER BY r.priority DESC, r.id ASC",
    )?;
    let rules = stmt
        .query_map([], |row| {
            Ok(RuleRow {
                id: row.get(0)?,
                pattern: row.get(1)?,
                match_type: row.get(2)?,
                vendor: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                priority: row.get(6)?,
                hit_count: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rules)
}

pub fn add(
    pattern: &str,
    category: &str,
    vendor: Option<&str>,
    match_type: &str,
    priority: i64,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let cat_id: i64 = conn
        .query_row(
            "SELECT id FROM categories WHERE name = ?1",
            [category],
            |row| row.get(0),
        )
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

    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Pattern", "Type", "Vendor", "Category", "Priority", "Hits",
    ]);
    for rule in list_rules(&conn)? {
        table.add_row(vec![
            Cell::new(rule.id),
            Cell::new(rule.pattern),
            Cell::new(rule.match_type),
            Cell::new(rule.vendor.unwrap_or_default()),
            Cell::new(rule.category),
            Cell::new(rule.priority),
            Cell::new(rule.hit_count),
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
    let is_active: i32 =
        match conn.query_row("SELECT is_active FROM rules WHERE id = ?1", [id], |row| {
            row.get::<_, i32>(0)
        }) {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(NigelError::Other(format!("No rule with ID {id}")));
            }
            Err(e) => return Err(e.into()),
        };

    if is_active == 0 {
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
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(NigelError::Other(format!("No rule with ID {id}")))
        }
        Err(e) => Err(e.into()),
        Ok((_, _, 0)) => Err(NigelError::Other(format!("Rule {id} is already inactive"))),
        Ok((pattern, category, _)) => {
            conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])?;
            println!("Deleted rule {id}: '{pattern}' \u{2192} {category}");
            Ok(())
        }
    }
}

pub fn test(pattern: &str, match_type: &str) -> Result<()> {
    let valid_types = ["contains", "starts_with", "regex"];
    if !valid_types.contains(&match_type) {
        return Err(NigelError::Other(format!(
            "Invalid match type: {match_type}. Must be one of: {}",
            valid_types.join(", ")
        )));
    }

    if match_type == "regex" {
        Regex::new(pattern).map_err(|e| NigelError::Other(format!("Invalid regex: {e}")))?;
    }

    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare("SELECT description FROM transactions")?;
    let descriptions: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut match_counts: HashMap<String, usize> = HashMap::new();
    for desc in &descriptions {
        if rule_matches(desc, pattern, match_type) {
            *match_counts.entry(desc.clone()).or_default() += 1;
        }
    }

    let total: usize = match_counts.values().sum();
    if total == 0 {
        println!("No transactions match pattern \"{pattern}\" ({match_type})");
        return Ok(());
    }

    let mut sorted: Vec<_> = match_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    println!(
        "Would match {total} transaction{}:",
        if total == 1 { "" } else { "s" }
    );
    for (desc, count) in &sorted {
        println!("  {desc} ({count})");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::{get_connection, init_db};
    use rusqlite::Connection;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn add_rule(conn: &Connection, pattern: &str) -> i64 {
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', 'TestVendor', ?2, 0, 1)",
            rusqlite::params![pattern, cat_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn add_rule_with(conn: &Connection, pattern: &str, vendor: Option<&str>, priority: i64) -> i64 {
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', ?2, ?3, ?4, 1)",
            rusqlite::params![pattern, vendor, cat_id, priority],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn test_delete_deactivates_rule() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let active: i32 = conn
            .query_row("SELECT is_active FROM rules WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(active, 1);

        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        let active: i32 = conn
            .query_row("SELECT is_active FROM rules WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(active, 0);
    }

    #[test]
    fn test_delete_preserves_hit_count() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        conn.execute("UPDATE rules SET hit_count = 42 WHERE id = ?1", [id])
            .unwrap();
        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        let hits: i64 = conn
            .query_row("SELECT hit_count FROM rules WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(hits, 42);
    }

    #[test]
    fn test_update_changes_priority() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        conn.execute("UPDATE rules SET priority = 10 WHERE id = ?1", [id])
            .unwrap();
        let pri: i64 = conn
            .query_row("SELECT priority FROM rules WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(pri, 10);
    }

    #[test]
    fn test_update_changes_pattern() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        conn.execute("UPDATE rules SET pattern = 'PHOTOSHOP' WHERE id = ?1", [id])
            .unwrap();
        let pat: String = conn
            .query_row("SELECT pattern FROM rules WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(pat, "PHOTOSHOP");
    }

    #[test]
    fn list_rules_orders_by_priority_then_id() {
        let (_dir, conn) = test_db();
        let low = add_rule_with(&conn, "LOW", None, 0);
        let high = add_rule_with(&conn, "HIGH", None, 10);
        let also_high = add_rule_with(&conn, "ALSO", None, 10);

        let ids: Vec<i64> = super::list_rules(&conn)
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![high, also_high, low]);
    }

    #[test]
    fn list_rules_keeps_a_null_vendor_null() {
        let (_dir, conn) = test_db();
        add_rule_with(&conn, "NOVENDOR", None, 0);
        add_rule_with(&conn, "VENDOR", Some("Adobe"), 0);

        let rules = super::list_rules(&conn).unwrap();
        let by_pattern = |p: &str| {
            rules
                .iter()
                .find(|r| r.pattern == p)
                .unwrap_or_else(|| panic!("no rule {p}"))
        };
        assert_eq!(by_pattern("NOVENDOR").vendor, None);
        assert_eq!(by_pattern("VENDOR").vendor, Some("Adobe".to_string()));
    }

    #[test]
    fn list_rules_excludes_inactive_rules_and_carries_the_category() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let rule = super::list_rules(&conn).unwrap().remove(0);
        assert_eq!(rule.id, id);
        assert_eq!(rule.category, "Software & Subscriptions");
        let expected_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rule.category_id, expected_id);

        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        assert!(super::list_rules(&conn).unwrap().is_empty());
    }

    #[test]
    fn rule_row_serializes_camel_case() {
        let (_dir, conn) = test_db();
        add_rule(&conn, "ADOBE");
        let rule = super::list_rules(&conn).unwrap().remove(0);
        let json = serde_json::to_value(&rule).unwrap();
        for key in ["matchType", "categoryId", "hitCount"] {
            assert!(json.get(key).is_some(), "missing {key} in {json}");
        }
    }

    #[test]
    fn test_inactive_rule_excluded_from_active_list() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM rules WHERE is_active = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM rules WHERE is_active = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count_after, count_before - 1);
    }
}

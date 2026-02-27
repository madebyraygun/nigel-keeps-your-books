use regex::Regex;
use rusqlite::Connection;

use crate::error::Result;

fn matches(description: &str, pattern: &str, match_type: &str) -> bool {
    let desc_upper = description.to_uppercase();
    let pat_upper = pattern.to_uppercase();
    match match_type {
        "contains" => desc_upper.contains(&pat_upper),
        "starts_with" => desc_upper.starts_with(&pat_upper),
        "regex" => Regex::new(pattern)
            .map(|re| re.is_match(description))
            .unwrap_or(false),
        _ => false,
    }
}

pub struct CategorizeResult {
    pub categorized: usize,
    pub still_flagged: usize,
}

pub fn categorize_transactions(conn: &Connection) -> Result<CategorizeResult> {
    let mut rule_stmt = conn.prepare(
        "SELECT id, pattern, match_type, vendor, category_id FROM rules \
         WHERE is_active = 1 ORDER BY priority DESC",
    )?;
    let rules: Vec<(i64, String, String, Option<String>, i64)> = rule_stmt
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

    let mut txn_stmt =
        conn.prepare("SELECT id, description FROM transactions WHERE category_id IS NULL")?;
    let flagged: Vec<(i64, String)> = txn_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut categorized = 0usize;
    let mut still_flagged = 0usize;

    for (txn_id, description) in &flagged {
        let mut matched = false;
        for (rule_id, pattern, match_type, vendor, category_id) in &rules {
            if matches(description, pattern, match_type) {
                conn.execute(
                    "UPDATE transactions SET category_id = ?1, vendor = ?2, is_flagged = 0, flag_reason = NULL WHERE id = ?3",
                    rusqlite::params![category_id, vendor, txn_id],
                )?;
                conn.execute(
                    "UPDATE rules SET hit_count = hit_count + 1 WHERE id = ?1",
                    [rule_id],
                )?;
                categorized += 1;
                matched = true;
                break;
            }
        }
        if !matched {
            still_flagged += 1;
        }
    }

    Ok(CategorizeResult {
        categorized,
        still_flagged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn setup_account_and_txns(conn: &Connection, descriptions: &[&str]) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')", [],
        ).unwrap();
        let account_id = conn.last_insert_rowid();
        for desc in descriptions {
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
                 VALUES (?1, '2025-01-15', ?2, -50.00, 1, 'No matching rule')",
                rusqlite::params![account_id, desc],
            ).unwrap();
        }
    }

    fn add_rule(conn: &Connection, pattern: &str, match_type: &str, category_name: &str, priority: i64) {
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = ?1", [category_name], |r| r.get(0),
        ).unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, category_id, priority, is_active) VALUES (?1, ?2, ?3, ?4, 1)",
            rusqlite::params![pattern, match_type, cat_id, priority],
        ).unwrap();
    }

    #[test]
    fn test_contains_rule() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["ADOBE CREATIVE CLOUD"]);
        add_rule(&conn, "adobe", "contains", "Software & Subscriptions", 0);
        let result = categorize_transactions(&conn).unwrap();
        assert_eq!(result.categorized, 1);
        assert_eq!(result.still_flagged, 0);
    }

    #[test]
    fn test_starts_with_rule() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["STRIPE PAYMENT", "PAY STRIPE FEE"]);
        add_rule(&conn, "STRIPE", "starts_with", "Bank & Merchant Fees", 0);
        let result = categorize_transactions(&conn).unwrap();
        assert_eq!(result.categorized, 1);
        assert_eq!(result.still_flagged, 1);
    }

    #[test]
    fn test_regex_rule() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["AWS Services 12345"]);
        add_rule(&conn, r"^AWS.*\d+$", "regex", "Hosting & Infrastructure", 0);
        let result = categorize_transactions(&conn).unwrap();
        assert_eq!(result.categorized, 1);
    }

    #[test]
    fn test_higher_priority_wins() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["PAYMENT RECEIVED"]);
        add_rule(&conn, "PAYMENT", "contains", "Client Services", 10);
        add_rule(&conn, "PAYMENT", "contains", "Bank & Merchant Fees", 5);
        let result = categorize_transactions(&conn).unwrap();
        assert_eq!(result.categorized, 1);
        let cat_name: String = conn.query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id LIMIT 1",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(cat_name, "Client Services");
    }

    #[test]
    fn test_unmatched_stays_flagged() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["RANDOM VENDOR XYZ"]);
        let result = categorize_transactions(&conn).unwrap();
        assert_eq!(result.categorized, 0);
        assert_eq!(result.still_flagged, 1);
    }

    #[test]
    fn test_hit_count_incremented() {
        let (_dir, conn) = test_db();
        setup_account_and_txns(&conn, &["ADOBE PHOTOSHOP", "ADOBE ILLUSTRATOR"]);
        add_rule(&conn, "ADOBE", "contains", "Software & Subscriptions", 0);
        categorize_transactions(&conn).unwrap();
        let hit_count: i64 = conn.query_row(
            "SELECT hit_count FROM rules LIMIT 1", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(hit_count, 2);
    }
}

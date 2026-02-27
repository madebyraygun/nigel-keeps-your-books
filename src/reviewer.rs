use rusqlite::Connection;

use crate::error::Result;

pub struct FlaggedTxn {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub account_name: String,
}

pub struct CategoryChoice {
    pub id: i64,
    pub name: String,
    pub category_type: String,
}

pub fn get_flagged_transactions(conn: &Connection) -> Result<Vec<FlaggedTxn>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name \
         FROM transactions t JOIN accounts a ON t.account_id = a.id \
         WHERE t.is_flagged = 1 ORDER BY t.date",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FlaggedTxn {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                account_name: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_categories(conn: &Connection) -> Result<Vec<CategoryChoice>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category_type FROM categories WHERE is_active = 1 ORDER BY category_type, name",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CategoryChoice {
                id: row.get(0)?,
                name: row.get(1)?,
                category_type: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn apply_review(
    conn: &Connection,
    transaction_id: i64,
    category_id: i64,
    vendor: Option<&str>,
    create_rule: bool,
    rule_pattern: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE transactions SET category_id = ?1, vendor = ?2, is_flagged = 0, flag_reason = NULL WHERE id = ?3",
        rusqlite::params![category_id, vendor, transaction_id],
    )?;
    if create_rule {
        if let Some(pattern) = rule_pattern {
            conn.execute(
                "INSERT INTO rules (pattern, match_type, vendor, category_id) VALUES (?1, 'contains', ?2, ?3)",
                rusqlite::params![pattern, vendor, category_id],
            )?;
        }
    }
    Ok(())
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

    fn add_flagged_txn(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')", [],
        ).unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, '2025-01-15', 'ADOBE CREATIVE', -50.0, 1, 'No matching rule')",
            rusqlite::params![acct],
        ).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn test_get_flagged_transactions() {
        let (_dir, conn) = test_db();
        add_flagged_txn(&conn);
        let flagged = get_flagged_transactions(&conn).unwrap();
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].description, "ADOBE CREATIVE");
        assert!(flagged[0].amount < 0.0);
    }

    #[test]
    fn test_apply_review_categorizes() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();
        apply_review(&conn, txn_id, cat_id, Some("Adobe"), false, None).unwrap();
        let (is_flagged, vendor): (i32, Option<String>) = conn.query_row(
            "SELECT is_flagged, vendor FROM transactions WHERE id = ?1", [txn_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(is_flagged, 0);
        assert_eq!(vendor.as_deref(), Some("Adobe"));
    }

    #[test]
    fn test_apply_review_creates_rule() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();
        apply_review(&conn, txn_id, cat_id, Some("Adobe"), true, Some("ADOBE")).unwrap();
        let count: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        let pattern: String = conn.query_row(
            "SELECT pattern FROM rules LIMIT 1", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(pattern, "ADOBE");
    }
}

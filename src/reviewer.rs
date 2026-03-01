use rusqlite::Connection;

use crate::error::{NigelError, Result};

#[derive(Debug)]
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
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_transaction_by_id(conn: &Connection, id: i64) -> Result<FlaggedTxn> {
    conn.query_row(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name \
         FROM transactions t JOIN accounts a ON t.account_id = a.id \
         WHERE t.id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(FlaggedTxn {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                account_name: row.get(4)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::Other(format!("No transaction found with ID {id}"))
        }
        other => other.into(),
    })
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
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn apply_review(
    conn: &Connection,
    transaction_id: i64,
    category_id: i64,
    vendor: Option<&str>,
    create_rule: bool,
    rule_pattern: Option<&str>,
) -> Result<Option<i64>> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE transactions SET category_id = ?1, vendor = ?2, is_flagged = 0, flag_reason = NULL WHERE id = ?3",
        rusqlite::params![category_id, vendor, transaction_id],
    )?;
    let rule_id = if create_rule {
        if let Some(pattern) = rule_pattern {
            tx.execute(
                "INSERT INTO rules (pattern, match_type, vendor, category_id) VALUES (?1, 'contains', ?2, ?3)",
                rusqlite::params![pattern, vendor, category_id],
            )?;
            Some(conn.last_insert_rowid())
        } else {
            None
        }
    } else {
        None
    };
    tx.commit()?;
    Ok(rule_id)
}

pub fn undo_review(
    conn: &Connection,
    transaction_id: i64,
    rule_id: Option<i64>,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE transactions SET category_id = NULL, vendor = NULL, is_flagged = 1, flag_reason = 'Uncategorized' WHERE id = ?1",
        rusqlite::params![transaction_id],
    )?;
    if let Some(rid) = rule_id {
        tx.execute("DELETE FROM rules WHERE id = ?1", rusqlite::params![rid])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn update_transaction_category(
    conn: &Connection,
    transaction_id: i64,
    category_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE transactions SET category_id = ?1 WHERE id = ?2",
        rusqlite::params![category_id, transaction_id],
    )?;
    Ok(())
}

pub fn update_transaction_vendor(
    conn: &Connection,
    transaction_id: i64,
    vendor: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE transactions SET vendor = ?1 WHERE id = ?2",
        rusqlite::params![vendor, transaction_id],
    )?;
    Ok(())
}

pub fn toggle_transaction_flag(conn: &Connection, transaction_id: i64) -> Result<bool> {
    conn.execute(
        "UPDATE transactions SET is_flagged = NOT is_flagged WHERE id = ?1",
        rusqlite::params![transaction_id],
    )?;
    let new_state: bool = conn.query_row(
        "SELECT is_flagged FROM transactions WHERE id = ?1",
        rusqlite::params![transaction_id],
        |row| row.get(0),
    )?;
    Ok(new_state)
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
    fn test_get_transaction_by_id() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let txn = get_transaction_by_id(&conn, txn_id).unwrap();
        assert_eq!(txn.id, txn_id);
        assert_eq!(txn.description, "ADOBE CREATIVE");
        assert_eq!(txn.account_name, "Test");
    }

    #[test]
    fn test_get_transaction_by_id_not_found() {
        let (_dir, conn) = test_db();
        let result = get_transaction_by_id(&conn, 99999);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No transaction found with ID 99999"));
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
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), false, None).unwrap();
        assert!(rule_id.is_none());
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
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), true, Some("ADOBE")).unwrap();
        assert!(rule_id.is_some());
        let count: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        let pattern: String = conn.query_row(
            "SELECT pattern FROM rules LIMIT 1", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(pattern, "ADOBE");
    }

    #[test]
    fn test_undo_review() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), true, Some("ADOBE")).unwrap();
        assert!(rule_id.is_some());

        undo_review(&conn, txn_id, rule_id).unwrap();

        let (is_flagged, category_id, vendor): (i32, Option<i64>, Option<String>) = conn.query_row(
            "SELECT is_flagged, category_id, vendor FROM transactions WHERE id = ?1", [txn_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(is_flagged, 1);
        assert!(category_id.is_none());
        assert!(vendor.is_none());

        let count: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_undo_review_without_rule() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();
        // Categorize without creating a rule
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), false, None).unwrap();
        assert!(rule_id.is_none());

        undo_review(&conn, txn_id, None).unwrap();

        let (is_flagged, category_id): (i32, Option<i64>) = conn.query_row(
            "SELECT is_flagged, category_id FROM transactions WHERE id = ?1", [txn_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(is_flagged, 1);
        assert!(category_id.is_none());
    }

    #[test]
    fn test_update_transaction_category() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();

        update_transaction_category(&conn, txn_id, cat_id).unwrap();

        let stored: Option<i64> = conn.query_row(
            "SELECT category_id FROM transactions WHERE id = ?1", [txn_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(stored, Some(cat_id));
    }

    #[test]
    fn test_update_transaction_vendor() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);

        update_transaction_vendor(&conn, txn_id, Some("Adobe")).unwrap();

        let vendor: Option<String> = conn.query_row(
            "SELECT vendor FROM transactions WHERE id = ?1", [txn_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(vendor.as_deref(), Some("Adobe"));

        // Clear vendor
        update_transaction_vendor(&conn, txn_id, None).unwrap();
        let vendor2: Option<String> = conn.query_row(
            "SELECT vendor FROM transactions WHERE id = ?1", [txn_id], |r| r.get(0),
        ).unwrap();
        assert!(vendor2.is_none());
    }

    #[test]
    fn test_toggle_transaction_flag() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn); // starts flagged

        let new_state = toggle_transaction_flag(&conn, txn_id).unwrap();
        assert!(!new_state); // was flagged, now unflagged

        let new_state2 = toggle_transaction_flag(&conn, txn_id).unwrap();
        assert!(new_state2); // toggled back to flagged
    }
}

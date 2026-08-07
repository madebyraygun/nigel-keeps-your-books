use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedTxn {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub account_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
            NigelError::NotFound(format!("No transaction found with ID {id}"))
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
            Some(tx.last_insert_rowid())
        } else {
            None
        }
    } else {
        None
    };
    tx.commit()?;
    Ok(rule_id)
}

pub fn undo_review(conn: &Connection, transaction_id: i64, rule_id: Option<i64>) -> Result<()> {
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

/// The flag as it stands, erroring if the transaction is not there.
pub fn transaction_flag(conn: &Connection, transaction_id: i64) -> Result<bool> {
    conn.query_row(
        "SELECT is_flagged FROM transactions WHERE id = ?1",
        rusqlite::params![transaction_id],
        |row| row.get(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No transaction found with ID {transaction_id}"))
        }
        other => other.into(),
    })
}

/// Set the flag to a given state. Idempotent, which is what an HTTP client
/// needs — a toggle would drift out of step with the screen it came from.
///
/// `flag_reason` is deliberately untouched: the flag is the state, the reason
/// is the story of how it got there, and clearing a flag by hand does not
/// rewrite that story.
pub fn set_transaction_flag(conn: &Connection, transaction_id: i64, flagged: bool) -> Result<()> {
    let updated = conn.execute(
        "UPDATE transactions SET is_flagged = ?1 WHERE id = ?2",
        rusqlite::params![flagged, transaction_id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!(
            "No transaction found with ID {transaction_id}"
        )));
    }
    Ok(())
}

/// Flip the flag and report the new state — what the register's `f` key does.
pub fn toggle_transaction_flag(conn: &Connection, transaction_id: i64) -> Result<bool> {
    let new_state = !transaction_flag(conn, transaction_id)?;
    set_transaction_flag(conn, transaction_id, new_state)?;
    Ok(new_state)
}

/// Filter for selecting transactions to recategorize. All set fields are ANDed.
/// Amount bounds compare against the absolute transaction amount.
pub struct RecategorizeFilter {
    pub from_category_id: Option<i64>,
    pub uncategorized: bool,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub pattern: Option<String>,
    pub match_type: String,
    pub account_id: Option<i64>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
}

// Manual impl so `..Default::default()` yields a usable match_type — the derived
// empty string would make any pattern silently match nothing.
impl Default for RecategorizeFilter {
    fn default() -> Self {
        Self {
            from_category_id: None,
            uncategorized: false,
            year: None,
            month: None,
            from_date: None,
            to_date: None,
            pattern: None,
            match_type: "contains".to_string(),
            account_id: None,
            min_amount: None,
            max_amount: None,
        }
    }
}

#[derive(Debug)]
pub struct RecategorizeCandidate {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category_id: Option<i64>,
    pub category: Option<String>,
}

fn candidate_from_row(row: &rusqlite::Row) -> rusqlite::Result<RecategorizeCandidate> {
    Ok(RecategorizeCandidate {
        id: row.get(0)?,
        date: row.get(1)?,
        description: row.get(2)?,
        amount: row.get(3)?,
        category_id: row.get(4)?,
        category: row.get(5)?,
    })
}

pub fn find_transactions_for_recategorize(
    conn: &Connection,
    filter: &RecategorizeFilter,
) -> Result<Vec<RecategorizeCandidate>> {
    // date_filter's clause numbers its placeholders from ?1, so it must come first
    // and its params must lead the params vec.
    let (date_clause, date_params) = crate::reports::date_filter(
        filter.year,
        filter.month,
        filter.from_date.as_deref(),
        filter.to_date.as_deref(),
    )?;
    let mut clauses = vec![date_clause];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = date_params
        .into_iter()
        .map(|p| Box::new(p) as Box<dyn rusqlite::types::ToSql>)
        .collect();

    if let Some(cat_id) = filter.from_category_id {
        params.push(Box::new(cat_id));
        clauses.push(format!("t.category_id = ?{}", params.len()));
    }
    if filter.uncategorized {
        clauses.push("t.category_id IS NULL".to_string());
    }
    if let Some(acct_id) = filter.account_id {
        params.push(Box::new(acct_id));
        clauses.push(format!("t.account_id = ?{}", params.len()));
    }
    if let Some(min) = filter.min_amount {
        params.push(Box::new(min));
        clauses.push(format!("ABS(t.amount) >= ?{}", params.len()));
    }
    if let Some(max) = filter.max_amount {
        params.push(Box::new(max));
        clauses.push(format!("ABS(t.amount) <= ?{}", params.len()));
    }

    let sql = format!(
        "SELECT t.id, t.date, t.description, t.amount, t.category_id, c.name \
         FROM transactions t LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {} ORDER BY t.date, t.id",
        clauses.join(" AND ")
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows: Vec<RecategorizeCandidate> = stmt
        .query_map(param_refs.as_slice(), candidate_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Regex (and the other match types) filter in Rust via the categorizer, so the
    // semantics match rules exactly; SQLite has no regex function loaded. The
    // categorizer treats a bad match type or regex as "matches nothing", which
    // here would silently select zero rows — reject both up front instead.
    if let Some(ref pattern) = filter.pattern {
        let valid_types = ["contains", "starts_with", "regex"];
        if !valid_types.contains(&filter.match_type.as_str()) {
            return Err(NigelError::Other(format!(
                "Invalid match type: {}. Must be one of: {}",
                filter.match_type,
                valid_types.join(", ")
            )));
        }
        if filter.match_type == "regex" {
            regex::Regex::new(pattern)
                .map_err(|e| NigelError::Other(format!("Invalid regex: {e}")))?;
        }
        rows.retain(|r| crate::categorizer::matches(&r.description, pattern, &filter.match_type));
    }
    Ok(rows)
}

pub fn get_transactions_by_ids(
    conn: &Connection,
    ids: &[i64],
) -> Result<Vec<RecategorizeCandidate>> {
    let mut out = Vec::with_capacity(ids.len());
    let mut missing = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.date, t.description, t.amount, t.category_id, c.name \
         FROM transactions t LEFT JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
    )?;
    for &id in ids {
        match stmt.query_row([id], candidate_from_row) {
            Ok(c) => out.push(c),
            Err(rusqlite::Error::QueryReturnedNoRows) => missing.push(id.to_string()),
            Err(e) => return Err(e.into()),
        }
    }
    if !missing.is_empty() {
        return Err(NigelError::Other(format!(
            "No transaction found with ID{} {}. Nothing was changed.",
            if missing.len() == 1 { "" } else { "s" },
            missing.join(", ")
        )));
    }
    Ok(out)
}

/// Batch-move transactions to a category, clearing `is_flagged`/`flag_reason` the
/// same way a review does. Vendor is left untouched. All rows update in one
/// transaction — all or nothing.
pub fn recategorize_transactions(
    conn: &Connection,
    ids: &[i64],
    category_id: i64,
) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0;
    {
        let mut stmt = tx.prepare(
            "UPDATE transactions SET category_id = ?1, is_flagged = 0, flag_reason = NULL WHERE id = ?2",
        )?;
        for &id in ids {
            updated += stmt.execute(rusqlite::params![category_id, id])?;
        }
    }
    tx.commit()?;
    Ok(updated)
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
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, '2025-01-15', 'ADOBE CREATIVE', -50.0, 1, 'No matching rule')",
            rusqlite::params![acct],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn ensure_account(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT OR IGNORE INTO accounts (id, name, account_type) VALUES (1, 'Test', 'checking')",
            [],
        )
        .unwrap();
        1
    }

    fn add_categorized_txn(
        conn: &Connection,
        date: &str,
        desc: &str,
        amount: f64,
        category: &str,
    ) -> i64 {
        let acct = ensure_account(conn);
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = ?1",
                [category],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id, is_flagged) \
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            rusqlite::params![acct, date, desc, amount, cat_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn category_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn test_find_for_recategorize_by_category_and_year() {
        let (_dir, conn) = test_db();
        let a = add_categorized_txn(
            &conn,
            "2025-04-14",
            "MIXAM.COM",
            -667.10,
            "Cost of Goods Sold",
        );
        add_categorized_txn(
            &conn,
            "2024-11-01",
            "MIXAM.COM",
            -300.0,
            "Cost of Goods Sold",
        );
        add_categorized_txn(&conn, "2025-05-01", "DELTA AIR", -400.0, "Travel");

        let filter = RecategorizeFilter {
            from_category_id: Some(category_id(&conn, "Cost of Goods Sold")),
            year: Some(2025),
            ..Default::default()
        };
        let found = find_transactions_for_recategorize(&conn, &filter).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, a);
        assert_eq!(found[0].category.as_deref(), Some("Cost of Goods Sold"));
    }

    #[test]
    fn test_find_for_recategorize_pattern_and_amount() {
        let (_dir, conn) = test_db();
        add_categorized_txn(
            &conn,
            "2025-01-21",
            "ISTOCKPHOTO",
            -45.0,
            "Cost of Goods Sold",
        );
        let m = add_categorized_txn(
            &conn,
            "2025-04-14",
            "MIXAM.COM",
            -667.10,
            "Cost of Goods Sold",
        );

        let filter = RecategorizeFilter {
            pattern: Some("istock".to_string()),
            ..Default::default()
        };
        let found = find_transactions_for_recategorize(&conn, &filter).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].description, "ISTOCKPHOTO");

        let filter = RecategorizeFilter {
            min_amount: Some(100.0),
            ..Default::default()
        };
        let found = find_transactions_for_recategorize(&conn, &filter).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, m);
    }

    #[test]
    fn test_find_for_recategorize_uncategorized() {
        let (_dir, conn) = test_db();
        let flagged = add_flagged_txn(&conn);
        add_categorized_txn(&conn, "2025-05-01", "DELTA AIR", -400.0, "Travel");

        let filter = RecategorizeFilter {
            uncategorized: true,
            ..Default::default()
        };
        let found = find_transactions_for_recategorize(&conn, &filter).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, flagged);
        assert!(found[0].category.is_none());
    }

    #[test]
    fn test_find_for_recategorize_rejects_bad_pattern_config() {
        let (_dir, conn) = test_db();
        add_categorized_txn(
            &conn,
            "2025-01-21",
            "ISTOCKPHOTO",
            -45.0,
            "Cost of Goods Sold",
        );

        let err = find_transactions_for_recategorize(
            &conn,
            &RecategorizeFilter {
                pattern: Some("(".to_string()),
                match_type: "regex".to_string(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid regex"), "got: {err}");

        let err = find_transactions_for_recategorize(
            &conn,
            &RecategorizeFilter {
                pattern: Some("ISTOCK".to_string()),
                match_type: "fuzzy".to_string(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid match type"), "got: {err}");
    }

    #[test]
    fn test_get_transactions_by_ids_unknown_id_errors() {
        let (_dir, conn) = test_db();
        let real = add_categorized_txn(&conn, "2025-05-01", "DELTA AIR", -400.0, "Travel");

        let err = get_transactions_by_ids(&conn, &[real, 99999]).unwrap_err();
        assert!(err.to_string().contains("99999"));

        let found = get_transactions_by_ids(&conn, &[real]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, real);
    }

    #[test]
    fn test_recategorize_transactions_updates_and_clears_flags() {
        let (_dir, conn) = test_db();
        let flagged = add_flagged_txn(&conn);
        let cat = add_categorized_txn(
            &conn,
            "2025-04-14",
            "MIXAM.COM",
            -667.10,
            "Cost of Goods Sold",
        );
        let travel = category_id(&conn, "Travel");

        let updated = recategorize_transactions(&conn, &[flagged, cat], travel).unwrap();
        assert_eq!(updated, 2);

        for id in [flagged, cat] {
            let (cat_id, is_flagged, reason): (i64, i64, Option<String>) = conn
                .query_row(
                    "SELECT category_id, is_flagged, flag_reason FROM transactions WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(cat_id, travel);
            assert_eq!(is_flagged, 0);
            assert!(reason.is_none());
        }
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
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), false, None).unwrap();
        assert!(rule_id.is_none());
        let (is_flagged, vendor): (i32, Option<String>) = conn
            .query_row(
                "SELECT is_flagged, vendor FROM transactions WHERE id = ?1",
                [txn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_flagged, 0);
        assert_eq!(vendor.as_deref(), Some("Adobe"));
    }

    #[test]
    fn test_apply_review_creates_rule() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rule_id =
            apply_review(&conn, txn_id, cat_id, Some("Adobe"), true, Some("ADOBE")).unwrap();
        assert!(rule_id.is_some());
        let count: i64 = conn
            .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let pattern: String = conn
            .query_row("SELECT pattern FROM rules LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(pattern, "ADOBE");
    }

    #[test]
    fn test_undo_review() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rule_id =
            apply_review(&conn, txn_id, cat_id, Some("Adobe"), true, Some("ADOBE")).unwrap();
        assert!(rule_id.is_some());

        undo_review(&conn, txn_id, rule_id).unwrap();

        let (is_flagged, category_id, vendor): (i32, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT is_flagged, category_id, vendor FROM transactions WHERE id = ?1",
                [txn_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(is_flagged, 1);
        assert!(category_id.is_none());
        assert!(vendor.is_none());

        let count: i64 = conn
            .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_undo_review_without_rule() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // Categorize without creating a rule
        let rule_id = apply_review(&conn, txn_id, cat_id, Some("Adobe"), false, None).unwrap();
        assert!(rule_id.is_none());

        undo_review(&conn, txn_id, None).unwrap();

        let (is_flagged, category_id): (i32, Option<i64>) = conn
            .query_row(
                "SELECT is_flagged, category_id FROM transactions WHERE id = ?1",
                [txn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_flagged, 1);
        assert!(category_id.is_none());
    }

    #[test]
    fn test_update_transaction_category() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        update_transaction_category(&conn, txn_id, cat_id).unwrap();

        let stored: Option<i64> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id = ?1",
                [txn_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, Some(cat_id));
    }

    #[test]
    fn test_update_transaction_vendor() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn);

        update_transaction_vendor(&conn, txn_id, Some("Adobe")).unwrap();

        let vendor: Option<String> = conn
            .query_row(
                "SELECT vendor FROM transactions WHERE id = ?1",
                [txn_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(vendor.as_deref(), Some("Adobe"));

        // Clear vendor
        update_transaction_vendor(&conn, txn_id, None).unwrap();
        let vendor2: Option<String> = conn
            .query_row(
                "SELECT vendor FROM transactions WHERE id = ?1",
                [txn_id],
                |r| r.get(0),
            )
            .unwrap();
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

    #[test]
    fn test_set_transaction_flag_is_idempotent() {
        let (_dir, conn) = test_db();
        let txn_id = add_flagged_txn(&conn); // starts flagged

        // Setting the state it already has is a no-op, not a toggle.
        set_transaction_flag(&conn, txn_id, true).unwrap();
        assert!(transaction_flag(&conn, txn_id).unwrap());

        set_transaction_flag(&conn, txn_id, false).unwrap();
        set_transaction_flag(&conn, txn_id, false).unwrap();
        assert!(!transaction_flag(&conn, txn_id).unwrap());
    }

    #[test]
    fn test_flag_helpers_report_a_missing_transaction() {
        let (_dir, conn) = test_db();
        assert!(matches!(
            set_transaction_flag(&conn, 4242, true).unwrap_err(),
            NigelError::NotFound(_)
        ));
        assert!(matches!(
            transaction_flag(&conn, 4242).unwrap_err(),
            NigelError::NotFound(_)
        ));
        assert!(matches!(
            toggle_transaction_flag(&conn, 4242).unwrap_err(),
            NigelError::NotFound(_)
        ));
    }
}

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileResult {
    pub is_reconciled: bool,
    pub statement_balance: f64,
    pub calculated_balance: f64,
    pub discrepancy: f64,
}

/// A stored reconciliation, joined to the account it belongs to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationRecord {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub month: String,
    /// Nullable in the schema: a record can predate either balance.
    pub statement_balance: Option<f64>,
    pub calculated_balance: Option<f64>,
    pub is_reconciled: bool,
    pub reconciled_at: Option<String>,
    pub notes: Option<String>,
}

/// Reconciliation history, newest month first, optionally for one account.
pub fn list_reconciliations(
    conn: &Connection,
    account: Option<&str>,
) -> Result<Vec<ReconciliationRecord>> {
    let mut sql = String::from(
        "SELECT r.id, r.account_id, a.name, r.month, r.statement_balance, r.calculated_balance, \
                r.is_reconciled, r.reconciled_at, r.notes \
         FROM reconciliations r JOIN accounts a ON r.account_id = a.id",
    );
    let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
    if let Some(name) = account.as_ref() {
        sql.push_str(" WHERE a.name = ?1");
        params.push(name);
    }
    sql.push_str(" ORDER BY r.month DESC, r.id DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok(ReconciliationRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                month: row.get(3)?,
                statement_balance: row.get(4)?,
                calculated_balance: row.get(5)?,
                is_reconciled: row.get(6)?,
                reconciled_at: row.get(7)?,
                notes: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn reconcile(
    conn: &Connection,
    account_name: &str,
    month: &str,
    statement_balance: f64,
) -> Result<ReconcileResult> {
    let account_id: i64 = conn
        .query_row(
            "SELECT id FROM accounts WHERE name = ?1",
            [account_name],
            |row| row.get(0),
        )
        .map_err(|_| NigelError::UnknownAccount(account_name.to_string()))?;

    // Check if there are any transactions for this account in the given month
    let tx_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND date >= ?2 || '-01' AND date <= ?2 || '-31'",
        rusqlite::params![account_id, month],
        |row| row.get(0),
    )?;
    if tx_count == 0 {
        return Err(NigelError::NoTransactions {
            account: account_name.to_string(),
            month: month.to_string(),
        });
    }

    let calculated: f64 = conn.query_row(
        "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE account_id = ?1 AND date <= ?2 || '-31'",
        rusqlite::params![account_id, month],
        |row| row.get(0),
    )?;

    let discrepancy = (calculated - statement_balance).abs();
    let is_reconciled = discrepancy < 0.01;

    conn.execute(
        "INSERT INTO reconciliations (account_id, month, statement_balance, calculated_balance, is_reconciled, reconciled_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, CASE WHEN ?5 = 1 THEN datetime('now') ELSE NULL END)",
        rusqlite::params![account_id, month, statement_balance, calculated, is_reconciled as i32],
    )?;

    Ok(ReconcileResult {
        is_reconciled,
        statement_balance,
        calculated_balance: calculated,
        discrepancy: (discrepancy * 100.0).round() / 100.0,
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

    fn setup_account_with_txns(conn: &Connection, total: f64) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test Checking', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) VALUES (?1, '2025-01-15', 'Deposit', ?2)",
            rusqlite::params![acct, total],
        ).unwrap();
    }

    #[test]
    fn test_matching_balance() {
        let (_dir, conn) = test_db();
        setup_account_with_txns(&conn, 1000.0);
        let result = reconcile(&conn, "Test Checking", "2025-01", 1000.0).unwrap();
        assert!(result.is_reconciled);
        assert_eq!(result.discrepancy, 0.0);
    }

    #[test]
    fn test_with_discrepancy() {
        let (_dir, conn) = test_db();
        setup_account_with_txns(&conn, 1000.0);
        let result = reconcile(&conn, "Test Checking", "2025-01", 1100.0).unwrap();
        assert!(!result.is_reconciled);
        assert_eq!(result.discrepancy, 100.0);
    }

    #[test]
    fn test_stores_record() {
        let (_dir, conn) = test_db();
        setup_account_with_txns(&conn, 500.0);
        reconcile(&conn, "Test Checking", "2025-01", 500.0).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM reconciliations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_reconciliations_is_newest_first_and_filters_by_account() {
        let (_dir, conn) = test_db();
        setup_account_with_txns(&conn, 500.0);
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Other', 'credit_card')",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) VALUES (?1, '2025-02-05', 'Charge', -20.0)",
            rusqlite::params![other],
        )
        .unwrap();

        reconcile(&conn, "Test Checking", "2025-01", 500.0).unwrap();
        reconcile(&conn, "Other", "2025-02", -20.0).unwrap();

        let all = list_reconciliations(&conn, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].month, "2025-02");
        assert_eq!(all[0].account_name, "Other");
        assert!(all[0].is_reconciled);
        assert_eq!(all[0].statement_balance, Some(-20.0));
        assert!(all[0].reconciled_at.is_some());
        assert!(all[0].notes.is_none());

        let filtered = list_reconciliations(&conn, Some("Test Checking")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].month, "2025-01");

        assert!(list_reconciliations(&conn, Some("Nope"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_reconcile_reports_an_empty_month() {
        let (_dir, conn) = test_db();
        setup_account_with_txns(&conn, 500.0);
        let err = reconcile(&conn, "Test Checking", "2025-07", 500.0).unwrap_err();
        assert!(matches!(err, NigelError::NoTransactions { .. }));
    }
}

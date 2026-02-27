use rusqlite::Connection;

use crate::error::{NigelError, Result};

pub struct ReconcileResult {
    pub is_reconciled: bool,
    pub statement_balance: f64,
    pub calculated_balance: f64,
    pub discrepancy: f64,
}

pub fn reconcile(
    conn: &Connection,
    account_name: &str,
    month: &str,
    statement_balance: f64,
) -> Result<ReconcileResult> {
    let account_id: i64 = conn
        .query_row("SELECT id FROM accounts WHERE name = ?1", [account_name], |row| {
            row.get(0)
        })
        .map_err(|_| NigelError::UnknownAccount(account_name.to_string()))?;

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
            "INSERT INTO accounts (name, account_type) VALUES ('Test Checking', 'checking')", [],
        ).unwrap();
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
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM reconciliations", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
}

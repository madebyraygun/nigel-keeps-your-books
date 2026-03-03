use std::io::{self, BufRead, Write};

use rusqlite::Connection;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::settings::get_data_dir;

/// Information about the most recent import, used for display and deletion.
pub struct LastImport {
    pub import_id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
}

/// Query the most recent import and its associated transaction count.
/// Returns None if there are no imports in the database.
pub fn get_last_import(conn: &Connection) -> Result<Option<LastImport>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.filename, COALESCE(a.name, '(unknown)'), i.import_date
         FROM imports i
         LEFT JOIN accounts a ON a.id = i.account_id
         ORDER BY i.id DESC LIMIT 1",
    )?;

    let result = stmt
        .query_row([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .optional()?;

    let Some((import_id, filename, account_name, import_date)) = result else {
        return Ok(None);
    };

    let transaction_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE import_id = ?1",
        [import_id],
        |row| row.get(0),
    )?;

    Ok(Some(LastImport {
        import_id,
        filename,
        account_name,
        import_date,
        transaction_count,
    }))
}

/// Delete all transactions and the import record for the given import.
/// Returns the number of transactions deleted.
pub fn delete_import(conn: &Connection, import_id: i64) -> Result<usize> {
    let deleted = conn.execute("DELETE FROM transactions WHERE import_id = ?1", [import_id])?;
    conn.execute("DELETE FROM imports WHERE id = ?1", [import_id])?;
    Ok(deleted)
}

pub fn run() -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    let Some(last) = get_last_import(&conn)? else {
        println!("No imports to undo.");
        return Ok(());
    };

    println!("Last import:");
    println!("  File:         {}", last.filename);
    println!("  Account:      {}", last.account_name);
    println!("  Imported:     {}", last.import_date);
    println!("  Transactions: {}", last.transaction_count);
    println!();

    print!("Undo this import? [y/N] ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let line = stdin
        .lock()
        .lines()
        .next()
        .unwrap_or(Ok(String::new()))
        .map_err(|e| NigelError::Other(e.to_string()))?;

    if !line.trim().eq_ignore_ascii_case("y") {
        println!("Cancelled.");
        return Ok(());
    }

    let deleted = delete_import(&conn, last.import_id)?;
    println!(
        "Rolled back import of \"{}\" ({} transactions removed)",
        last.filename, deleted
    );

    Ok(())
}

/// Extension trait for optional query results (rusqlite doesn't expose this publicly).
trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
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

    fn add_account(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test Checking', 'checking')",
            [],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn add_import(conn: &Connection, account_id: i64, filename: &str) -> i64 {
        conn.execute(
            "INSERT INTO imports (filename, account_id, record_count, checksum) VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![filename, account_id, format!("checksum_{filename}")],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn add_transaction(conn: &Connection, account_id: i64, import_id: i64, desc: &str) {
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, import_id, is_flagged) VALUES (?1, '2025-01-15', ?2, -100.0, ?3, 1)",
            rusqlite::params![account_id, desc, import_id],
        )
        .unwrap();
    }

    #[test]
    fn test_get_last_import_empty_db() {
        let (_dir, conn) = test_db();
        let result = get_last_import(&conn).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_last_import_returns_most_recent() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        add_import(&conn, acct, "first.csv");
        add_import(&conn, acct, "second.csv");

        let last = get_last_import(&conn).unwrap().unwrap();
        assert_eq!(last.filename, "second.csv");
        assert_eq!(last.account_name, "Test Checking");
    }

    #[test]
    fn test_get_last_import_counts_transactions() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        let import_id = add_import(&conn, acct, "stmt.csv");
        add_transaction(&conn, acct, import_id, "TXN A");
        add_transaction(&conn, acct, import_id, "TXN B");
        add_transaction(&conn, acct, import_id, "TXN C");

        let last = get_last_import(&conn).unwrap().unwrap();
        assert_eq!(last.transaction_count, 3);
    }

    #[test]
    fn test_delete_import_removes_transactions_and_record() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        let import_id = add_import(&conn, acct, "stmt.csv");
        add_transaction(&conn, acct, import_id, "TXN A");
        add_transaction(&conn, acct, import_id, "TXN B");

        let deleted = delete_import(&conn, import_id).unwrap();
        assert_eq!(deleted, 2);

        let txn_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(txn_count, 0);

        let import_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(import_count, 0);
    }

    #[test]
    fn test_delete_import_only_affects_target_import() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        let import1 = add_import(&conn, acct, "first.csv");
        let import2 = add_import(&conn, acct, "second.csv");
        add_transaction(&conn, acct, import1, "TXN from first");
        add_transaction(&conn, acct, import2, "TXN from second");

        delete_import(&conn, import2).unwrap();

        let txn_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(txn_count, 1, "first import's transactions should remain");

        let import_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(import_count, 1, "first import record should remain");
    }

    #[test]
    fn test_delete_import_with_zero_transactions() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        let import_id = add_import(&conn, acct, "empty.csv");

        let deleted = delete_import(&conn, import_id).unwrap();
        assert_eq!(deleted, 0);

        let import_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(import_count, 0);
    }
}

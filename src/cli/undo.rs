use std::io::{self, BufRead, Write};

use rusqlite::Connection;
use serde::Serialize;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::settings::get_data_dir;

/// Information about the most recent import, used for display and deletion.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastImport {
    pub import_id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
}

/// One row of import history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportListItem {
    pub id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
}

/// Every import ever recorded, newest first, each with the number of
/// transactions still attached to it.
pub fn list_imports(conn: &Connection) -> Result<Vec<ImportListItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.filename, COALESCE(a.name, '(unknown)'), i.import_date, COUNT(t.id)
         FROM imports i
         LEFT JOIN accounts a ON a.id = i.account_id
         LEFT JOIN transactions t ON t.import_id = i.id
         GROUP BY i.id
         ORDER BY i.id DESC",
    )?;

    let imports = stmt
        .query_map([], |row| {
            Ok(ImportListItem {
                id: row.get(0)?,
                filename: row.get(1)?,
                account_name: row.get(2)?,
                import_date: row.get(3)?,
                transaction_count: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(imports)
}

/// Query the most recent import and its associated transaction count.
/// Returns None if there are no imports in the database.
pub fn get_last_import(conn: &Connection) -> Result<Option<LastImport>> {
    Ok(list_imports(conn)?
        .into_iter()
        .next()
        .map(|item| LastImport {
            import_id: item.id,
            filename: item.filename,
            account_name: item.account_name,
            import_date: item.import_date,
            transaction_count: item.transaction_count,
        }))
}

/// Delete all transactions and the import record for the given import.
/// Returns the number of transactions deleted.
pub fn delete_import(conn: &Connection, import_id: i64) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let deleted = tx.execute("DELETE FROM transactions WHERE import_id = ?1", [import_id])?;
    tx.execute("DELETE FROM imports WHERE id = ?1", [import_id])?;
    tx.commit()?;
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
    fn list_imports_returns_newest_first_with_counts() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        let first = add_import(&conn, acct, "first.csv");
        let second = add_import(&conn, acct, "second.csv");
        add_transaction(&conn, acct, first, "TXN A");
        add_transaction(&conn, acct, first, "TXN B");

        let imports = list_imports(&conn).unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].id, second);
        assert_eq!(imports[0].filename, "second.csv");
        // An import whose rows were all removed still lists, at zero.
        assert_eq!(imports[0].transaction_count, 0);
        assert_eq!(imports[1].id, first);
        assert_eq!(imports[1].transaction_count, 2);
        assert_eq!(imports[1].account_name, "Test Checking");
    }

    #[test]
    fn list_imports_is_empty_for_a_fresh_database() {
        let (_dir, conn) = test_db();
        assert!(list_imports(&conn).unwrap().is_empty());
    }

    #[test]
    fn list_imports_names_an_orphaned_account() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO imports (filename, account_id, record_count, checksum) \
             VALUES ('orphan.csv', NULL, 0, 'sum')",
            [],
        )
        .unwrap();

        let imports = list_imports(&conn).unwrap();
        assert_eq!(imports[0].account_name, "(unknown)");
    }

    #[test]
    fn import_list_item_serializes_camel_case() {
        let (_dir, conn) = test_db();
        let acct = add_account(&conn);
        add_import(&conn, acct, "stmt.csv");

        let json = serde_json::to_value(&list_imports(&conn).unwrap()[0]).unwrap();
        for key in ["accountName", "importDate", "transactionCount"] {
            assert!(json.get(key).is_some(), "missing {key} in {json}");
        }
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

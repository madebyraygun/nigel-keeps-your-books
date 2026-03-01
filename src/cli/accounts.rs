use comfy_table::{Table, Cell};
use rusqlite::Connection;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::models::Account;
use crate::settings::get_data_dir;

pub fn add(name: &str, account_type: &str, institution: Option<&str>, last_four: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, account_type, institution, last_four],
    )?;
    println!("Added account: {name}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare("SELECT id, name, account_type, institution, last_four FROM accounts")?;
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = stmt
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

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Type", "Institution", "Last Four"]);
    for (id, name, acct_type, inst, last) in rows {
        table.add_row(vec![
            Cell::new(id),
            Cell::new(name),
            Cell::new(acct_type),
            Cell::new(inst.unwrap_or_default()),
            Cell::new(last.unwrap_or_default()),
        ]);
    }
    println!("Accounts\n{table}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Data-layer functions for TUI account management
// ---------------------------------------------------------------------------

pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, institution, last_four FROM accounts ORDER BY name",
    )?;
    let accounts = stmt
        .query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: row.get(2)?,
                institution: row.get(3)?,
                last_four: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(accounts)
}

pub fn add_account(
    conn: &Connection,
    name: &str,
    account_type: &str,
    institution: Option<&str>,
    last_four: Option<&str>,
) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::Other(format!(
            "Account name already exists: {name}"
        )));
    }
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, account_type, institution, last_four],
    )?;
    Ok(())
}

pub fn rename_account(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    if new_name.trim().is_empty() {
        return Err(NigelError::Other("Name is required".into()));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1 AND id != ?2)",
        rusqlite::params![new_name, id],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::Other(format!(
            "Account name already exists: {new_name}"
        )));
    }
    let updated = conn.execute(
        "UPDATE accounts SET name = ?1 WHERE id = ?2",
        rusqlite::params![new_name, id],
    )?;
    if updated == 0 {
        return Err(NigelError::Other(format!("Account not found: id {id}")));
    }
    Ok(())
}

pub fn transaction_count(conn: &Connection, account_id: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
        [account_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn delete_account(conn: &Connection, id: i64) -> Result<()> {
    let count = transaction_count(conn, id)?;
    if count > 0 {
        let noun = if count == 1 { "transaction" } else { "transactions" };
        return Err(NigelError::Other(format!(
            "Cannot delete: account has {count} {noun}"
        )));
    }
    // Clean up reconciliations; null out imports to preserve checksums for duplicate detection
    conn.execute(
        "DELETE FROM reconciliations WHERE account_id = ?1",
        [id],
    )?;
    conn.execute("UPDATE imports SET account_id = NULL WHERE account_id = ?1", [id])?;
    let deleted = conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    if deleted == 0 {
        return Err(NigelError::Other(format!("Account not found: id {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_list_accounts_empty() {
        let (_dir, conn) = test_conn();
        let accounts = list_accounts(&conn).unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn test_add_and_list_accounts() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "BofA Checking", "checking", Some("Bank of America"), Some("1234")).unwrap();
        add_account(&conn, "Chase Credit", "credit_card", None, None).unwrap();

        let accounts = list_accounts(&conn).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].name, "BofA Checking");
        assert_eq!(accounts[0].account_type, "checking");
        assert_eq!(accounts[0].institution.as_deref(), Some("Bank of America"));
        assert_eq!(accounts[0].last_four.as_deref(), Some("1234"));
        assert_eq!(accounts[1].name, "Chase Credit");
    }

    #[test]
    fn test_add_duplicate_name_rejected() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "BofA Checking", "checking", None, None).unwrap();
        let err = add_account(&conn, "BofA Checking", "credit_card", None, None).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_rename_account() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Old Name", "checking", None, None).unwrap();
        let accounts = list_accounts(&conn).unwrap();
        let id = accounts[0].id;

        rename_account(&conn, id, "New Name").unwrap();

        let accounts = list_accounts(&conn).unwrap();
        assert_eq!(accounts[0].name, "New Name");
    }

    #[test]
    fn test_rename_to_empty_rejected() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Test", "checking", None, None).unwrap();
        let id = list_accounts(&conn).unwrap()[0].id;

        let err = rename_account(&conn, id, "  ").unwrap_err();
        assert!(err.to_string().contains("Name is required"));
    }

    #[test]
    fn test_rename_to_duplicate_rejected() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Account A", "checking", None, None).unwrap();
        add_account(&conn, "Account B", "checking", None, None).unwrap();
        let id_b = list_accounts(&conn).unwrap().iter().find(|a| a.name == "Account B").unwrap().id;

        let err = rename_account(&conn, id_b, "Account A").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_delete_account_no_transactions() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Test", "checking", None, None).unwrap();
        let id = list_accounts(&conn).unwrap()[0].id;

        delete_account(&conn, id).unwrap();
        assert!(list_accounts(&conn).unwrap().is_empty());
    }

    #[test]
    fn test_delete_account_with_transactions_blocked() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Test", "checking", None, None).unwrap();
        let id = list_accounts(&conn).unwrap()[0].id;

        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) VALUES (?1, '2025-01-01', 'Test', 100.0)",
            [id],
        ).unwrap();

        let err = delete_account(&conn, id).unwrap_err();
        assert!(err.to_string().contains("Cannot delete"));
        assert!(err.to_string().contains("1 transaction"));
    }

    #[test]
    fn test_transaction_count() {
        let (_dir, conn) = test_conn();
        add_account(&conn, "Test", "checking", None, None).unwrap();
        let id = list_accounts(&conn).unwrap()[0].id;

        assert_eq!(transaction_count(&conn, id).unwrap(), 0);

        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount) VALUES (?1, '2025-01-01', 'Test', 50.0)",
            [id],
        ).unwrap();

        assert_eq!(transaction_count(&conn, id).unwrap(), 1);
    }

    #[test]
    fn test_delete_nonexistent_account() {
        let (_dir, conn) = test_conn();
        let err = delete_account(&conn, 9999).unwrap_err();
        assert!(err.to_string().contains("Account not found"));
    }
}

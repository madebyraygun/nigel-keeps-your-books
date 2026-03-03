use std::path::PathBuf;

use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::settings::{get_data_dir, restrict_file_permissions, shellexpand_path};

pub fn run(path: &str) -> Result<()> {
    let backup_path = PathBuf::from(shellexpand_path(path));

    // 1. Validate the backup file exists
    if !backup_path.exists() {
        return Err(crate::error::NigelError::Other(format!(
            "Backup file not found: {}",
            backup_path.display()
        )));
    }

    if !backup_path.is_file() {
        return Err(crate::error::NigelError::Other(format!(
            "Not a file: {}",
            backup_path.display()
        )));
    }

    // 2. Validate the backup is a valid SQLite database
    let test_conn = get_connection(&backup_path).map_err(|e| {
        crate::error::NigelError::Other(format!(
            "Cannot open backup file (is the password correct?): {e}"
        ))
    })?;
    let has_tables: bool = test_conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('accounts', 'categories', 'transactions')",
            [],
            |row| {
                let count: i64 = row.get(0)?;
                Ok(count == 3)
            },
        )
        .map_err(|e| {
            crate::error::NigelError::Other(format!(
                "Cannot read backup file (is the password correct?): {e}"
            ))
        })?;
    drop(test_conn);

    if !has_tables {
        return Err(crate::error::NigelError::Other(
            "Backup file is not a valid Nigel database (missing expected tables).".into(),
        ));
    }

    let data_dir = get_data_dir();
    let db_path = data_dir.join("nigel.db");

    // 3. Confirm with user before replacing the database
    if db_path.exists() {
        print!("This will replace the current database. Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Restore cancelled.");
            return Ok(());
        }
    }

    // 4. Create a safety backup of the current database
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    crate::settings::restrict_dir_permissions(&backups_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let safety_path = backups_dir.join(format!("nigel-pre-restore-{stamp}.db"));

    if db_path.exists() {
        let conn = get_connection(&db_path)?;
        crate::cli::backup::snapshot(&conn, &safety_path)?;
        drop(conn);
        println!("Safety backup saved to {}", safety_path.display());
    }

    // 5. Restore using the SQLite backup API (handles WAL and encryption correctly)
    let src_conn = get_connection(&backup_path)?;
    crate::cli::backup::snapshot(&src_conn, &db_path)?;
    drop(src_conn);
    restrict_file_permissions(&db_path)?;

    // 6. Validate the restored database
    let conn = get_connection(&db_path)?;
    init_db(&conn)?;
    let tx_count: i64 = conn.query_row(
        "SELECT count(*) FROM transactions",
        [],
        |row| row.get(0),
    )?;
    let acct_count: i64 =
        conn.query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))?;
    drop(conn);

    let size = std::fs::metadata(&db_path)?.len();
    println!("Database restored from {}", backup_path.display());
    println!(
        "Size: {} — {} accounts, {} transactions",
        format_bytes(size),
        acct_count,
        tx_count
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, set_db_password};

    #[test]
    fn test_restore_replaces_database() {
        set_db_password(None);

        // Create a "current" database
        let data_dir = tempfile::tempdir().unwrap();
        let db_path = data_dir.path().join("nigel.db");
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Original', 'checking')",
            [],
        )
        .unwrap();
        drop(conn);

        // Create a "backup" database with different data
        let backup_dir = tempfile::tempdir().unwrap();
        let backup_path = backup_dir.path().join("backup.db");
        let conn = get_connection(&backup_path).unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Restored', 'checking')",
            [],
        )
        .unwrap();
        drop(conn);

        // Use the SQLite backup API (same as run() does)
        let src_conn = get_connection(&backup_path).unwrap();
        crate::cli::backup::snapshot(&src_conn, &db_path).unwrap();
        drop(src_conn);

        // Verify the restored DB has the backup's data
        let conn = get_connection(&db_path).unwrap();
        let name: String = conn
            .query_row("SELECT name FROM accounts LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "Restored");
    }

    #[test]
    fn test_restore_rejects_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent-backup.db");
        let result = run(missing.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_restore_rejects_non_nigel_db() {
        set_db_password(None);
        let dir = tempfile::tempdir().unwrap();
        let fake_path = dir.path().join("fake.db");

        // Create a valid SQLite DB but without Nigel tables
        let conn = rusqlite::Connection::open(&fake_path).unwrap();
        conn.execute_batch("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
            .unwrap();
        drop(conn);

        let result = run(fake_path.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not a valid Nigel database"));
    }

    #[test]
    fn test_restore_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = run(dir.path().to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Not a file"));
    }
}

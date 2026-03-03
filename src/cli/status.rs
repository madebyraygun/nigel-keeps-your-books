use crate::db::{get_connection, get_metadata, is_encrypted};
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::migrations::{get_schema_version, LATEST_VERSION};
use crate::settings::load_settings;

pub fn run() -> Result<()> {
    let settings = load_settings();
    let data_dir = std::path::PathBuf::from(&settings.data_dir);
    let db_path = data_dir.join("nigel.db");

    let user_name = if settings.user_name.is_empty() {
        "(not set)"
    } else {
        &settings.user_name
    };

    if !db_path.exists() {
        println!("User:       {user_name}");
        println!("Data dir:   {}", data_dir.display());
        println!("Database:   {}", db_path.display());
        println!();
        println!("Database not found. Run `nigel init` to set up.");
        return Ok(());
    }

    // Collect all data before printing to avoid partial output on error
    let size = std::fs::metadata(&db_path)?.len();
    let encrypted = is_encrypted(&db_path)?;
    let conn = get_connection(&db_path)?;

    let company = get_metadata(&conn, "company_name");
    let schema_v = get_schema_version(&conn)?;
    let accounts: i64 = conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))?;
    let transactions: i64 =
        conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))?;
    let flagged: i64 = conn.query_row(
        "SELECT count(*) FROM transactions WHERE is_flagged = 1",
        [],
        |r| r.get(0),
    )?;
    let rules: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0))?;

    // All data collected — print output
    println!("User:       {user_name}");
    println!("Data dir:   {}", data_dir.display());
    println!("Database:   {}", db_path.display());
    println!("DB size:    {}", format_bytes(size));
    println!("Encrypted:  {}", if encrypted { "yes" } else { "no" });
    println!("Company:    {}", company.as_deref().unwrap_or("(not set)"));
    println!("Schema:     v{schema_v} (latest: v{LATEST_VERSION})");
    println!();
    println!("Accounts:      {accounts}");
    println!("Transactions:  {transactions}");
    println!("Flagged:       {flagged}");
    println!("Rules:         {rules}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::{get_connection, init_db, is_encrypted, open_connection, set_db_password};

    #[test]
    fn test_is_encrypted_shown_false_for_plain_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_connection(&db_path, None).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_shown_true_for_encrypted_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        assert!(is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_encrypted_db_accessible_with_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Set global password and verify get_connection works
        set_db_password(Some("secret".to_string()));
        let conn = get_connection(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Clean up global state
        set_db_password(None);
    }

    #[test]
    fn test_encrypted_db_fails_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Ensure no global password is set
        set_db_password(None);
        // get_connection fails with "file is not a database" when password is missing
        let result = get_connection(&db_path);
        assert!(result.is_err());
    }
}

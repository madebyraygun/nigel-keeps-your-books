use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::Result;
use crate::migrations;

static DB_PASSWORD: Mutex<Option<String>> = Mutex::new(None);

/// Set the global database password. Call before `get_connection()`.
pub fn set_db_password(password: Option<String>) {
    *DB_PASSWORD.lock().unwrap() = password;
}

/// Read the current global database password.
pub fn get_db_password() -> Option<String> {
    DB_PASSWORD.lock().unwrap().clone()
}

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    institution TEXT,
    last_four TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id INTEGER,
    category_type TEXT NOT NULL,
    tax_line TEXT,
    form_line TEXT,
    description TEXT,
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (parent_id) REFERENCES categories(id)
);

CREATE TABLE IF NOT EXISTS imports (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    account_id INTEGER,
    import_date TEXT DEFAULT (datetime('now')),
    record_count INTEGER,
    date_range_start TEXT,
    date_range_end TEXT,
    checksum TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    category_id INTEGER,
    vendor TEXT,
    notes TEXT,
    is_flagged INTEGER DEFAULT 0,
    flag_reason TEXT,
    import_id INTEGER,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (import_id) REFERENCES imports(id)
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL,
    match_type TEXT DEFAULT 'contains',
    vendor TEXT,
    category_id INTEGER NOT NULL,
    priority INTEGER DEFAULT 0,
    hit_count INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE TABLE IF NOT EXISTS reconciliations (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    month TEXT NOT NULL,
    statement_balance REAL,
    calculated_balance REAL,
    is_reconciled INTEGER DEFAULT 0,
    reconciled_at TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

// (name, parent_id, category_type, tax_line, form_line, description)
const DEFAULT_CATEGORIES: &[(&str, Option<i64>, &str, Option<&str>, Option<&str>, &str)] = &[
    // Income
    ("Client Services", None, "income", Some("Gross receipts"), None, "Project fees, retainer payments"),
    ("Hosting & Maintenance", None, "income", Some("Gross receipts"), None, "Recurring client hosting/maintenance fees"),
    ("Reimbursements", None, "income", Some("Gross receipts"), None, "Client reimbursements for expenses"),
    ("Interest Income", None, "income", Some("Other income"), Some("K-4"), "Bank interest"),
    ("Other Income", None, "income", Some("Other income"), None, "Anything else"),
    // Expenses
    ("Advertising & Marketing", None, "expense", Some("Line 8"), Some("1120S-16"), "Ads, sponsorships, marketing tools"),
    ("Car & Truck", None, "expense", Some("Line 9"), Some("1120S-19"), "Mileage, fuel, parking"),
    ("Commissions & Fees", None, "expense", Some("Line 10"), Some("1120S-19"), "Subcontractor commissions, platform fees"),
    ("Contract Labor", None, "expense", Some("Line 11"), Some("1120S-19"), "Freelancers, subcontractors (1099 work)"),
    ("Insurance", None, "expense", Some("Line 15"), Some("1120S-19"), "Business insurance, E&O"),
    ("Legal & Professional", None, "expense", Some("Line 17"), Some("1120S-19"), "Accountant, lawyer, professional services"),
    ("Office Expense", None, "expense", Some("Line 18"), Some("1120S-19"), "Office supplies, minor equipment"),
    ("Rent / Lease", None, "expense", Some("Line 20b"), Some("1120S-11"), "Office rent, coworking"),
    ("Software & Subscriptions", None, "expense", Some("Line 18/27a"), Some("1120S-19"), "SaaS tools, domain renewals, cloud services"),
    ("Hosting & Infrastructure", None, "expense", Some("Line 18/27a"), Some("1120S-19"), "AWS, server costs, CDN"),
    ("Taxes & Licenses", None, "expense", Some("Line 23"), Some("1120S-12"), "Business licenses, state fees"),
    ("Travel", None, "expense", Some("Line 24a"), Some("1120S-19"), "Flights, hotels, conference travel"),
    ("Meals", None, "expense", Some("Line 24b"), Some("1120S-19"), "Business meals (50% deductible)"),
    ("Utilities", None, "expense", Some("Line 25"), Some("1120S-19"), "Internet, phone (business portion)"),
    ("Payroll — Wages", None, "expense", Some("Line 26"), Some("1120S-8"), "Employee salaries (from Gusto)"),
    ("Payroll — Taxes", None, "expense", Some("Line 23"), Some("1120S-12"), "Employer payroll taxes (from Gusto)"),
    ("Payroll — Benefits", None, "expense", Some("Line 14"), Some("1120S-18"), "Health insurance, retirement (from Gusto)"),
    ("Bank & Merchant Fees", None, "expense", Some("Line 27a"), Some("1120S-19"), "Stripe fees, bank charges, wire fees"),
    ("Education & Training", None, "expense", Some("Line 27a"), Some("1120S-19"), "Courses, books, conferences"),
    ("Equipment", None, "expense", Some("Line 13"), Some("1120S-19"), "Hardware, major purchases"),
    ("Home Office", None, "expense", Some("Line 30"), Some("1120S-19"), "Simplified method or actual expenses"),
    ("Owner Draw / Distribution", None, "expense", Some("Not deductible"), Some("K-16d"), "Owner payments, distributions"),
    ("Transfer", None, "expense", Some("Not deductible"), None, "Transfers between own accounts"),
    ("Uncategorized", None, "expense", Some("\u{2014}"), None, "Needs review"),
];

pub fn get_connection(db_path: &Path) -> Result<Connection> {
    let password = get_db_password();
    open_connection(db_path, password.as_deref())
}

/// Open a connection with an explicit password (bypasses global state).
/// Used by backup, password management, and tests.
pub fn open_connection(db_path: &Path, password: Option<&str>) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    if let Some(pw) = password {
        conn.pragma_update(None, "key", pw)?;
    }
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Check whether a database file is encrypted (requires a password to open).
/// Returns false for nonexistent files (they will be created fresh).
/// Detection uses the SQLite magic header: plaintext databases always start
/// with "SQLite format 3\0". Anything else is encrypted or corrupt.
pub fn is_encrypted(db_path: &Path) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }
    let mut buf = [0u8; 16];
    let mut file = std::fs::File::open(db_path)?;
    use std::io::Read;
    let n = file.read(&mut buf)?;
    if n < 16 {
        return Ok(false); // too small to be a valid DB
    }
    Ok(&buf != b"SQLite format 3\0")
}

/// If the database is encrypted, prompt the user for a password (up to 3 attempts).
/// Sets the global password on success. Returns an error after 3 failures.
pub fn prompt_password_if_needed(db_path: &Path) -> Result<()> {
    if !is_encrypted(db_path)? {
        return Ok(());
    }
    for attempt in 1..=3 {
        let pw = rpassword::prompt_password("Database password: ")
            .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;
        set_db_password(Some(pw));
        // get_connection runs PRAGMAs that read the DB header, so it will fail
        // on a wrong password. Match on the result instead of using ? to avoid
        // short-circuiting the retry loop.
        match get_connection(db_path) {
            Ok(_) => return Ok(()),
            Err(_) => {
                set_db_password(None);
                if attempt < 3 {
                    eprintln!("Wrong password. Try again ({attempt}/3).");
                }
            }
        }
    }
    Err(crate::error::NigelError::Other(
        "Failed to unlock database after 3 attempts.".into(),
    ))
}

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;

    let count: i64 = conn.query_row("SELECT count(*) FROM categories", [], |row| row.get(0))?;
    if count == 0 {
        for cat in DEFAULT_CATEGORIES {
            conn.execute(
                "INSERT INTO categories (name, parent_id, category_type, tax_line, form_line, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![cat.0, cat.1, cat.2, cat.3, cat.4, cat.5],
            )?;
        }
    }

    migrations::run_migrations(conn)?;
    Ok(())
}

pub fn get_metadata(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM metadata WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

// Tests mutate the global DB_PASSWORD mutex and must run with --test-threads=1
// to avoid interference between tests. See also: cli::password::tests, cli::backup::tests.
#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_init_db_creates_tables() {
        let (_dir, conn) = test_db();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        for expected in &["accounts", "categories", "transactions", "rules", "imports", "reconciliations", "metadata"] {
            assert!(tables.contains(&expected.to_string()), "missing table: {expected}");
        }
    }

    #[test]
    fn test_init_db_is_idempotent() {
        let (_dir, conn) = test_db();
        init_db(&conn).unwrap();
    }

    #[test]
    fn test_init_db_seeds_categories() {
        let (_dir, conn) = test_db();
        let count: i64 = conn.query_row("SELECT count(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert!(count >= 29, "expected at least 29 categories, got {count}");
    }

    #[test]
    fn test_income_and_expense_categories() {
        let (_dir, conn) = test_db();
        let income: i64 = conn.query_row(
            "SELECT count(*) FROM categories WHERE category_type = 'income'", [], |r| r.get(0),
        ).unwrap();
        let expense: i64 = conn.query_row(
            "SELECT count(*) FROM categories WHERE category_type = 'expense'", [], |r| r.get(0),
        ).unwrap();
        assert!(income >= 5, "expected >= 5 income categories, got {income}");
        assert!(expense >= 20, "expected >= 20 expense categories, got {expense}");
    }

    #[test]
    fn test_encrypted_db_cannot_open_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");

        // Create encrypted DB
        set_db_password(Some("secret".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Try opening without password — should fail
        set_db_password(None);
        let result = get_connection(&db_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_db_opens_with_correct_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");

        set_db_password(Some("secret".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Reopen with same password
        set_db_password(Some("secret".into()));
        let conn = get_connection(&db_path).unwrap();
        let result: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_unencrypted_db_works_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");

        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        let result: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_is_encrypted_returns_false_for_plain_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");
        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_returns_true_for_encrypted_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        set_db_password(Some("secret".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password(None);
        assert!(is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_returns_false_for_nonexistent_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nope.db");
        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_categories_have_form_line() {
        let (_dir, conn) = test_db();
        let form_line: Option<String> = conn.query_row(
            "SELECT form_line FROM categories WHERE name = 'Payroll — Wages'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(form_line.as_deref(), Some("1120S-8"));
    }

    #[test]
    fn test_init_db_sets_schema_version() {
        let (_dir, conn) = test_db();
        let version = crate::migrations::get_schema_version(&conn).unwrap();
        assert_eq!(version, crate::migrations::LATEST_VERSION);
    }
}

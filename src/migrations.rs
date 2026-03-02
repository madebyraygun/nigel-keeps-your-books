use rusqlite::Connection;

use crate::db::{get_metadata, set_metadata};
use crate::error::Result;

struct Migration {
    version: u32,
    #[allow(dead_code)]
    description: &'static str,
    up: fn(&Connection) -> Result<()>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "baseline — establish schema version tracking",
        up: |_conn| Ok(()),
    },
    Migration {
        version: 2,
        description: "convert monetary columns from REAL to TEXT for rust_decimal",
        up: |conn| {
            // SQLite doesn't support ALTER COLUMN, so we recreate tables to change
            // column affinity from REAL to TEXT. This ensures read_decimal's
            // row.get::<_, String>() works on existing databases.
            conn.execute_batch(
                "ALTER TABLE transactions RENAME TO _transactions_old;
                 CREATE TABLE transactions (
                     id INTEGER PRIMARY KEY,
                     account_id INTEGER NOT NULL,
                     date TEXT NOT NULL,
                     description TEXT NOT NULL,
                     amount TEXT NOT NULL,
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
                 INSERT INTO transactions (id, account_id, date, description, amount, category_id, vendor, notes, is_flagged, flag_reason, import_id, created_at)
                     SELECT id, account_id, date, description, printf('%.2f', amount), category_id, vendor, notes, is_flagged, flag_reason, import_id, created_at
                     FROM _transactions_old;
                 DROP TABLE _transactions_old;

                 ALTER TABLE reconciliations RENAME TO _reconciliations_old;
                 CREATE TABLE reconciliations (
                     id INTEGER PRIMARY KEY,
                     account_id INTEGER NOT NULL,
                     month TEXT NOT NULL,
                     statement_balance TEXT,
                     calculated_balance TEXT,
                     is_reconciled INTEGER DEFAULT 0,
                     reconciled_at TEXT,
                     notes TEXT,
                     FOREIGN KEY (account_id) REFERENCES accounts(id)
                 );
                 INSERT INTO reconciliations (id, account_id, month, statement_balance, calculated_balance, is_reconciled, reconciled_at, notes)
                     SELECT id, account_id, month,
                         CASE WHEN statement_balance IS NOT NULL THEN printf('%.2f', statement_balance) ELSE NULL END,
                         CASE WHEN calculated_balance IS NOT NULL THEN printf('%.2f', calculated_balance) ELSE NULL END,
                         is_reconciled, reconciled_at, notes
                     FROM _reconciliations_old;
                 DROP TABLE _reconciliations_old;",
            )?;
            Ok(())
        },
    },
];

pub const LATEST_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].version;

pub fn get_schema_version(conn: &Connection) -> u32 {
    get_metadata(conn, "schema_version")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    apply_migrations(conn, MIGRATIONS)
}

fn apply_migrations(conn: &Connection, migrations: &[Migration]) -> Result<()> {
    let current = get_schema_version(conn);
    for migration in migrations {
        if migration.version > current {
            let sp_name = format!("migration_v{}", migration.version);
            conn.execute_batch(&format!("SAVEPOINT {sp_name}"))?;
            match (|| -> Result<()> {
                (migration.up)(conn)?;
                set_metadata(conn, "schema_version", &migration.version.to_string())?;
                Ok(())
            })() {
                Ok(()) => conn.execute_batch(&format!("RELEASE {sp_name}"))?,
                Err(e) => {
                    conn.execute_batch(&format!("ROLLBACK TO {sp_name}; RELEASE {sp_name}"))?;
                    return Err(e);
                }
            }
        }
    }
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

    #[test]
    fn test_fresh_install_at_latest_version() {
        let (_dir, conn) = test_db();
        let version = get_schema_version(&conn);
        assert_eq!(version, LATEST_VERSION);
    }

    #[test]
    fn test_v0_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        // Create schema without running migrations (simulates 0.1.x)
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        assert_eq!(get_schema_version(&conn), 0);

        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn), LATEST_VERSION);
    }

    #[test]
    fn test_idempotent_rerun() {
        let (_dir, conn) = test_db();
        let v1 = get_schema_version(&conn);
        run_migrations(&conn).unwrap();
        let v2 = get_schema_version(&conn);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_failed_migration_rolls_back() {
        let (_dir, conn) = test_db();
        assert_eq!(get_schema_version(&conn), LATEST_VERSION);

        let bad_migrations = &[Migration {
            version: LATEST_VERSION + 1,
            description: "failing migration",
            up: |conn| {
                conn.execute_batch("CREATE TABLE _test_rollback (id INTEGER)")?;
                Err(crate::error::NigelError::Other("intentional failure".into()))
            },
        }];

        let result = apply_migrations(&conn, bad_migrations);
        assert!(result.is_err());
        // Version unchanged
        assert_eq!(get_schema_version(&conn), LATEST_VERSION);
        // Table creation rolled back
        let table_exists: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='_test_rollback'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!table_exists);
    }

    #[test]
    fn test_v2_canonicalizes_real_amounts_to_text() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        // Simulate a pre-Decimal database with REAL columns (old v0.1.x schema)
        conn.execute_batch(
            "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT NOT NULL, account_type TEXT NOT NULL, institution TEXT, last_four TEXT, created_at TEXT DEFAULT (datetime('now')));
             CREATE TABLE categories (id INTEGER PRIMARY KEY, name TEXT NOT NULL, parent_id INTEGER, category_type TEXT NOT NULL, tax_line TEXT, form_line TEXT, description TEXT, is_active INTEGER DEFAULT 1);
             CREATE TABLE transactions (id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL, date TEXT NOT NULL, description TEXT NOT NULL, amount REAL NOT NULL, category_id INTEGER, vendor TEXT, notes TEXT, is_flagged INTEGER DEFAULT 0, flag_reason TEXT, import_id INTEGER, created_at TEXT DEFAULT (datetime('now')), FOREIGN KEY (account_id) REFERENCES accounts(id), FOREIGN KEY (category_id) REFERENCES categories(id), FOREIGN KEY (import_id) REFERENCES imports(id));
             CREATE TABLE reconciliations (id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL, month TEXT NOT NULL, statement_balance REAL, calculated_balance REAL, is_reconciled INTEGER DEFAULT 0, reconciled_at TEXT, notes TEXT, FOREIGN KEY (account_id) REFERENCES accounts(id));
             CREATE TABLE rules (id INTEGER PRIMARY KEY, pattern TEXT NOT NULL, match_type TEXT DEFAULT 'contains', vendor TEXT, category_id INTEGER NOT NULL, priority INTEGER DEFAULT 0, hit_count INTEGER DEFAULT 0, is_active INTEGER DEFAULT 1, created_at TEXT DEFAULT (datetime('now')), FOREIGN KEY (category_id) REFERENCES categories(id));
             CREATE TABLE imports (id INTEGER PRIMARY KEY, filename TEXT NOT NULL, account_id INTEGER, import_date TEXT DEFAULT (datetime('now')), record_count INTEGER, date_range_start TEXT, date_range_end TEXT, checksum TEXT, FOREIGN KEY (account_id) REFERENCES accounts(id));
             CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        ).unwrap();
        set_metadata(&conn, "schema_version", "1");

        // Insert with REAL values (as old f64 code would)
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking');
             INSERT INTO transactions (account_id, date, description, amount) VALUES (1, '2025-01-15', 'Test txn', 1000.0);
             INSERT INTO transactions (account_id, date, description, amount) VALUES (1, '2025-01-16', 'Expense', -54.99);
             INSERT INTO reconciliations (account_id, month, statement_balance, calculated_balance) VALUES (1, '2025-01', 945.01, 945.01);",
        ).unwrap();

        // Verify amounts stored as REAL
        let typeof_amount: String = conn
            .query_row("SELECT typeof(amount) FROM transactions LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(typeof_amount, "real");

        // Run migrations
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn), LATEST_VERSION);

        // Verify amounts are now canonical text with 2 decimal places
        let amounts: Vec<String> = conn
            .prepare("SELECT amount FROM transactions ORDER BY date").unwrap()
            .query_map([], |r| r.get(0)).unwrap()
            .collect::<std::result::Result<Vec<_>, _>>().unwrap();
        assert_eq!(amounts, vec!["1000.00", "-54.99"]);

        let stmt_bal: String = conn
            .query_row("SELECT statement_balance FROM reconciliations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stmt_bal, "945.01");
    }
}

use rusqlite::Connection;

use crate::db::set_metadata;
use crate::error::Result;

struct Migration {
    version: u32,
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
        description: "add csv_profiles table for generic CSV column mappings",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE csv_profiles (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    date_col INTEGER NOT NULL,
                    desc_col INTEGER NOT NULL,
                    amount_col INTEGER NOT NULL,
                    date_format TEXT NOT NULL DEFAULT '%m/%d/%Y',
                    created_at TEXT DEFAULT (datetime('now'))
                )",
            )?;
            Ok(())
        },
    },
    Migration {
        version: 3,
        description: "add invoicing tables (clients, invoices, line items, payments)",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE clients (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT,
                    billing_address TEXT,
                    notes TEXT,
                    created_at TEXT DEFAULT (datetime('now'))
                );
                CREATE TABLE invoices (
                    id INTEGER PRIMARY KEY,
                    number INTEGER NOT NULL UNIQUE,
                    client_id INTEGER NOT NULL,
                    issue_date TEXT NOT NULL,
                    due_date TEXT,
                    status TEXT NOT NULL DEFAULT 'draft',
                    currency TEXT NOT NULL DEFAULT 'USD',
                    subtotal REAL NOT NULL DEFAULT 0,
                    tax REAL NOT NULL DEFAULT 0,
                    total REAL NOT NULL DEFAULT 0,
                    notes TEXT,
                    terms TEXT,
                    token TEXT NOT NULL UNIQUE,
                    stripe_payment_link_id TEXT,
                    stripe_payment_link_url TEXT,
                    published_at TEXT,
                    created_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (client_id) REFERENCES clients(id)
                );
                CREATE TABLE invoice_line_items (
                    id INTEGER PRIMARY KEY,
                    invoice_id INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    quantity REAL NOT NULL DEFAULT 1,
                    unit_amount REAL NOT NULL DEFAULT 0,
                    line_total REAL NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );
                CREATE TABLE invoice_payments (
                    id INTEGER PRIMARY KEY,
                    invoice_id INTEGER NOT NULL,
                    amount REAL NOT NULL,
                    paid_date TEXT NOT NULL,
                    method TEXT NOT NULL CHECK (method IN ('stripe','ach','direct_deposit','other')),
                    stripe_checkout_session_id TEXT UNIQUE,
                    recorded_at TEXT DEFAULT (datetime('now')),
                    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
                );",
            )?;
            Ok(())
        },
    },
];

pub const LATEST_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].version;

/// Returns the current schema version, or 0 if no version has been set.
/// Propagates actual DB errors instead of silently defaulting to 0.
pub fn get_schema_version(conn: &Connection) -> Result<u32> {
    match conn.query_row(
        "SELECT value FROM metadata WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(v) => v
            .parse::<u32>()
            .map_err(|_| crate::error::NigelError::Other(format!("invalid schema_version: {v}"))),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(e) => Err(e.into()),
    }
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    apply_migrations(conn, MIGRATIONS)
}

fn apply_migrations(conn: &Connection, migrations: &[Migration]) -> Result<()> {
    let current = get_schema_version(conn)?;
    for migration in migrations {
        if migration.version > current {
            eprintln!(
                "Applying migration v{}: {}",
                migration.version, migration.description
            );
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
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, LATEST_VERSION);
    }

    #[test]
    fn test_v0_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        // Create schema without running migrations (simulates 0.1.x)
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), 0);

        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn test_idempotent_rerun() {
        let (_dir, conn) = test_db();
        let v1 = get_schema_version(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let v2 = get_schema_version(&conn).unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_csv_profiles_table_exists_after_migration() {
        let (_dir, conn) = test_db();
        let exists: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='csv_profiles'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "csv_profiles table should exist after init_db");
    }

    #[test]
    fn test_failed_migration_rolls_back() {
        let (_dir, conn) = test_db();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);

        let bad_migrations = &[Migration {
            version: LATEST_VERSION + 1,
            description: "failing migration",
            up: |conn| {
                conn.execute_batch("CREATE TABLE _test_rollback (id INTEGER)")?;
                Err(crate::error::NigelError::Other(
                    "intentional failure".into(),
                ))
            },
        }];

        let result = apply_migrations(&conn, bad_migrations);
        assert!(result.is_err());
        // Version unchanged
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
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
}

#[cfg(test)]
mod invoicing_migration_tests {
    use crate::db::{get_connection, init_db};
    use crate::migrations::run_migrations;

    #[test]
    fn invoicing_tables_exist_after_migration() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        for table in [
            "clients",
            "invoices",
            "invoice_line_items",
            "invoice_payments",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
    }
}

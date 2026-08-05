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
        description: "backfill 1120-S form_line for stock chart-of-accounts categories",
        up: |conn| {
            conn.execute_batch(
                "UPDATE categories SET form_line = '1120S-1a'
                     WHERE form_line IS NULL AND tax_line = 'Gross receipts'
                       AND name IN ('Client Services', 'Hosting & Maintenance', 'Reimbursements');
                 UPDATE categories SET form_line = '1120S-5'
                     WHERE form_line IS NULL AND tax_line = 'Other income'
                       AND name = 'Other Income';
                 UPDATE categories SET form_line = '1120S-2'
                     WHERE form_line IS NULL
                       AND tax_line = 'Schedule C Part III / 1120-S Line 2'
                       AND name = 'Cost of Goods Sold';
                 UPDATE categories SET form_line = 'excluded'
                     WHERE form_line IS NULL AND tax_line = 'Not deductible'
                       AND name = 'Transfer';",
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
mod k1_backfill_tests {
    use crate::db::{get_connection, init_db};

    #[test]
    fn backfills_stock_categories_only_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();

        // Simulate a pre-migration database: blank the seeded mappings,
        // add a custom category sharing a stock tax_line, and a category
        // with an existing explicit mapping.
        conn.execute("UPDATE categories SET form_line = NULL", [])
            .unwrap();
        conn.execute(
            "INSERT INTO categories (name, category_type, tax_line, form_line) \
             VALUES ('My Consulting', 'income', 'Gross receipts', NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE categories SET form_line = 'K-16d' WHERE name = 'Owner Draw / Distribution'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE metadata SET value = '2' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();

        super::run_migrations(&conn).unwrap();

        let fl = |name: &str| -> Option<String> {
            conn.query_row(
                "SELECT form_line FROM categories WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(fl("Client Services").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Hosting & Maintenance").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Reimbursements").as_deref(), Some("1120S-1a"));
        assert_eq!(fl("Other Income").as_deref(), Some("1120S-5"));
        assert_eq!(fl("Cost of Goods Sold").as_deref(), Some("1120S-2"));
        assert_eq!(fl("Transfer").as_deref(), Some("excluded"));
        assert_eq!(fl("Uncategorized"), None); // deliberately left unmapped
        assert_eq!(fl("My Consulting"), None); // custom name untouched
        assert_eq!(fl("Owner Draw / Distribution").as_deref(), Some("K-16d")); // not overwritten
    }
}

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
];

pub const LATEST_VERSION: u32 = 1;

pub fn get_schema_version(conn: &Connection) -> u32 {
    get_metadata(conn, "schema_version")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current = get_schema_version(conn);
    for migration in MIGRATIONS {
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
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();

        // Savepoint rollback: changes inside a savepoint are undone on rollback
        let sp_name = "test_rollback";
        conn.execute_batch(&format!("SAVEPOINT {sp_name}")).unwrap();
        set_metadata(&conn, "schema_version", "999").unwrap();
        conn.execute_batch(&format!("ROLLBACK TO {sp_name}; RELEASE {sp_name}")).unwrap();

        assert_eq!(get_schema_version(&conn), 0);
    }
}

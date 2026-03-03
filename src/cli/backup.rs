use std::path::{Path, PathBuf};

use rusqlite::backup::Backup;

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::settings::{get_data_dir, restrict_dir_permissions, restrict_file_permissions};

/// Copy the database to `dest_path` using SQLite's online-backup API.
/// Preserves encryption state: encrypted sources produce encrypted backups.
/// The `password` must match the source database's encryption key (if any).
pub fn snapshot(conn: &rusqlite::Connection, dest_path: &Path) -> Result<()> {
    snapshot_with_password(conn, dest_path, crate::db::get_db_password().as_deref())
}

/// Like `snapshot`, but accepts an explicit password instead of reading global state.
/// Used by tests that cannot rely on the global DB_PASSWORD mutex.
pub fn snapshot_with_password(
    conn: &rusqlite::Connection,
    dest_path: &Path,
    password: Option<&str>,
) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut dest_conn = rusqlite::Connection::open(dest_path)?;
    if let Some(pw) = password {
        dest_conn.pragma_update(None, "key", pw)?;
    }
    let backup = Backup::new(conn, &mut dest_conn)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    drop(backup);
    drop(dest_conn);
    restrict_file_permissions(dest_path)?;
    Ok(())
}

pub fn run(output: Option<String>) -> Result<()> {
    let data_dir = get_data_dir();
    let db_path = data_dir.join("nigel.db");
    let conn = get_connection(&db_path)?;

    let dest_path = match output {
        Some(p) => PathBuf::from(p),
        None => {
            let backups_dir = data_dir.join("backups");
            std::fs::create_dir_all(&backups_dir)?;
            restrict_dir_permissions(&backups_dir)?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            backups_dir.join(format!("nigel-{stamp}.db"))
        }
    };

    snapshot(&conn, &dest_path)?;

    let size = std::fs::metadata(&dest_path)?.len();
    println!("Backup saved to {}", dest_path.display());
    println!("Size: {}", format_bytes(size));
    Ok(())
}

// Root cause of prior failures: tests used the global DB_PASSWORD mutex for
// snapshot encryption, but cargo test runs in parallel so concurrent tests
// would overwrite each other's password.  Fixed by using snapshot_with_password()
// which accepts an explicit password, eliminating shared mutable state.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_db, is_encrypted, open_connection};

    #[test]
    fn test_snapshot_preserves_encryption() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.db");
        let dst_path = dir.path().join("backup.db");

        // Create encrypted source
        let conn = open_connection(&src_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Snapshot with explicit password (no global state dependency)
        let src_conn = open_connection(&src_path, Some("secret")).unwrap();
        snapshot_with_password(&src_conn, &dst_path, Some("secret")).unwrap();
        drop(src_conn);

        // Verify backup is also encrypted
        assert!(is_encrypted(&dst_path).unwrap());

        // Verify backup opens with same password
        let conn = open_connection(&dst_path, Some("secret")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_snapshot_plain_stays_plain() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.db");
        let dst_path = dir.path().join("backup.db");

        let conn = open_connection(&src_path, None).unwrap();
        init_db(&conn).unwrap();

        snapshot_with_password(&conn, &dst_path, None).unwrap();
        drop(conn);

        assert!(!is_encrypted(&dst_path).unwrap());
    }
}

use std::path::{Path, PathBuf};

use rusqlite::backup::Backup;

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::settings::{get_data_dir, restrict_dir_permissions, restrict_file_permissions};

/// Copy the database to `dest_path` using SQLite's online-backup API.
/// Preserves encryption state: encrypted sources produce encrypted backups.
pub fn snapshot(conn: &rusqlite::Connection, dest_path: &Path) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
        restrict_dir_permissions(parent)?;
    }
    let password = crate::db::get_db_password();
    let mut dest_conn = rusqlite::Connection::open(dest_path)?;
    if let Some(ref pw) = password {
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

// Tests mutate the global DB_PASSWORD mutex and must run with --test-threads=1.
// See also: db::tests, cli::password::tests.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db, is_encrypted, open_connection, set_db_password};

    #[test]
    fn test_snapshot_preserves_encryption() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.db");
        let dst_path = dir.path().join("backup.db");

        // Create encrypted source
        set_db_password(Some("secret".into()));
        let conn = get_connection(&src_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Snapshot
        let src_conn = get_connection(&src_path).unwrap();
        snapshot(&src_conn, &dst_path).unwrap();
        drop(src_conn);

        // Verify backup is also encrypted
        set_db_password(None);
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

        set_db_password(None);
        let conn = get_connection(&src_path).unwrap();
        init_db(&conn).unwrap();

        snapshot(&conn, &dst_path).unwrap();
        drop(conn);

        assert!(!is_encrypted(&dst_path).unwrap());
    }
}

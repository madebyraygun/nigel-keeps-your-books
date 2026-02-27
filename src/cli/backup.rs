use std::path::PathBuf;

use rusqlite::backup::Backup;

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::settings::get_data_dir;

pub fn run(output: Option<String>) -> Result<()> {
    let data_dir = get_data_dir();
    let db_path = data_dir.join("nigel.db");
    let conn = get_connection(&db_path)?;

    let dest_path = match output {
        Some(p) => PathBuf::from(p),
        None => {
            let backups_dir = data_dir.join("backups");
            std::fs::create_dir_all(&backups_dir)?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            backups_dir.join(format!("nigel-{stamp}.db"))
        }
    };

    let mut dest_conn = rusqlite::Connection::open(&dest_path)?;
    let backup = Backup::new(&conn, &mut dest_conn)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;

    let size = std::fs::metadata(&dest_path)?.len();
    println!("Backup saved to {}", dest_path.display());
    println!("Size: {}", format_bytes(size));
    Ok(())
}

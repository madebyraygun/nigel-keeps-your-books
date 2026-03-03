use std::path::PathBuf;

use crate::categorizer::categorize_transactions;
use crate::cli::backup;
use crate::db::get_connection;
use crate::error::Result;
use crate::importer::import_file;
use crate::settings::get_data_dir;

pub fn run(file: &str, account: &str, format: Option<&str>, dry_run: bool) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    if !dry_run {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-import snapshot saved to {}", snap_path.display());
    }

    let result = import_file(&conn, &file_path, account, format, dry_run)?;

    if result.duplicate_file {
        println!("This file has already been imported (duplicate checksum).");
        return Ok(());
    }

    if dry_run {
        println!("Dry run \u{2014} no changes made");
        if result.malformed > 0 {
            println!(
                "{} would be imported, {} duplicates, {} malformed",
                result.imported, result.skipped, result.malformed
            );
        } else {
            println!(
                "{} would be imported, {} duplicates",
                result.imported, result.skipped
            );
        }
        if !result.sample.is_empty() {
            println!("\nSample transactions:");
            for row in &result.sample {
                let sign = if row.amount >= 0.0 { "+" } else { "" };
                println!(
                    "  {}  {:40} {:>10}",
                    row.date,
                    row.description,
                    format!("{sign}{:.2}", row.amount)
                );
            }
        }
        return Ok(());
    }

    if result.malformed > 0 {
        println!(
            "{} imported, {} skipped (duplicates), {} skipped (malformed data)",
            result.imported, result.skipped, result.malformed
        );
    } else {
        println!(
            "{} imported, {} skipped (duplicates)",
            result.imported, result.skipped
        );
    }

    let cat_result = categorize_transactions(&conn)?;
    println!(
        "{} categorized, {} still flagged",
        cat_result.categorized, cat_result.still_flagged
    );

    Ok(())
}

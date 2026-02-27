use std::path::PathBuf;

use crate::categorizer::categorize_transactions;
use crate::db::get_connection;
use crate::error::Result;
use crate::importer::import_file;
use crate::settings::get_data_dir;

pub fn run(file: &str, account: &str, format: Option<&str>) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    let result = import_file(&conn, &file_path, account, format)?;

    if result.duplicate_file {
        println!("This file has already been imported (duplicate checksum).");
        return Ok(());
    }

    println!("{} imported, {} skipped (duplicates)", result.imported, result.skipped);

    let cat_result = categorize_transactions(&conn)?;
    println!(
        "{} categorized, {} still flagged",
        cat_result.categorized, cat_result.still_flagged
    );

    Ok(())
}

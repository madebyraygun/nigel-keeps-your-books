use std::path::PathBuf;

use crate::categorizer::categorize_transactions;
use crate::cli::backup;
use crate::db::get_connection;
use crate::error::Result;
use crate::importer::{import_file, save_csv_profile, GenericCsvConfig};
use crate::settings::get_data_dir;

pub fn run(
    file: &str,
    account: &str,
    format: Option<&str>,
    dry_run: bool,
    date_col: Option<usize>,
    desc_col: Option<usize>,
    amount_col: Option<usize>,
    date_format: Option<&str>,
    save_profile: Option<&str>,
) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    // If --save-profile is provided, save the generic CSV config
    if let Some(profile_name) = save_profile {
        let config = GenericCsvConfig {
            date_col: date_col.ok_or_else(|| {
                crate::error::NigelError::Other(
                    "--date-col is required with --save-profile".into(),
                )
            })?,
            desc_col: desc_col.ok_or_else(|| {
                crate::error::NigelError::Other(
                    "--desc-col is required with --save-profile".into(),
                )
            })?,
            amount_col: amount_col.ok_or_else(|| {
                crate::error::NigelError::Other(
                    "--amount-col is required with --save-profile".into(),
                )
            })?,
            date_format: date_format.unwrap_or("%m/%d/%Y").to_string(),
        };
        save_csv_profile(&conn, profile_name, &config)?;
        println!("Saved profile '{profile_name}'");
    }

    // Determine effective format:
    // - If column flags provided without --format, save as __inline__ and use that
    // - Otherwise use whatever --format was given (or None for auto-detect)
    let effective_format: Option<String> = if format.is_none() && date_col.is_some() {
        let config = GenericCsvConfig {
            date_col: date_col.unwrap(),
            desc_col: desc_col.ok_or_else(|| {
                crate::error::NigelError::Other("--desc-col is required with --date-col".into())
            })?,
            amount_col: amount_col.ok_or_else(|| {
                crate::error::NigelError::Other(
                    "--amount-col is required with --date-col".into(),
                )
            })?,
            date_format: date_format.unwrap_or("%m/%d/%Y").to_string(),
        };
        save_csv_profile(&conn, "__inline__", &config)?;
        Some("__inline__".to_string())
    } else {
        format.map(|s| s.to_string())
    };

    if !dry_run {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-import snapshot saved to {}", snap_path.display());
    }

    let result = import_file(
        &conn,
        &file_path,
        account,
        effective_format.as_deref(),
        dry_run,
    )?;

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

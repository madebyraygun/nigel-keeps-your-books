use std::path::PathBuf;

use crate::categorizer::categorize_transactions;
use crate::cli::backup;
use crate::db::get_connection;
use crate::error::Result;
use crate::importer::{import_file, save_csv_profile, GenericCsvConfig};
use crate::settings::get_data_dir;

pub struct ImportOpts<'a> {
    pub format: Option<&'a str>,
    pub dry_run: bool,
    pub date_col: Option<usize>,
    pub desc_col: Option<usize>,
    pub amount_col: Option<usize>,
    pub date_format: Option<&'a str>,
    pub save_profile: Option<&'a str>,
}

pub fn run(file: &str, account: &str, opts: ImportOpts<'_>) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    // Build inline config when column flags are provided without --format
    let inline_config = if opts.format.is_none() && opts.date_col.is_some() {
        Some(build_generic_config(
            opts.date_col,
            opts.desc_col,
            opts.amount_col,
            opts.date_format,
            "--date-col",
        )?)
    } else {
        None
    };

    // Save profile only when not doing a dry run
    if !opts.dry_run {
        if let Some(profile_name) = opts.save_profile {
            let config = inline_config.as_ref().map_or_else(
                || {
                    build_generic_config(
                        opts.date_col,
                        opts.desc_col,
                        opts.amount_col,
                        opts.date_format,
                        "--save-profile",
                    )
                },
                |c| Ok(c.clone()),
            )?;
            save_csv_profile(&conn, profile_name, &config)?;
            println!("Saved profile '{profile_name}'");
        }
    }

    if !opts.dry_run {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-import snapshot saved to {}", snap_path.display());
    }

    let result = import_file(
        &conn,
        &file_path,
        account,
        opts.format,
        opts.dry_run,
        inline_config.as_ref(),
    )?;

    if result.duplicate_file {
        println!("This file has already been imported (duplicate checksum).");
        return Ok(());
    }

    if opts.dry_run {
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

fn build_generic_config(
    date_col: Option<usize>,
    desc_col: Option<usize>,
    amount_col: Option<usize>,
    date_format: Option<&str>,
    context: &str,
) -> Result<GenericCsvConfig> {
    Ok(GenericCsvConfig {
        date_col: date_col.ok_or_else(|| {
            crate::error::NigelError::Other(format!("--date-col is required with {context}"))
        })?,
        desc_col: desc_col.ok_or_else(|| {
            crate::error::NigelError::Other(format!("--desc-col is required with {context}"))
        })?,
        amount_col: amount_col.ok_or_else(|| {
            crate::error::NigelError::Other(format!("--amount-col is required with {context}"))
        })?,
        date_format: date_format.unwrap_or("%m/%d/%Y").to_string(),
    })
}

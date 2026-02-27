use crate::categorizer::categorize_transactions;
use crate::db::get_connection;
use crate::error::Result;
use crate::settings::get_data_dir;

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let result = categorize_transactions(&conn)?;
    println!(
        "{} categorized, {} still flagged",
        result.categorized, result.still_flagged
    );
    Ok(())
}

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::format_bytes;
use crate::settings::load_settings;

pub fn run() -> Result<()> {
    let settings = load_settings();
    let data_dir = std::path::PathBuf::from(&settings.data_dir);
    let db_path = data_dir.join("nigel.db");

    println!("Company:    {}", if settings.company_name.is_empty() { "(not set)" } else { &settings.company_name });
    println!("Data dir:   {}", data_dir.display());
    println!("Database:   {}", db_path.display());

    if db_path.exists() {
        let size = std::fs::metadata(&db_path)?.len();
        println!("DB size:    {}", format_bytes(size));

        let conn = get_connection(&db_path)?;
        let accounts: i64 = conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))?;
        let transactions: i64 = conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))?;
        let flagged: i64 = conn.query_row(
            "SELECT count(*) FROM transactions WHERE is_flagged = 1",
            [],
            |r| r.get(0),
        )?;
        let rules: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0))?;

        println!();
        println!("Accounts:      {accounts}");
        println!("Transactions:  {transactions}");
        println!("Flagged:       {flagged}");
        println!("Rules:         {rules}");
    } else {
        println!();
        println!("Database not found. Run `nigel init` to set up.");
    }

    Ok(())
}

use rust_decimal::Decimal;

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reconciler;
use crate::settings::get_data_dir;

pub fn run(account: &str, month: &str, balance: Decimal) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let result = reconciler::reconcile(&conn, account, month, balance)?;

    if result.is_reconciled {
        println!("Reconciled! Calculated: {}", money(result.calculated_balance));
    } else {
        println!(
            "DISCREPANCY: {}\n  Statement:  {}\n  Calculated: {}",
            money(result.discrepancy),
            money(result.statement_balance),
            money(result.calculated_balance)
        );
    }
    Ok(())
}

use std::io::IsTerminal;

use clap::Args;
use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::reviewer::{
    find_transactions_for_recategorize, get_transactions_by_ids, recategorize_transactions,
    RecategorizeFilter,
};
use crate::settings::get_data_dir;

#[derive(Args)]
pub struct RecategorizeArgs {
    /// Transaction IDs to recategorize (mutually exclusive with filters)
    pub ids: Vec<i64>,
    /// Target category name
    #[arg(long)]
    pub category: String,
    /// Filter: current category name
    #[arg(long = "from-category")]
    pub from_category: Option<String>,
    /// Filter: only uncategorized transactions
    #[arg(long)]
    pub uncategorized: bool,
    /// Filter: year YYYY
    #[arg(long)]
    pub year: Option<i32>,
    /// Filter: month YYYY-MM
    #[arg(long)]
    pub month: Option<String>,
    /// Filter: start date YYYY-MM-DD (requires --to)
    #[arg(long = "from")]
    pub from_date: Option<String>,
    /// Filter: end date YYYY-MM-DD (requires --from)
    #[arg(long = "to")]
    pub to_date: Option<String>,
    /// Filter: description pattern
    #[arg(long)]
    pub pattern: Option<String>,
    /// Match type for --pattern: contains, starts_with, regex
    #[arg(long = "match-type", default_value = "contains")]
    pub match_type: String,
    /// Filter: account name
    #[arg(long)]
    pub account: Option<String>,
    /// Filter: minimum absolute amount
    #[arg(long = "min-amount")]
    pub min_amount: Option<f64>,
    /// Filter: maximum absolute amount
    #[arg(long = "max-amount")]
    pub max_amount: Option<f64>,
    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,
    /// Apply without confirmation (filter mode; required when stdin is not a TTY)
    #[arg(long)]
    pub yes: bool,
}

fn has_filters(args: &RecategorizeArgs) -> bool {
    args.from_category.is_some()
        || args.uncategorized
        || args.year.is_some()
        || args.month.is_some()
        || args.from_date.is_some()
        || args.to_date.is_some()
        || args.pattern.is_some()
        || args.account.is_some()
        || args.min_amount.is_some()
        || args.max_amount.is_some()
}

pub(crate) fn validate(args: &RecategorizeArgs) -> Result<()> {
    let valid_types = ["contains", "starts_with", "regex"];
    if !valid_types.contains(&args.match_type.as_str()) {
        return Err(NigelError::Other(format!(
            "Invalid match type: {}. Must be one of: {}",
            args.match_type,
            valid_types.join(", ")
        )));
    }
    if args.match_type == "regex" {
        if let Some(ref p) = args.pattern {
            regex::Regex::new(p).map_err(|e| NigelError::Other(format!("Invalid regex: {e}")))?;
        }
    }
    if !args.ids.is_empty() && has_filters(args) {
        return Err(NigelError::Other(
            "Select transactions with either IDs or filters, not both".to_string(),
        ));
    }
    if args.ids.is_empty() && !has_filters(args) {
        return Err(NigelError::Other(
            "Provide transaction IDs or at least one filter (see `nigel recategorize --help`)"
                .to_string(),
        ));
    }
    if args.uncategorized && args.from_category.is_some() {
        return Err(NigelError::Other(
            "--uncategorized and --from-category are mutually exclusive".to_string(),
        ));
    }
    if (args.year.is_some() || args.month.is_some())
        && (args.from_date.is_some() || args.to_date.is_some())
    {
        return Err(NigelError::Other(
            "--year/--month and --from/--to are mutually exclusive".to_string(),
        ));
    }
    if args.year.is_some() && args.month.is_some() {
        return Err(NigelError::Other(
            "--year and --month are mutually exclusive (the month already names its year)"
                .to_string(),
        ));
    }
    // parse_month_opt is lenient — a malformed month would silently widen the
    // selection to the whole ledger, so this write path must hard-error instead.
    if let Some(ref m) = args.month {
        let parts: Vec<&str> = m.split('-').collect();
        let ok = parts.len() == 2
            && parts[0].len() == 4
            && parts[0].parse::<i32>().is_ok()
            && parts[1]
                .parse::<u32>()
                .map(|v| (1..=12).contains(&v))
                .unwrap_or(false);
        if !ok {
            return Err(NigelError::Other(format!(
                "Invalid month '{m}': expected YYYY-MM (e.g. 2025-04)"
            )));
        }
    }
    for (flag, value) in [
        ("--min-amount", args.min_amount),
        ("--max-amount", args.max_amount),
    ] {
        if let Some(v) = value {
            if v < 0.0 {
                return Err(NigelError::Other(format!(
                    "{flag} must be non-negative: amounts are compared by absolute value"
                )));
            }
        }
    }
    if let (Some(min), Some(max)) = (args.min_amount, args.max_amount) {
        if min > max {
            return Err(NigelError::Other(
                "--min-amount is greater than --max-amount".to_string(),
            ));
        }
    }
    Ok(())
}

pub fn run(args: RecategorizeArgs) -> Result<()> {
    validate(&args)?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let target_id: i64 = match conn.query_row(
        "SELECT id, is_active FROM categories WHERE name = ?1 ORDER BY is_active DESC",
        [&args.category],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    ) {
        Ok((id, 1)) => id,
        Ok((_, _)) => {
            return Err(NigelError::Other(format!(
                "Category '{}' is inactive. Reactivate it or pick another category.",
                args.category
            )))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Err(NigelError::UnknownCategory(args.category.clone()))
        }
        Err(e) => return Err(e.into()),
    };

    let candidates = if args.ids.is_empty() {
        let from_category_id = match &args.from_category {
            Some(name) => {
                match conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |r| {
                    r.get(0)
                }) {
                    Ok(id) => Some(id),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(NigelError::UnknownCategory(name.clone()))
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            None => None,
        };
        let account_id = match &args.account {
            Some(name) => {
                match conn.query_row("SELECT id FROM accounts WHERE name = ?1", [name], |r| {
                    r.get(0)
                }) {
                    Ok(id) => Some(id),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(NigelError::UnknownAccount(name.clone()))
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            None => None,
        };
        // validate() guarantees --month is well-formed and excludes --year here.
        let (year, month) = if args.month.is_some() {
            crate::cli::parse_month_opt(&args.month)
        } else {
            (args.year, None)
        };
        let filter = RecategorizeFilter {
            from_category_id,
            uncategorized: args.uncategorized,
            year,
            month,
            from_date: args.from_date.clone(),
            to_date: args.to_date.clone(),
            pattern: args.pattern.clone(),
            match_type: args.match_type.clone(),
            account_id,
            min_amount: args.min_amount,
            max_amount: args.max_amount,
        };
        find_transactions_for_recategorize(&conn, &filter)?
    } else {
        let mut ids = args.ids.clone();
        ids.sort_unstable();
        ids.dedup();
        get_transactions_by_ids(&conn, &ids)?
    };

    let (to_move, already): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|c| c.category_id != Some(target_id));

    if to_move.is_empty() && already.is_empty() {
        println!("No transactions matched.");
        return Ok(());
    }

    if !to_move.is_empty() {
        let mut table = Table::new();
        table.set_header(vec!["ID", "Date", "Description", "Amount", "Category", "→"]);
        let mut total = 0.0;
        for c in &to_move {
            total += c.amount.abs();
            table.add_row(vec![
                Cell::new(c.id),
                Cell::new(&c.date),
                Cell::new(&c.description),
                Cell::new(format!("{:.2}", c.amount)),
                Cell::new(c.category.as_deref().unwrap_or("—")),
                Cell::new(&args.category),
            ]);
        }
        println!("{table}");
        println!(
            "{} transaction{} → {} (total ${:.2})",
            to_move.len(),
            if to_move.len() == 1 { "" } else { "s" },
            args.category,
            total
        );
    }
    if !already.is_empty() {
        println!("Skipping {} already in {}.", already.len(), args.category);
    }

    if to_move.is_empty() {
        return Ok(());
    }
    if args.dry_run {
        println!("Dry run — nothing written.");
        return Ok(());
    }

    // Filter mode requires confirmation; explicit IDs are their own confirmation.
    if args.ids.is_empty() && !args.yes {
        if std::io::stdin().is_terminal() {
            print!("Apply? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !answer.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        } else {
            return Err(NigelError::Other(
                "Refusing to apply a filter-based recategorization without confirmation. Pass --yes."
                    .to_string(),
            ));
        }
    }

    let ids: Vec<i64> = to_move.iter().map(|c| c.id).collect();
    let updated = recategorize_transactions(&conn, &ids, target_id)?;
    println!(
        "Recategorized {updated} transaction{} → {}.",
        if updated == 1 { "" } else { "s" },
        args.category
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> RecategorizeArgs {
        RecategorizeArgs {
            ids: Vec::new(),
            category: "Travel".to_string(),
            from_category: None,
            uncategorized: false,
            year: None,
            month: None,
            from_date: None,
            to_date: None,
            pattern: None,
            match_type: "contains".to_string(),
            account: None,
            min_amount: None,
            max_amount: None,
            dry_run: false,
            yes: false,
        }
    }

    #[test]
    fn ids_and_filters_conflict() {
        let mut args = base_args();
        args.ids = vec![1];
        args.year = Some(2025);
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("either IDs or filters"), "got: {err}");
    }

    #[test]
    fn no_ids_no_filters_errors() {
        let args = base_args();
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("at least one filter"), "got: {err}");
    }

    #[test]
    fn uncategorized_conflicts_with_from_category() {
        let mut args = base_args();
        args.uncategorized = true;
        args.from_category = Some("Travel".to_string());
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"), "got: {err}");
    }

    #[test]
    fn year_conflicts_with_date_range() {
        let mut args = base_args();
        args.year = Some(2025);
        args.from_date = Some("2025-01-01".to_string());
        args.to_date = Some("2025-06-30".to_string());
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"), "got: {err}");
    }

    #[test]
    fn bad_match_type_errors() {
        let mut args = base_args();
        args.ids = vec![1];
        args.match_type = "fuzzy".to_string();
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("contains, starts_with, regex"), "got: {err}");
    }

    #[test]
    fn bad_regex_errors() {
        let mut args = base_args();
        args.pattern = Some("(".to_string());
        args.match_type = "regex".to_string();
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("Invalid regex"), "got: {err}");
    }

    #[test]
    fn year_and_month_conflict() {
        let mut args = base_args();
        args.year = Some(2025);
        args.month = Some("2025-06".to_string());
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"), "got: {err}");
    }

    #[test]
    fn malformed_month_errors() {
        for bad in ["April", "2025", "2025/06", "2025-", "2025-13", "25-06"] {
            let mut args = base_args();
            args.month = Some(bad.to_string());
            let err = validate(&args).unwrap_err().to_string();
            assert!(err.contains("expected YYYY-MM"), "'{bad}' got: {err}");
        }
        let mut args = base_args();
        args.month = Some("2025-06".to_string());
        assert!(validate(&args).is_ok());
    }

    #[test]
    fn negative_amounts_error() {
        let mut args = base_args();
        args.min_amount = Some(-100.0);
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("non-negative"), "got: {err}");

        let mut args = base_args();
        args.min_amount = Some(500.0);
        args.max_amount = Some(100.0);
        let err = validate(&args).unwrap_err().to_string();
        assert!(err.contains("greater than"), "got: {err}");
    }

    #[test]
    fn valid_id_mode_passes() {
        let mut args = base_args();
        args.ids = vec![1, 2];
        assert!(validate(&args).is_ok());
    }

    #[test]
    fn valid_filter_mode_passes() {
        let mut args = base_args();
        args.from_category = Some("Cost of Goods Sold".to_string());
        args.year = Some(2025);
        assert!(validate(&args).is_ok());
    }
}

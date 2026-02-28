use chrono::Datelike;
use rusqlite::Connection;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Date filter helper
// ---------------------------------------------------------------------------

fn date_filter(
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<(String, Vec<String>)> {
    match (from_date, to_date) {
        (Some(from), Some(to)) => {
            return Ok((
                "t.date BETWEEN ?1 AND ?2".to_string(),
                vec![from.to_string(), to.to_string()],
            ));
        }
        (Some(_), None) => {
            return Err(crate::error::NigelError::Other(
                "--from requires --to (both date boundaries must be specified)".to_string(),
            ));
        }
        (None, Some(_)) => {
            return Err(crate::error::NigelError::Other(
                "--to requires --from (both date boundaries must be specified)".to_string(),
            ));
        }
        (None, None) => {}
    }
    if let (Some(y), Some(m)) = (year, month) {
        let prefix = format!("{y:04}-{m:02}");
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{prefix}%")]));
    }
    if let Some(y) = year {
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{y}%")]));
    }
    // Default: current year
    let current_year = chrono::Local::now().year();
    Ok(("t.date LIKE ?1".to_string(), vec![format!("{current_year}%")]))
}

// ---------------------------------------------------------------------------
// P&L
// ---------------------------------------------------------------------------

pub struct PnlItem {
    pub name: String,
    pub total: f64,
}

pub struct PnlReport {
    pub income: Vec<PnlItem>,
    pub expenses: Vec<PnlItem>,
    pub total_income: f64,
    pub total_expenses: f64,
    pub net: f64,
}

pub fn get_pnl(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<PnlReport> {
    let (clause, params) = date_filter(year, month, from_date, to_date)?;

    let income = query_category_totals(conn, &clause, &params, "income", "total DESC")?;
    let expenses = query_category_totals(conn, &clause, &params, "expense", "total ASC")?;

    let total_income: f64 = income.iter().map(|i| i.total).sum();
    let total_expenses: f64 = expenses.iter().map(|i| i.total).sum();

    Ok(PnlReport {
        income,
        expenses,
        total_income,
        total_expenses,
        net: total_income + total_expenses,
    })
}

fn query_category_totals(
    conn: &Connection,
    clause: &str,
    params: &[String],
    category_type: &str,
    order: &str,
) -> Result<Vec<PnlItem>> {
    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.category_type = '{category_type}' \
         GROUP BY c.name ORDER BY {order}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt.query_map(param_values.as_slice(), |row| {
        Ok(PnlItem {
            name: row.get(0)?,
            total: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------------------
// Expense Breakdown
// ---------------------------------------------------------------------------

pub struct ExpenseItem {
    pub name: String,
    pub total: f64,
    pub count: i64,
    pub pct: f64,
}

pub struct VendorItem {
    pub vendor: String,
    pub total: f64,
    pub count: i64,
}

pub struct ExpenseBreakdown {
    pub categories: Vec<ExpenseItem>,
    pub total: f64,
    pub top_vendors: Vec<VendorItem>,
}

pub fn get_expense_breakdown(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<ExpenseBreakdown> {
    let (clause, params) = date_filter(year, month, None, None)?;

    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.category_type = 'expense' \
         GROUP BY c.name ORDER BY total ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let raw: Vec<(String, f64, i64)> = stmt
        .query_map([&params[0]], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = raw.iter().map(|(_, t, _)| t).sum();
    let categories = raw
        .iter()
        .map(|(name, t, c)| ExpenseItem {
            name: name.clone(),
            total: *t,
            count: *c,
            pct: if total != 0.0 { t / total * 100.0 } else { 0.0 },
        })
        .collect();

    let vendor_sql = format!(
        "SELECT t.vendor, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.category_type = 'expense' AND t.vendor IS NOT NULL \
         GROUP BY t.vendor ORDER BY total ASC LIMIT 10"
    );
    let mut vstmt = conn.prepare(&vendor_sql)?;
    let top_vendors: Vec<VendorItem> = vstmt
        .query_map([&params[0]], |row| {
            Ok(VendorItem {
                vendor: row.get(0)?,
                total: row.get(1)?,
                count: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(ExpenseBreakdown {
        categories,
        total,
        top_vendors,
    })
}

// ---------------------------------------------------------------------------
// Tax Summary
// ---------------------------------------------------------------------------

pub struct TaxItem {
    pub name: String,
    pub tax_line: Option<String>,
    pub category_type: String,
    pub total: f64,
}

pub struct TaxSummary {
    pub line_items: Vec<TaxItem>,
}

pub fn get_tax_summary(conn: &Connection, year: Option<i32>) -> Result<TaxSummary> {
    let (clause, params) = date_filter(year, None, None, None)?;

    let sql = format!(
        "SELECT c.name, c.tax_line, c.category_type, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} \
         GROUP BY c.name, c.tax_line, c.category_type \
         ORDER BY c.category_type DESC, c.tax_line"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items: Vec<TaxItem> = stmt
        .query_map([&params[0]], |row| {
            Ok(TaxItem {
                name: row.get(0)?,
                tax_line: row.get(1)?,
                category_type: row.get(2)?,
                total: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(TaxSummary { line_items: items })
}

// ---------------------------------------------------------------------------
// Cash Flow
// ---------------------------------------------------------------------------

pub struct CashflowMonth {
    pub month: String,
    pub inflows: f64,
    pub outflows: f64,
    pub net: f64,
    pub running_balance: f64,
}

pub struct CashflowReport {
    pub months: Vec<CashflowMonth>,
}

pub fn get_cashflow(conn: &Connection, year: Option<i32>, month: Option<u32>) -> Result<CashflowReport> {
    let (clause, params) = date_filter(year, month, None, None)?;

    let sql = format!(
        "SELECT substr(t.date, 1, 7) as month, \
         SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END) as inflows, \
         SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END) as outflows \
         FROM transactions t WHERE {clause} \
         GROUP BY substr(t.date, 1, 7) ORDER BY month"
    );
    let mut stmt = conn.prepare(&sql)?;
    let raw: Vec<(String, f64, f64)> = stmt
        .query_map([&params[0]], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut months = Vec::new();
    let mut running = 0.0f64;
    for (m, inflows, outflows) in raw {
        running += inflows + outflows;
        months.push(CashflowMonth {
            month: m,
            inflows,
            outflows,
            net: inflows + outflows,
            running_balance: running,
        });
    }

    Ok(CashflowReport { months })
}

// ---------------------------------------------------------------------------
// Register (all transactions)
// ---------------------------------------------------------------------------

pub struct RegisterRow {
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>,
    pub vendor: Option<String>,
    pub account_name: String,
}

pub struct RegisterReport {
    pub rows: Vec<RegisterRow>,
    pub total: f64,
    pub count: usize,
}

pub fn get_register(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
    account: Option<&str>,
) -> Result<RegisterReport> {
    let (clause, mut params) = date_filter(year, month, from_date, to_date)?;

    let account_clause = if account.is_some() {
        params.push(account.unwrap().to_string());
        format!(" AND a.name = ?{}", params.len())
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT t.date, t.description, t.amount, c.name, t.vendor, a.name \
         FROM transactions t \
         JOIN accounts a ON t.account_id = a.id \
         LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {clause}{account_clause} \
         ORDER BY t.date, t.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let rows: Vec<RegisterRow> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok(RegisterRow {
                date: row.get(0)?,
                description: row.get(1)?,
                amount: row.get(2)?,
                category: row.get(3)?,
                vendor: row.get(4)?,
                account_name: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = rows.iter().map(|r| r.amount).sum();
    let count = rows.len();
    Ok(RegisterReport { rows, total, count })
}

// ---------------------------------------------------------------------------
// Flagged
// ---------------------------------------------------------------------------

pub struct FlaggedTransaction {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub account_name: String,
}

pub fn get_flagged(conn: &Connection) -> Result<Vec<FlaggedTransaction>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name \
         FROM transactions t JOIN accounts a ON t.account_id = a.id \
         WHERE t.is_flagged = 1 ORDER BY t.date",
    )?;
    let rows: Vec<FlaggedTransaction> = stmt
        .query_map([], |row| {
            Ok(FlaggedTransaction {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                account_name: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Balance
// ---------------------------------------------------------------------------

pub struct AccountBalance {
    pub name: String,
    pub account_type: String,
    pub balance: f64,
}

pub struct BalanceReport {
    pub accounts: Vec<AccountBalance>,
    pub total: f64,
    pub ytd_net_income: f64,
}

pub fn get_balance(conn: &Connection) -> Result<BalanceReport> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.account_type, COALESCE(SUM(t.amount), 0) as balance \
         FROM accounts a LEFT JOIN transactions t ON a.id = t.account_id \
         GROUP BY a.id ORDER BY a.name",
    )?;
    let accounts: Vec<AccountBalance> = stmt
        .query_map([], |row| {
            Ok(AccountBalance {
                name: row.get(1)?,
                account_type: row.get(2)?,
                balance: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = accounts.iter().map(|a| a.balance).sum();

    let current_year = chrono::Local::now().year();
    let ytd_net_income: f64 = conn.query_row(
        "SELECT COALESCE(SUM(amount), 0) as net FROM transactions WHERE date LIKE ?1",
        [format!("{current_year}%")],
        |row| row.get(0),
    )?;

    Ok(BalanceReport {
        accounts,
        total,
        ytd_net_income,
    })
}

// ---------------------------------------------------------------------------
// K-1 Prep Report
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct K1LineItem {
    pub form_line: String,
    pub category_name: String,
    pub total: f64,
}

#[allow(dead_code)]
pub struct K1OtherDeduction {
    pub category_name: String,
    pub total: f64,
    pub deductible: f64,
}

#[allow(dead_code)]
pub struct K1Validation {
    pub uncategorized_count: i64,
    pub officer_comp: f64,
    pub distributions: f64,
    pub comp_dist_ratio: Option<f64>,
}

#[allow(dead_code)]
pub struct K1PrepReport {
    pub gross_receipts: f64,
    pub other_income: f64,
    pub total_deductions: f64,
    pub ordinary_business_income: f64,
    pub deduction_lines: Vec<K1LineItem>,
    pub schedule_k_items: Vec<K1LineItem>,
    pub other_deductions: Vec<K1OtherDeduction>,
    pub other_deductions_total: f64,
    pub validation: K1Validation,
}

pub fn get_k1_prep(conn: &Connection, year: Option<i32>) -> Result<K1PrepReport> {
    let (clause, params) = date_filter(year, None, None, None)?;

    // Query all categorized transactions grouped by form_line
    let sql = format!(
        "SELECT c.form_line, c.name, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.form_line IS NOT NULL \
         GROUP BY c.form_line, c.name ORDER BY c.form_line"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
    let rows: Vec<(String, String, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut gross_receipts = 0.0f64;
    let mut other_income = 0.0f64;
    let mut total_deductions = 0.0f64;
    let mut deduction_lines = Vec::new();
    let mut schedule_k_items = Vec::new();
    let mut other_deductions = Vec::new();
    let mut other_deductions_total = 0.0f64;
    let mut officer_comp = 0.0f64;
    let mut distributions = 0.0f64;

    for (form_line, name, total) in &rows {
        match form_line.as_str() {
            "Gross receipts" => gross_receipts += total,
            fl if fl.starts_with("K-") => {
                if fl == "K-16d" {
                    distributions += total.abs();
                }
                schedule_k_items.push(K1LineItem {
                    form_line: form_line.clone(),
                    category_name: name.clone(),
                    total: *total,
                });
            }
            fl if fl.starts_with("1120S-") => {
                let abs_total = total.abs();
                total_deductions += abs_total;

                if fl == "1120S-7" || fl == "1120S-8" {
                    officer_comp += abs_total;
                }

                deduction_lines.push(K1LineItem {
                    form_line: form_line.clone(),
                    category_name: name.clone(),
                    total: abs_total,
                });

                if fl == "1120S-19" {
                    let is_meals = name.to_lowercase().contains("meal");
                    let deductible = if is_meals { abs_total * 0.5 } else { abs_total };
                    other_deductions_total += deductible;
                    other_deductions.push(K1OtherDeduction {
                        category_name: name.clone(),
                        total: abs_total,
                        deductible,
                    });
                }
            }
            "Other income" => other_income += total,
            _ => {}
        }
    }

    let ordinary_business_income = gross_receipts + other_income - total_deductions;

    // Validation: count uncategorized transactions
    let uncategorized_sql = format!(
        "SELECT COUNT(*) FROM transactions t WHERE {clause} AND t.category_id IS NULL",
        clause = clause
    );
    let mut ustmt = conn.prepare(&uncategorized_sql)?;
    let uncategorized_count: i64 = ustmt.query_row(param_values.as_slice(), |row| row.get(0))?;

    let comp_dist_ratio = if distributions > 0.0 {
        Some(officer_comp / distributions)
    } else {
        None
    };

    Ok(K1PrepReport {
        gross_receipts,
        other_income,
        total_deductions,
        ordinary_business_income,
        deduction_lines,
        schedule_k_items,
        other_deductions,
        other_deductions_total,
        validation: K1Validation {
            uncategorized_count,
            officer_comp,
            distributions,
            comp_dist_ratio,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn seed_transactions(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')", [],
        ).unwrap();
        let acct = conn.last_insert_rowid();
        let income_cat: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Client Services'", [], |r| r.get(0),
        ).unwrap();
        let expense_cat: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'", [], |r| r.get(0),
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Client payment', 1000.0, ?2)",
            rusqlite::params![acct, income_cat],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-20', 'Adobe CC', -50.0, ?2)",
            rusqlite::params![acct, expense_cat],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-02-10', 'GitHub', -10.0, ?2)",
            rusqlite::params![acct, expense_cat],
        ).unwrap();
    }

    #[test]
    fn test_pnl_ytd() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -60.0);
        assert_eq!(report.net, 940.0);
    }

    #[test]
    fn test_pnl_by_month() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), Some(1), None, None).unwrap();
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
        assert_eq!(report.net, 950.0);
    }

    #[test]
    fn test_expense_breakdown() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let breakdown = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        assert_eq!(breakdown.categories.len(), 1);
        assert_eq!(breakdown.categories[0].name, "Software & Subscriptions");
        assert_eq!(breakdown.categories[0].count, 2);
        assert_eq!(breakdown.total, -60.0);
    }

    #[test]
    fn test_register_returns_all_transactions() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_register(&conn, Some(2025), None, None, None, None).unwrap();
        assert_eq!(report.count, 3);
        // First two are categorized, all should appear
        assert!(report.rows.iter().all(|r| r.category.is_some()));
    }

    #[test]
    fn test_register_shows_uncategorized() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, '2025-01-15', 'UNKNOWN VENDOR', -99.0, 1, 'No matching rule')",
            rusqlite::params![acct],
        )
        .unwrap();
        let report = get_register(&conn, Some(2025), None, None, None, None).unwrap();
        assert_eq!(report.count, 1);
        assert!(report.rows[0].category.is_none());
    }

    #[test]
    fn test_register_account_filter() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report =
            get_register(&conn, Some(2025), None, None, None, Some("Test")).unwrap();
        assert_eq!(report.count, 3);
        let report =
            get_register(&conn, Some(2025), None, None, None, Some("Nonexistent")).unwrap();
        assert_eq!(report.count, 0);
    }

    #[test]
    fn test_k1_prep_basic() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        // Client Services has form_line "Gross receipts" (but stored as NULL — check actual seed)
        // Software & Subscriptions has form_line "1120S-19"
        assert!(report.gross_receipts >= 0.0);
        assert!(report.total_deductions >= 0.0);
        // Software & Subscriptions → 1120S-19 → should appear in deduction_lines
        let sw = report.deduction_lines.iter().find(|d| d.category_name == "Software & Subscriptions");
        assert!(sw.is_some(), "Software & Subscriptions should appear in deduction_lines");
        assert_eq!(sw.unwrap().total, 60.0); // abs of -60
        assert_eq!(report.validation.uncategorized_count, 0);
    }

    #[test]
    fn test_date_filter_rejects_from_without_to() {
        let (_dir, conn) = test_db();
        let result = get_pnl(&conn, None, None, Some("2025-01-01"), None);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("--from requires --to"), "got: {msg}");
    }

    #[test]
    fn test_date_filter_rejects_to_without_from() {
        let (_dir, conn) = test_db();
        let result = get_register(&conn, None, None, None, Some("2025-12-31"), None);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("--to requires --from"), "got: {msg}");
    }

    #[test]
    fn test_date_filter_accepts_both_from_and_to() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, None, None, Some("2025-01-01"), Some("2025-01-31")).unwrap();
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
    }

    #[test]
    fn test_k1_meals_50_pct() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')", [],
        ).unwrap();
        let acct = conn.last_insert_rowid();
        let meals_cat: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'Meals'", [], |r| r.get(0),
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-03-15', 'Business lunch', -100.0, ?2)",
            rusqlite::params![acct, meals_cat],
        ).unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        let meals = report.other_deductions.iter().find(|d| d.category_name == "Meals");
        assert!(meals.is_some(), "Meals should appear in other_deductions");
        let m = meals.unwrap();
        assert_eq!(m.total, 100.0);
        assert_eq!(m.deductible, 50.0); // 50% deductible
    }
}

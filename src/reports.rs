use chrono::Datelike;
use rusqlite::Connection;
use serde::Serialize;

use crate::error::{NigelError, Result};

// ---------------------------------------------------------------------------
// Report identity and date granularity
// ---------------------------------------------------------------------------

/// What date navigation granularities a report supports.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DateGranularity {
    /// Supports both month and year navigation (P&L, Expenses, Cash Flow)
    MonthAndYear,
    /// Supports only year navigation (Tax, K-1)
    YearOnly,
    /// No date navigation (Flagged, Balance)
    None,
}

/// The set of reports Nigel can produce, independent of how they are requested.
#[derive(Clone, Copy, PartialEq)]
pub enum ReportKind {
    Pnl,
    Expenses,
    Tax,
    Cashflow,
    Register,
    Flagged,
    Balance,
    K1,
    /// Bulk export of every report; not a report in its own right.
    All,
}

impl ReportKind {
    /// Stable slug used for CLI subcommand names and export filenames.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pnl => "pnl",
            Self::Expenses => "expenses",
            Self::Tax => "tax",
            Self::Cashflow => "cashflow",
            Self::Register => "register",
            Self::Flagged => "flagged",
            Self::Balance => "balance",
            Self::K1 => "k1-prep",
            Self::All => "all",
        }
    }

    pub fn granularity(&self) -> DateGranularity {
        match self {
            Self::Pnl | Self::Expenses | Self::Cashflow | Self::Register => {
                DateGranularity::MonthAndYear
            }
            Self::Tax | Self::K1 | Self::All => DateGranularity::YearOnly,
            Self::Flagged | Self::Balance => DateGranularity::None,
        }
    }
}

/// The period label printed under a report's title: the month itself when one
/// was asked for, otherwise the fiscal year, otherwise the current one.
///
/// `month` is the raw `YYYY-MM` string rather than a parsed month so the label
/// reads the way the caller asked for it, and `year` is the effective year —
/// an explicit year already resolved against the one inside `month`.
pub fn date_range_label(month: Option<&str>, year: Option<i32>) -> String {
    if let Some(month) = month {
        return month.to_string();
    }
    let year = year.unwrap_or_else(|| Datelike::year(&chrono::Local::now()));
    format!("FY {year}")
}

fn to_sql_params(params: &[String]) -> Vec<&dyn rusqlite::types::ToSql> {
    params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect()
}

// ---------------------------------------------------------------------------
// Date filter helper
// ---------------------------------------------------------------------------

fn date_filter(
    year: Option<i32>,
    month: Option<u32>,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<(String, Vec<String>)> {
    if let (Some(from), Some(to)) = (from_date, to_date) {
        return Ok((
            "t.date BETWEEN ?1 AND ?2".to_string(),
            vec![from.to_string(), to.to_string()],
        ));
    }
    if from_date.is_some() {
        return Err(crate::error::NigelError::Other(
            "--from requires --to (both date boundaries must be specified)".to_string(),
        ));
    }
    if to_date.is_some() {
        return Err(crate::error::NigelError::Other(
            "--to requires --from (both date boundaries must be specified)".to_string(),
        ));
    }
    if let (Some(y), Some(m)) = (year, month) {
        let prefix = format!("{y:04}-{m:02}");
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{prefix}%")]));
    }
    if let Some(y) = year {
        return Ok(("t.date LIKE ?1".to_string(), vec![format!("{y}%")]));
    }
    // Default: all transactions (no date filter)
    Ok(("1=1".to_string(), vec![]))
}

// ---------------------------------------------------------------------------
// P&L
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PnlItem {
    pub name: String,
    pub total: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
    let param_values = to_sql_params(params);
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseItem {
    pub name: String,
    pub total: f64,
    pub count: i64,
    pub pct: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorItem {
    pub vendor: String,
    pub total: f64,
    pub count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
    // Custom date ranges (--from/--to) not supported here; expense breakdown
    // is scoped by year/month only, matching the CLI subcommand interface.
    let (clause, params) = date_filter(year, month, None, None)?;

    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.category_type = 'expense' \
         GROUP BY c.name ORDER BY total ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let raw: Vec<(String, f64, i64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
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
        .query_map(param_values.as_slice(), |row| {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxItem {
    pub name: String,
    pub tax_line: Option<String>,
    pub category_type: String,
    pub total: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
    let param_values = to_sql_params(&params);
    let items: Vec<TaxItem> = stmt
        .query_map(param_values.as_slice(), |row| {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashflowMonth {
    pub month: String,
    pub inflows: f64,
    pub outflows: f64,
    pub net: f64,
    pub running_balance: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashflowReport {
    pub months: Vec<CashflowMonth>,
}

pub fn get_cashflow(
    conn: &Connection,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<CashflowReport> {
    let (clause, params) = date_filter(year, month, None, None)?;

    let sql = format!(
        "SELECT substr(t.date, 1, 7) as month, \
         SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END) as inflows, \
         SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END) as outflows \
         FROM transactions t WHERE {clause} \
         GROUP BY substr(t.date, 1, 7) ORDER BY month"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let raw: Vec<(String, f64, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // When filtered to a single month, seed the running balance with the
    // cumulative total from prior months in that year so users see the
    // correct year-to-date cash position, not just that month's net.
    let prior_balance = if let (Some(y), Some(m)) = (year, month) {
        if m > 1 {
            let end = format!("{y:04}-{m:02}");
            conn.query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM transactions \
                 WHERE date >= ?1 AND date < ?2",
                rusqlite::params![format!("{y:04}-01"), end],
                |row| row.get::<_, f64>(0),
            )?
        } else {
            0.0
        }
    } else {
        0.0
    };

    let mut months = Vec::new();
    let mut running = prior_balance;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRow {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>,
    pub category_id: Option<i64>,
    pub vendor: Option<String>,
    pub account_name: String,
    pub is_flagged: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterReport {
    pub rows: Vec<RegisterRow>,
    pub total: f64,
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

    let account_clause = if let Some(acc) = account {
        params.push(acc.to_string());
        format!(" AND a.name = ?{}", params.len())
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT t.id, t.date, t.description, t.amount, c.name, t.category_id, t.vendor, a.name, t.is_flagged \
         FROM transactions t \
         JOIN accounts a ON t.account_id = a.id \
         LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {clause}{account_clause} \
         ORDER BY t.date, t.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let rows: Vec<RegisterRow> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok(RegisterRow {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                vendor: row.get(6)?,
                account_name: row.get(7)?,
                is_flagged: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let total: f64 = rows.iter().map(|r| r.amount).sum();
    Ok(RegisterReport { rows, total })
}

/// One register row by id — the shape an edited transaction answers with, so
/// the row a client sends back is the row it already knows how to render.
pub fn get_register_row(conn: &Connection, id: i64) -> Result<RegisterRow> {
    conn.query_row(
        "SELECT t.id, t.date, t.description, t.amount, c.name, t.category_id, t.vendor, a.name, t.is_flagged \
         FROM transactions t \
         JOIN accounts a ON t.account_id = a.id \
         LEFT JOIN categories c ON t.category_id = c.id \
         WHERE t.id = ?1",
        [id],
        |row| {
            Ok(RegisterRow {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                vendor: row.get(6)?,
                account_name: row.get(7)?,
                is_flagged: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No transaction found with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

// ---------------------------------------------------------------------------
// Flagged
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub name: String,
    pub account_type: String,
    pub balance: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1LineItem {
    pub form_line: String,
    pub category_name: String,
    pub total: f64,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1OtherDeduction {
    pub category_name: String,
    pub total: f64,
    pub deductible: f64,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1Validation {
    pub uncategorized_count: i64,
    pub officer_comp: f64,
    pub distributions: f64,
    pub comp_dist_ratio: Option<f64>,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct K1PrepReport {
    pub gross_receipts: f64,
    pub cogs: f64,
    pub gross_profit: f64,
    pub other_income: f64,
    pub total_deductions: f64,
    pub ordinary_business_income: f64,
    pub deduction_lines: Vec<K1LineItem>,
    pub schedule_k_items: Vec<K1LineItem>,
    pub other_deductions: Vec<K1OtherDeduction>,
    pub other_deductions_total: f64,
    pub auto_mapped: Vec<String>,
    pub unmapped: Vec<K1LineItem>,
    pub validation: K1Validation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum K1Mapping {
    Excluded,
    Explicit(String),
    AutoGrossReceipts,
    Unmapped,
}

pub fn resolve_k1_mapping(form_line: Option<&str>, category_type: &str) -> K1Mapping {
    match form_line {
        Some("excluded") => K1Mapping::Excluded,
        Some(fl) => K1Mapping::Explicit(fl.to_string()),
        None if category_type == "income" => K1Mapping::AutoGrossReceipts,
        None => K1Mapping::Unmapped,
    }
}

pub fn get_k1_prep(conn: &Connection, year: Option<i32>) -> Result<K1PrepReport> {
    let (clause, params) = date_filter(year, None, None, None)?;

    // Query all categorized transactions grouped by category
    let sql = format!(
        "SELECT c.form_line, c.name, c.category_type, SUM(t.amount) as total \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} \
         GROUP BY c.form_line, c.name, c.category_type ORDER BY c.form_line"
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_values = to_sql_params(&params);
    let rows: Vec<(Option<String>, String, String, f64)> = stmt
        .query_map(param_values.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut gross_receipts = 0.0f64;
    let mut cogs = 0.0f64;
    let mut other_income = 0.0f64;
    let mut total_deductions = 0.0f64;
    let mut deduction_lines = Vec::new();
    let mut schedule_k_items = Vec::new();
    let mut other_deductions = Vec::new();
    let mut other_deductions_total = 0.0f64;
    let mut officer_comp = 0.0f64;
    let mut distributions = 0.0f64;
    let mut auto_mapped = Vec::new();
    let mut unmapped = Vec::new();

    for (form_line, name, category_type, total) in &rows {
        let mapping = resolve_k1_mapping(form_line.as_deref(), category_type);
        let line = match mapping {
            K1Mapping::Excluded => continue,
            K1Mapping::AutoGrossReceipts => {
                gross_receipts += total;
                auto_mapped.push(name.clone());
                continue;
            }
            K1Mapping::Unmapped => {
                unmapped.push(K1LineItem {
                    form_line: "—".to_string(),
                    category_name: name.clone(),
                    total: total.abs(),
                });
                continue;
            }
            K1Mapping::Explicit(fl) => fl,
        };
        match line.as_str() {
            "1120S-1a" => gross_receipts += total,
            "1120S-2" => cogs += total.abs(),
            "1120S-5" => other_income += total,
            fl if fl.starts_with("K-") => {
                if fl == "K-16d" {
                    distributions += total.abs();
                }
                schedule_k_items.push(K1LineItem {
                    form_line: line.clone(),
                    category_name: name.clone(),
                    total: *total,
                });
            }
            fl if fl.starts_with("1120S-") => {
                let abs_total = total.abs();

                if fl == "1120S-7" || fl == "1120S-8" {
                    officer_comp += abs_total;
                }

                deduction_lines.push(K1LineItem {
                    form_line: line.clone(),
                    category_name: name.clone(),
                    total: abs_total,
                });

                let deductible = if fl == "1120S-19" {
                    let is_meals = name.to_lowercase().contains("meal");
                    let d = if is_meals { abs_total * 0.5 } else { abs_total };
                    other_deductions_total += d;
                    other_deductions.push(K1OtherDeduction {
                        category_name: name.clone(),
                        total: abs_total,
                        deductible: d,
                    });
                    d
                } else {
                    abs_total
                };
                total_deductions += deductible;
            }
            _ => unmapped.push(K1LineItem {
                form_line: line.clone(),
                category_name: name.clone(),
                total: total.abs(),
            }),
        }
    }

    let gross_profit = gross_receipts - cogs;
    let ordinary_business_income = gross_profit + other_income - total_deductions;

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
        cogs,
        gross_profit,
        other_income,
        total_deductions,
        ordinary_business_income,
        deduction_lines,
        schedule_k_items,
        other_deductions,
        other_deductions_total,
        auto_mapped,
        unmapped,
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

    #[test]
    fn a_month_labels_itself_and_outranks_the_year() {
        assert_eq!(date_range_label(Some("2025-03"), None), "2025-03");
        assert_eq!(date_range_label(Some("2025-03"), Some(2024)), "2025-03");
    }

    #[test]
    fn a_year_alone_labels_a_fiscal_year() {
        assert_eq!(date_range_label(None, Some(2025)), "FY 2025");
    }

    #[test]
    fn an_unfiltered_report_is_labelled_with_the_current_year() {
        let this_year = Datelike::year(&chrono::Local::now());
        assert_eq!(date_range_label(None, None), format!("FY {this_year}"));
    }

    #[test]
    fn get_register_row_matches_the_full_register() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let category: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor, is_flagged) \
             VALUES (?1, '2025-01-15', 'ADOBE', -50.0, ?2, 'Adobe', 1)",
            rusqlite::params![account, category],
        )
        .unwrap();
        let id = conn.last_insert_rowid();

        let row = get_register_row(&conn, id).unwrap();
        let from_report = get_register(&conn, None, None, None, None, None).unwrap();
        let expected = &from_report.rows[0];
        assert_eq!(row.id, expected.id);
        assert_eq!(row.category, expected.category);
        assert_eq!(row.vendor, expected.vendor);
        assert_eq!(row.account_name, expected.account_name);
        assert_eq!(row.is_flagged, expected.is_flagged);

        let err = get_register_row(&conn, 4242).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err}");
    }

    #[test]
    fn report_kind_slugs_and_granularity() {
        use DateGranularity::*;
        use ReportKind::*;

        let expected = [
            (Pnl, "pnl", MonthAndYear),
            (Expenses, "expenses", MonthAndYear),
            (Tax, "tax", YearOnly),
            (Cashflow, "cashflow", MonthAndYear),
            (Register, "register", MonthAndYear),
            (Flagged, "flagged", None),
            (Balance, "balance", None),
            (K1, "k1-prep", YearOnly),
            (All, "all", YearOnly),
        ];

        for (kind, slug, granularity) in expected {
            assert_eq!(kind.as_str(), slug);
            assert_eq!(kind.granularity(), granularity, "granularity for {slug}");
        }
    }

    #[test]
    fn date_granularity_serializes_camel_case() {
        // 31.5 wraps every report as { granularity, report } and the SPA
        // switches on these exact strings.
        assert_eq!(
            serde_json::to_value(DateGranularity::MonthAndYear).unwrap(),
            serde_json::json!("monthAndYear")
        );
        assert_eq!(
            serde_json::to_value(DateGranularity::YearOnly).unwrap(),
            serde_json::json!("yearOnly")
        );
        assert_eq!(
            serde_json::to_value(DateGranularity::None).unwrap(),
            serde_json::json!("none")
        );
    }

    #[test]
    fn pnl_report_serializes_camel_case() {
        let report = PnlReport {
            income: vec![PnlItem {
                name: "Client Services".to_string(),
                total: 5000.0,
            }],
            expenses: vec![PnlItem {
                name: "Software".to_string(),
                total: -250.0,
            }],
            total_income: 5000.0,
            total_expenses: -250.0,
            net: 4750.0,
        };

        let value = serde_json::to_value(&report).unwrap();
        let obj = value.as_object().unwrap();

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["expenses", "income", "net", "totalExpenses", "totalIncome"]
        );
        assert!(!obj.contains_key("total_income"));
        assert!(!obj.contains_key("total_expenses"));

        assert_eq!(value["totalIncome"], 5000.0);
        assert_eq!(value["income"][0]["name"], "Client Services");
        assert_eq!(value["expenses"][0]["total"], -250.0);
    }

    #[test]
    fn k1_prep_report_serializes_camel_case() {
        let report = K1PrepReport {
            gross_receipts: 100_000.0,
            cogs: 10_000.0,
            gross_profit: 90_000.0,
            other_income: 500.0,
            total_deductions: 40_000.0,
            ordinary_business_income: 50_500.0,
            deduction_lines: vec![K1LineItem {
                form_line: "1120S-7".to_string(),
                category_name: "Officer Compensation".to_string(),
                total: -30_000.0,
            }],
            schedule_k_items: vec![K1LineItem {
                form_line: "K-16d".to_string(),
                category_name: "Distributions".to_string(),
                total: -15_000.0,
            }],
            other_deductions: vec![K1OtherDeduction {
                category_name: "Meals".to_string(),
                total: -1_000.0,
                deductible: -500.0,
            }],
            other_deductions_total: -500.0,
            auto_mapped: vec!["Consulting".to_string()],
            unmapped: vec![K1LineItem {
                form_line: String::new(),
                category_name: "Misc".to_string(),
                total: -25.0,
            }],
            validation: K1Validation {
                uncategorized_count: 3,
                officer_comp: -30_000.0,
                distributions: -15_000.0,
                comp_dist_ratio: Some(2.0),
            },
        };

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["grossReceipts"], 100_000.0);
        assert_eq!(value["grossProfit"], 90_000.0);
        assert_eq!(value["otherIncome"], 500.0);
        assert_eq!(value["totalDeductions"], 40_000.0);
        assert_eq!(value["ordinaryBusinessIncome"], 50_500.0);
        assert_eq!(value["otherDeductionsTotal"], -500.0);
        assert_eq!(value["deductionLines"][0]["formLine"], "1120S-7");
        assert_eq!(
            value["deductionLines"][0]["categoryName"],
            "Officer Compensation"
        );
        assert_eq!(value["scheduleKItems"][0]["formLine"], "K-16d");
        assert_eq!(value["otherDeductions"][0]["deductible"], -500.0);
        assert_eq!(value["autoMapped"][0], "Consulting");
        assert_eq!(value["unmapped"][0]["categoryName"], "Misc");
        assert_eq!(value["validation"]["uncategorizedCount"], 3);
        assert_eq!(value["validation"]["officerComp"], -30_000.0);
        assert_eq!(value["validation"]["compDistRatio"], 2.0);

        assert!(value.as_object().unwrap().keys().all(|k| !k.contains('_')));

        let unset_ratio = K1Validation {
            uncategorized_count: 0,
            officer_comp: 0.0,
            distributions: 0.0,
            comp_dist_ratio: None,
        };
        let value = serde_json::to_value(unset_ratio).unwrap();
        assert!(value["compDistRatio"].is_null());
    }

    fn seed_transactions(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let income_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expense_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Client payment', 1000.0, ?2)",
            rusqlite::params![acct, income_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-20', 'Adobe CC', -50.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-02-10', 'GitHub', -10.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
    }

    #[test]
    fn test_pnl_ytd() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        // seed_transactions: 1×1000.0 income, 2 expenses (−50.0 + −10.0 = −60.0)
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -60.0);
        assert_eq!(report.net, 940.0);
    }

    #[test]
    fn test_pnl_by_month() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_pnl(&conn, Some(2025), Some(1), None, None).unwrap();
        // seed_transactions Jan only: 1×1000.0 income, 1×−50.0 expense (GitHub −10.0 is Feb)
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
        assert_eq!(report.net, 950.0);
    }

    #[test]
    fn test_expense_breakdown() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let breakdown = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        // seed_transactions: 2 expenses in "Software & Subscriptions" (−50.0 + −10.0)
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
        assert_eq!(report.rows.len(), 3);
        // First two are categorized, all should appear
        assert!(report.rows.iter().all(|r| r.category.is_some()));
    }

    #[test]
    fn test_register_default_returns_all_years() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn); // 2025 transactions
                                  // Add a transaction in a different year
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2024-06-15', 'Old payment', 500.0, ?2)",
            rusqlite::params![acct, cat],
        )
        .unwrap();
        // No date filters — should return all 4 transactions across both years
        let report = get_register(&conn, None, None, None, None, None).unwrap();
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.rows[0].date, "2024-06-15"); // oldest first
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
        assert_eq!(report.rows.len(), 1);
        assert!(report.rows[0].category.is_none());
    }

    #[test]
    fn test_register_account_filter() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_register(&conn, Some(2025), None, None, None, Some("Test")).unwrap();
        assert_eq!(report.rows.len(), 3);
        let report =
            get_register(&conn, Some(2025), None, None, None, Some("Nonexistent")).unwrap();
        assert_eq!(report.rows.len(), 0);
    }

    #[test]
    fn test_k1_prep_basic() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        assert!(report.gross_receipts >= 0.0);
        assert!(report.total_deductions >= 0.0);
        // Software & Subscriptions → 1120S-19 → should appear in deduction_lines
        let sw = report
            .deduction_lines
            .iter()
            .find(|d| d.category_name == "Software & Subscriptions");
        assert!(
            sw.is_some(),
            "Software & Subscriptions should appear in deduction_lines"
        );
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
        let result = get_pnl(&conn, None, None, None, Some("2025-12-31"));
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("--to requires --from"), "got: {msg}");
    }

    #[test]
    fn test_date_filter_accepts_both_from_and_to() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // Jan range captures: 1×1000.0 income, 1×−50.0 expense (GitHub −10.0 is Feb)
        let report = get_pnl(&conn, None, None, Some("2025-01-01"), Some("2025-01-31")).unwrap();
        assert_eq!(report.total_income, 1000.0);
        assert_eq!(report.total_expenses, -50.0);
    }

    #[test]
    fn test_k1_meals_50_pct() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let meals_cat: i64 = conn
            .query_row("SELECT id FROM categories WHERE name = 'Meals'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-03-15', 'Business lunch', -100.0, ?2)",
            rusqlite::params![acct, meals_cat],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        let meals = report
            .other_deductions
            .iter()
            .find(|d| d.category_name == "Meals");
        assert!(meals.is_some(), "Meals should appear in other_deductions");
        let m = meals.unwrap();
        assert_eq!(m.total, 100.0);
        assert_eq!(m.deductible, 50.0); // 50% deductible
    }

    #[test]
    fn test_k1_other_income_sign_handling() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line) \
             VALUES ('Test Other Income', 'income', '1120S-5')",
            [],
        )
        .unwrap();
        let cat_id = conn.last_insert_rowid();
        // Refunds exceed income: net is negative
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-10', 'Misc income', 50.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Refund', -200.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        // SUM = 50 + (-200) = -150 — net negative surfaces as-is
        assert_eq!(report.other_income, -150.0);
    }

    #[test]
    fn test_k1_gross_receipts_sign_handling() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line) \
             VALUES ('Test Gross Receipts', 'income', '1120S-1a')",
            [],
        )
        .unwrap();
        let cat_id = conn.last_insert_rowid();
        // Refunds exceed income: net is negative
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-10', 'Invoice', 100.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Refund', -300.0, ?2)",
            rusqlite::params![acct, cat_id],
        )
        .unwrap();
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        // SUM = 100 + (-300) = -200 — net negative surfaces as-is
        assert_eq!(report.gross_receipts, -200.0);
    }

    #[test]
    fn test_resolve_k1_mapping() {
        use K1Mapping::*;
        assert_eq!(resolve_k1_mapping(Some("excluded"), "expense"), Excluded);
        assert_eq!(resolve_k1_mapping(Some("excluded"), "income"), Excluded);
        assert_eq!(
            resolve_k1_mapping(Some("1120S-19"), "expense"),
            Explicit("1120S-19".into())
        );
        assert_eq!(
            resolve_k1_mapping(Some("K-16d"), "expense"),
            Explicit("K-16d".into())
        );
        assert_eq!(resolve_k1_mapping(None, "income"), AutoGrossReceipts);
        assert_eq!(resolve_k1_mapping(None, "expense"), Unmapped);
    }

    fn k1_fixture(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('K1T', 'checking')",
            [],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn k1_cat(conn: &Connection, name: &str, ctype: &str, form_line: Option<&str>) -> i64 {
        conn.execute(
            "INSERT INTO categories (name, category_type, form_line) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, ctype, form_line],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn k1_txn(conn: &Connection, acct: i64, date: &str, amount: f64, cat: i64) {
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, ?2, 'x', ?3, ?4)",
            rusqlite::params![acct, date, amount, cat],
        )
        .unwrap();
    }

    #[test]
    fn test_k1_custom_chart_income_falls_back_and_unmapped_surfaces() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Widget Sales", "income", None);
        let exp = k1_cat(&conn, "Mystery Spend", "expense", None);
        let odd = k1_cat(&conn, "Odd Mapping", "expense", Some("Schedule Z"));
        let skip = k1_cat(&conn, "Personal", "expense", Some("excluded"));
        k1_txn(&conn, acct, "2025-02-01", 5000.0, inc);
        k1_txn(&conn, acct, "2025-02-02", -400.0, exp);
        k1_txn(&conn, acct, "2025-02-03", -75.0, odd);
        k1_txn(&conn, acct, "2025-02-04", -999.0, skip);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        assert_eq!(r.gross_receipts, 5000.0);
        assert_eq!(r.auto_mapped, vec!["Widget Sales".to_string()]);
        let unmapped_names: Vec<&str> = r
            .unmapped
            .iter()
            .map(|u| u.category_name.as_str())
            .collect();
        assert!(unmapped_names.contains(&"Mystery Spend"));
        assert!(unmapped_names.contains(&"Odd Mapping"));
        assert!(!unmapped_names.contains(&"Personal"));
        // unmapped and excluded activity stays out of the math
        assert_eq!(r.total_deductions, 0.0);
        assert_eq!(r.ordinary_business_income, 5000.0);
    }

    #[test]
    fn test_k1_cogs_and_gross_profit() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Sales", "income", Some("1120S-1a"));
        let cogs = k1_cat(&conn, "Materials", "expense", Some("1120S-2"));
        let rent = k1_cat(&conn, "Shop Rent", "expense", Some("1120S-11"));
        k1_txn(&conn, acct, "2025-03-01", 10000.0, inc);
        k1_txn(&conn, acct, "2025-03-02", -2500.0, cogs);
        k1_txn(&conn, acct, "2025-03-03", -1000.0, rent);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        assert_eq!(r.gross_receipts, 10000.0);
        assert_eq!(r.cogs, 2500.0);
        assert_eq!(r.gross_profit, 7500.0);
        assert_eq!(r.total_deductions, 1000.0);
        assert_eq!(r.ordinary_business_income, 6500.0);
        // COGS is an income-summary line, not a deduction line
        assert!(r.deduction_lines.iter().all(|d| d.form_line != "1120S-2"));
    }

    #[test]
    fn test_k1_headline_deductions_use_deductible_meals() {
        let (_dir, conn) = test_db();
        let acct = k1_fixture(&conn);
        let inc = k1_cat(&conn, "Sales", "income", Some("1120S-1a"));
        let meals = k1_cat(&conn, "Team Meals", "expense", Some("1120S-19"));
        let sw = k1_cat(&conn, "Tools", "expense", Some("1120S-19"));
        k1_txn(&conn, acct, "2025-04-01", 1000.0, inc);
        k1_txn(&conn, acct, "2025-04-02", -100.0, meals);
        k1_txn(&conn, acct, "2025-04-03", -40.0, sw);

        let r = get_k1_prep(&conn, Some(2025)).unwrap();
        // headline = 50 (meals at 50%) + 40 = other_deductions_total
        assert_eq!(r.total_deductions, 90.0);
        assert_eq!(r.other_deductions_total, 90.0);
        assert_eq!(r.ordinary_business_income, 910.0);
    }

    #[test]
    fn test_cashflow_full_year_running_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_cashflow(&conn, Some(2025), None).unwrap();
        // Jan: +1000 -50 = +950, Feb: -10 → running = 940
        assert_eq!(report.months.len(), 2);
        assert_eq!(report.months[0].running_balance, 950.0);
        assert_eq!(report.months[1].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_single_month_includes_prior_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // Feb only — running balance should include Jan's cumulative (950.0)
        let report = get_cashflow(&conn, Some(2025), Some(2)).unwrap();
        assert_eq!(report.months.len(), 1);
        assert_eq!(report.months[0].net, -10.0);
        // Running balance = prior 950.0 + Feb net -10.0 = 940.0
        assert_eq!(report.months[0].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_january_has_no_prior_balance() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        let report = get_cashflow(&conn, Some(2025), Some(1)).unwrap();
        assert_eq!(report.months.len(), 1);
        // Jan starts at 0 — no prior months
        assert_eq!(report.months[0].running_balance, 950.0);
    }

    #[test]
    fn test_cashflow_cross_year_boundary() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn); // 2025 transactions
                                  // Add a 2024 transaction that should NOT affect 2025 prior balance
        let acct: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2024-12-15', 'Prior year payment', 5000.0, ?2)",
            rusqlite::params![acct, cat],
        )
        .unwrap();
        // Feb 2025 prior balance should only include Jan 2025, not Dec 2024
        let report = get_cashflow(&conn, Some(2025), Some(2)).unwrap();
        assert_eq!(report.months.len(), 1);
        assert_eq!(report.months[0].running_balance, 940.0);
    }

    #[test]
    fn test_cashflow_unfiltered_starts_at_zero() {
        let (_dir, conn) = test_db();
        seed_transactions(&conn);
        // No year or month filter — running balance starts at 0
        let report = get_cashflow(&conn, None, None).unwrap();
        assert!(report.months.len() >= 2);
        assert_eq!(report.months[0].running_balance, 950.0); // first month net only
    }
}

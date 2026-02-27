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
) -> (String, Vec<String>) {
    if let (Some(from), Some(to)) = (from_date, to_date) {
        return (
            "t.date BETWEEN ?1 AND ?2".to_string(),
            vec![from.to_string(), to.to_string()],
        );
    }
    if let (Some(y), Some(m)) = (year, month) {
        let prefix = format!("{y:04}-{m:02}");
        return ("t.date LIKE ?1".to_string(), vec![format!("{prefix}%")]);
    }
    if let Some(y) = year {
        return ("t.date LIKE ?1".to_string(), vec![format!("{y}%")]);
    }
    // Default: current year
    let current_year = chrono::Local::now().year();
    ("t.date LIKE ?1".to_string(), vec![format!("{current_year}%")])
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
    let (clause, params) = date_filter(year, month, from_date, to_date);

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
    Ok(rows.filter_map(|r| r.ok()).collect())
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
    let (clause, params) = date_filter(year, month, None, None);

    let sql = format!(
        "SELECT c.name, SUM(t.amount) as total, COUNT(*) as count \
         FROM transactions t JOIN categories c ON t.category_id = c.id \
         WHERE {clause} AND c.category_type = 'expense' \
         GROUP BY c.name ORDER BY total ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let raw: Vec<(String, f64, i64)> = stmt
        .query_map([&params[0]], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

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
        .filter_map(|r| r.ok())
        .collect();

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
    let (clause, params) = date_filter(year, None, None, None);

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
        .filter_map(|r| r.ok())
        .collect();

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
    let (clause, params) = date_filter(year, month, None, None);

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
        .filter_map(|r| r.ok())
        .collect();

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
        .filter_map(|r| r.ok())
        .collect();
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
        .filter_map(|r| r.ok())
        .collect();

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
}

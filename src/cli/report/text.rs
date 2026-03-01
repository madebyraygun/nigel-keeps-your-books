use colored::Colorize;
use comfy_table::{Cell, Table};

use crate::cli::parse_month_opt;
use crate::db::{get_connection, get_metadata};
use crate::error::Result;
use crate::fmt::money;
use crate::reports;
use crate::settings::get_data_dir;

/// Prepend company name as a header line if non-empty.
fn with_header(company_name: &str, body: String) -> String {
    if company_name.is_empty() {
        body
    } else {
        format!("{company_name}\n{body}")
    }
}

// ---------------------------------------------------------------------------
// Data-fetching + formatting wrappers (used by dispatch)
// ---------------------------------------------------------------------------

pub fn pnl(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let data = reports::get_pnl(&conn, y, mm, from_date.as_deref(), to_date.as_deref())?;
    Ok(with_header(&company, format_pnl(&data)))
}

pub fn expenses(month: Option<String>, year: Option<i32>) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_expense_breakdown(&conn, year.or(my), mm)?;
    Ok(with_header(&company, format_expenses(&data)))
}

pub fn tax(year: Option<i32>) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let data = reports::get_tax_summary(&conn, year)?;
    Ok(with_header(&company, format_tax(&data)))
}

pub fn cashflow(month: Option<String>, year: Option<i32>) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_cashflow(&conn, year.or(my), mm)?;
    Ok(with_header(&company, format_cashflow(&data)))
}

pub fn register(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    account: Option<String>,
) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let data = reports::get_register(
        &conn, y, mm,
        from_date.as_deref(), to_date.as_deref(), account.as_deref(),
    )?;
    Ok(with_header(&company, format_register(&data)))
}

pub fn flagged() -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let rows = reports::get_flagged(&conn)?;
    Ok(with_header(&company, format_flagged(&rows)))
}

pub fn balance() -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let data = reports::get_balance(&conn)?;
    Ok(with_header(&company, format_balance(&data)))
}

pub fn k1(year: Option<i32>) -> Result<String> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let company = get_metadata(&conn, "company_name").unwrap_or_default();
    let data = reports::get_k1_prep(&conn, year)?;
    Ok(with_header(&company, format_k1(&data)))
}

// ---------------------------------------------------------------------------
// Pure formatting functions (report data → String)
// ---------------------------------------------------------------------------

pub fn format_pnl(pnl: &reports::PnlReport) -> String {
    let mut table = Table::new();
    table.set_header(vec!["Category", "Amount"]);

    if !pnl.income.is_empty() {
        table.add_row(vec![Cell::new("INCOME".green().bold()), Cell::new("")]);
        for item in &pnl.income {
            table.add_row(vec![
                Cell::new(format!("  {}", item.name)),
                Cell::new(money(item.total)),
            ]);
        }
        table.add_row(vec![
            Cell::new("Total Income".bold()),
            Cell::new(money(pnl.total_income)),
        ]);
        table.add_row(vec![Cell::new(""), Cell::new("")]);
    }

    if !pnl.expenses.is_empty() {
        table.add_row(vec![Cell::new("EXPENSES".red().bold()), Cell::new("")]);
        for item in &pnl.expenses {
            table.add_row(vec![
                Cell::new(format!("  {}", item.name)),
                Cell::new(money(item.total.abs())),
            ]);
        }
        table.add_row(vec![
            Cell::new("Total Expenses".bold()),
            Cell::new(money(pnl.total_expenses.abs())),
        ]);
        table.add_row(vec![Cell::new(""), Cell::new("")]);
    }

    let net_label = if pnl.net >= 0.0 {
        "NET".green().bold()
    } else {
        "NET".red().bold()
    };
    table.add_row(vec![Cell::new(net_label), Cell::new(money(pnl.net))]);

    format!("Profit & Loss\n{table}")
}

pub fn format_expenses(data: &reports::ExpenseBreakdown) -> String {
    let mut table = Table::new();
    table.set_header(vec!["Category", "Amount", "%", "Count"]);
    for item in &data.categories {
        table.add_row(vec![
            Cell::new(&item.name),
            Cell::new(money(item.total.abs())),
            Cell::new(format!("{:.1}%", item.pct)),
            Cell::new(item.count),
        ]);
    }
    table.add_row(vec![
        Cell::new("Total".bold()),
        Cell::new(money(data.total.abs())),
        Cell::new(""),
        Cell::new(""),
    ]);
    let mut out = format!("Expense Breakdown\n{table}");

    if !data.top_vendors.is_empty() {
        let mut vtable = Table::new();
        vtable.set_header(vec!["Vendor", "Amount", "Count"]);
        for v in &data.top_vendors {
            vtable.add_row(vec![
                Cell::new(&v.vendor),
                Cell::new(money(v.total.abs())),
                Cell::new(v.count),
            ]);
        }
        out.push_str(&format!("\n\nTop Vendors\n{vtable}"));
    }
    out
}

pub fn format_tax(data: &reports::TaxSummary) -> String {
    let mut table = Table::new();
    table.set_header(vec!["Category", "Tax Line", "Type", "Amount"]);
    for item in &data.line_items {
        table.add_row(vec![
            Cell::new(&item.name),
            Cell::new(item.tax_line.as_deref().unwrap_or("")),
            Cell::new(&item.category_type),
            Cell::new(money(item.total.abs())),
        ]);
    }
    format!("Tax Summary\n{table}")
}

pub fn format_cashflow(data: &reports::CashflowReport) -> String {
    let mut table = Table::new();
    table.set_header(vec!["Month", "Inflows", "Outflows", "Net", "Running"]);
    for m in &data.months {
        let net_str = if m.net >= 0.0 {
            money(m.net).green().to_string()
        } else {
            money(m.net).red().to_string()
        };
        table.add_row(vec![
            Cell::new(&m.month),
            Cell::new(money(m.inflows)),
            Cell::new(money(m.outflows.abs())),
            Cell::new(net_str),
            Cell::new(money(m.running_balance)),
        ]);
    }
    format!("Cash Flow\n{table}")
}

pub fn format_register(data: &reports::RegisterReport) -> String {
    if data.rows.is_empty() {
        return "No transactions found.".to_string();
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Date", "Description", "Amount", "Category", "Vendor", "Account"]);
    for r in &data.rows {
        let amt = if r.amount < 0.0 {
            money(r.amount.abs()).red().to_string()
        } else {
            money(r.amount).green().to_string()
        };
        let cat = r.category.as_deref().unwrap_or("\u{2014}");
        let vendor = r.vendor.as_deref().unwrap_or("");
        table.add_row(vec![
            Cell::new(r.id),
            Cell::new(&r.date),
            Cell::new(&r.description),
            Cell::new(amt),
            Cell::new(cat),
            Cell::new(vendor),
            Cell::new(&r.account_name),
        ]);
    }
    format!(
        "Transaction Register ({} transactions, net: {})\n{table}",
        data.rows.len(),
        money(data.total)
    )
}

pub fn format_flagged(rows: &[reports::FlaggedTransaction]) -> String {
    if rows.is_empty() {
        return "No flagged transactions.".to_string();
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Date", "Description", "Amount", "Account"]);
    for r in rows {
        let amt = if r.amount < 0.0 {
            money(r.amount.abs()).red().to_string()
        } else {
            money(r.amount.abs()).green().to_string()
        };
        table.add_row(vec![
            Cell::new(r.id),
            Cell::new(&r.date),
            Cell::new(&r.description),
            Cell::new(amt),
            Cell::new(&r.account_name),
        ]);
    }
    format!("Flagged Transactions ({})\n{table}", rows.len())
}

pub fn format_balance(data: &reports::BalanceReport) -> String {
    let mut table = Table::new();
    table.set_header(vec!["Account", "Type", "Balance"]);
    for a in &data.accounts {
        let bal = if a.balance >= 0.0 {
            money(a.balance).green().to_string()
        } else {
            money(a.balance).red().to_string()
        };
        table.add_row(vec![
            Cell::new(&a.name),
            Cell::new(&a.account_type),
            Cell::new(bal),
        ]);
    }
    table.add_row(vec![
        Cell::new("Total".bold()),
        Cell::new(""),
        Cell::new(money(data.total)),
    ]);
    format!("Cash Position\n{table}\n\nYTD Net Income: {}", money(data.ytd_net_income))
}

pub fn format_k1(data: &reports::K1PrepReport) -> String {
    let mut out = String::new();

    // 1. Income Summary
    let mut summary = Table::new();
    summary.set_header(vec!["Item", "Amount"]);
    summary.add_row(vec![Cell::new("Gross Receipts"), Cell::new(money(data.gross_receipts))]);
    summary.add_row(vec![Cell::new("Other Income"), Cell::new(money(data.other_income))]);
    summary.add_row(vec![Cell::new("Total Deductions"), Cell::new(money(data.total_deductions))]);
    let obi_label = if data.ordinary_business_income >= 0.0 {
        "Ordinary Business Income".green().bold().to_string()
    } else {
        "Ordinary Business Loss".red().bold().to_string()
    };
    summary.add_row(vec![Cell::new(obi_label), Cell::new(money(data.ordinary_business_income))]);
    out.push_str(&format!("K-1 Preparation Worksheet (Form 1120-S)\n\nIncome Summary\n{summary}"));

    // 2. Deductions by Line
    if !data.deduction_lines.is_empty() {
        let mut ded = Table::new();
        ded.set_header(vec!["Line", "Category", "Amount"]);
        for item in &data.deduction_lines {
            ded.add_row(vec![
                Cell::new(&item.form_line),
                Cell::new(&item.category_name),
                Cell::new(money(item.total)),
            ]);
        }
        out.push_str(&format!("\n\nDeductions by Line\n{ded}"));
    }

    // 3. Schedule K Items
    if !data.schedule_k_items.is_empty() {
        let mut sk = Table::new();
        sk.set_header(vec!["Line", "Item", "Amount"]);
        for item in &data.schedule_k_items {
            sk.add_row(vec![
                Cell::new(&item.form_line),
                Cell::new(&item.category_name),
                Cell::new(money(item.total.abs())),
            ]);
        }
        out.push_str(&format!("\n\nSchedule K\n{sk}"));
    }

    // 4. Line 19 Detail (Other Deductions)
    if !data.other_deductions.is_empty() {
        let mut od = Table::new();
        od.set_header(vec!["Category", "Full Amount", "Deductible"]);
        for item in &data.other_deductions {
            let note = if item.deductible < item.total { " (50%)" } else { "" };
            od.add_row(vec![
                Cell::new(format!("{}{}", item.category_name, note)),
                Cell::new(money(item.total)),
                Cell::new(money(item.deductible)),
            ]);
        }
        od.add_row(vec![
            Cell::new("Total Other Deductions".bold()),
            Cell::new(""),
            Cell::new(money(data.other_deductions_total)),
        ]);
        out.push_str(&format!("\n\nLine 19 \u{2014} Other Deductions\n{od}"));
    }

    // 5. Validation warnings
    if data.validation.uncategorized_count > 0 {
        out.push_str(&format!(
            "\n{}",
            format!(
                "Warning: {} uncategorized transactions \u{2014} run `nigel review` before filing",
                data.validation.uncategorized_count
            )
            .yellow()
        ));
    }
    if let Some(ratio) = data.validation.comp_dist_ratio {
        if ratio < 1.0 {
            out.push_str(&format!(
                "\n{}",
                format!(
                    "Warning: Officer compensation ({}) is less than distributions ({}) \u{2014} review reasonable comp",
                    money(data.validation.officer_comp),
                    money(data.validation.distributions)
                )
                .yellow()
            ));
        }
    }

    out
}

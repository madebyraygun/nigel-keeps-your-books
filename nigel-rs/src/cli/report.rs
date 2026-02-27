use colored::Colorize;
use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reports;
use crate::settings::get_data_dir;

fn parse_month_opt(month: &Option<String>) -> (Option<i32>, Option<u32>) {
    if let Some(m) = month {
        let parts: Vec<&str> = m.split('-').collect();
        if parts.len() == 2 {
            let year = parts[0].parse().ok();
            let month = parts[1].parse().ok();
            return (year, month);
        }
    }
    (None, None)
}

pub fn pnl(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let m = mm;
    let pnl = reports::get_pnl(&conn, y, m, from_date.as_deref(), to_date.as_deref())?;

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

    println!("Profit & Loss\n{table}");
    Ok(())
}

pub fn expenses(month: Option<String>, year: Option<i32>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_expense_breakdown(&conn, year.or(my), mm)?;

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
    println!("Expense Breakdown\n{table}");

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
        println!("\nTop Vendors\n{vtable}");
    }
    Ok(())
}

pub fn tax(year: Option<i32>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_tax_summary(&conn, year)?;

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
    println!("Tax Summary\n{table}");
    Ok(())
}

pub fn cashflow(month: Option<String>, year: Option<i32>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let data = reports::get_cashflow(&conn, year.or(my), mm)?;

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
    println!("Cash Flow\n{table}");
    Ok(())
}

pub fn flagged() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let rows = reports::get_flagged(&conn)?;

    if rows.is_empty() {
        println!("No flagged transactions.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Date", "Description", "Amount", "Account"]);
    for r in &rows {
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
    println!("Flagged Transactions ({})\n{table}", rows.len());
    Ok(())
}

pub fn balance() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let data = reports::get_balance(&conn)?;

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
    println!("Cash Position\n{table}");
    println!("\nYTD Net Income: {}", money(data.ytd_net_income));
    Ok(())
}

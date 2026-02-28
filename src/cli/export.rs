use std::path::PathBuf;

use chrono::Datelike;

use super::parse_month_opt;
use crate::db::get_connection;
use crate::error::Result;
use crate::pdf;
use crate::reports;
use crate::settings::{get_data_dir, load_settings};

fn date_range_label(month: &Option<String>, year: &Option<i32>) -> String {
    if let Some(m) = month {
        return m.clone();
    }
    if let Some(y) = year {
        return format!("FY {y}");
    }
    let y = chrono::Local::now().year();
    format!("FY {y}")
}

fn default_path(name: &str) -> PathBuf {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    get_data_dir().join("exports").join(format!("{name}-{date}.pdf"))
}

fn write_pdf(bytes: &[u8], path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    println!("Wrote {}", path.display());
    Ok(())
}

pub fn pnl(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let report = reports::get_pnl(&conn, y, mm, from_date.as_deref(), to_date.as_deref())?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my).map(|v| v));
    let bytes = pdf::render_pnl(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("pnl"));
    write_pdf(&bytes, &path)
}

pub fn expenses(month: Option<String>, year: Option<i32>, output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let report = reports::get_expense_breakdown(&conn, year.or(my), mm)?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my).map(|v| v));
    let bytes = pdf::render_expenses(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("expenses"));
    write_pdf(&bytes, &path)
}

pub fn tax(year: Option<i32>, output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let report = reports::get_tax_summary(&conn, year)?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let bytes = pdf::render_tax(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("tax"));
    write_pdf(&bytes, &path)
}

pub fn cashflow(month: Option<String>, year: Option<i32>, output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let report = reports::get_cashflow(&conn, year.or(my), mm)?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my).map(|v| v));
    let bytes = pdf::render_cashflow(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("cashflow"));
    write_pdf(&bytes, &path)
}

pub fn register(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    account: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let report = reports::get_register(
        &conn,
        y,
        mm,
        from_date.as_deref(),
        to_date.as_deref(),
        account.as_deref(),
    )?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my).map(|v| v));
    let bytes = pdf::render_register(&report, &company, &range)?;
    let path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_path("register"));
    write_pdf(&bytes, &path)
}

pub fn flagged(output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let rows = reports::get_flagged(&conn)?;
    let company = load_settings().company_name;
    let bytes = pdf::render_flagged(&rows, &company)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("flagged"));
    write_pdf(&bytes, &path)
}

pub fn balance(output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let report = reports::get_balance(&conn)?;
    let company = load_settings().company_name;
    let bytes = pdf::render_balance(&report, &company)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("balance"));
    write_pdf(&bytes, &path)
}

pub fn k1(year: Option<i32>, output: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let report = reports::get_k1_prep(&conn, year)?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let bytes = pdf::render_k1(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("k1-prep"));
    write_pdf(&bytes, &path)
}

pub fn all(year: Option<i32>, output_dir: Option<String>) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("exports"));
    std::fs::create_dir_all(&dir)?;

    let path = |name: &str| dir.join(format!("{name}-{date}.pdf"));

    let report = reports::get_pnl(&conn, year, None, None, None)?;
    write_pdf(&pdf::render_pnl(&report, &company, &range)?, &path("pnl"))?;

    let report = reports::get_expense_breakdown(&conn, year, None)?;
    write_pdf(
        &pdf::render_expenses(&report, &company, &range)?,
        &path("expenses"),
    )?;

    let report = reports::get_tax_summary(&conn, year)?;
    write_pdf(&pdf::render_tax(&report, &company, &range)?, &path("tax"))?;

    let report = reports::get_cashflow(&conn, year, None)?;
    write_pdf(
        &pdf::render_cashflow(&report, &company, &range)?,
        &path("cashflow"),
    )?;

    let register = reports::get_register(&conn, year, None, None, None, None)?;
    write_pdf(
        &pdf::render_register(&register, &company, &range)?,
        &path("register"),
    )?;

    let rows = reports::get_flagged(&conn)?;
    write_pdf(&pdf::render_flagged(&rows, &company)?, &path("flagged"))?;

    let report = reports::get_balance(&conn)?;
    write_pdf(
        &pdf::render_balance(&report, &company)?,
        &path("balance"),
    )?;

    let report = reports::get_k1_prep(&conn, year)?;
    write_pdf(
        &pdf::render_k1(&report, &company, &range)?,
        &path("k1-prep"),
    )?;

    Ok(())
}

#[cfg(feature = "pdf")]
use std::path::PathBuf;

#[cfg(feature = "pdf")]
use crate::cli::parse_month_opt;
#[cfg(feature = "pdf")]
use crate::cli::ReportCommands;
#[cfg(feature = "pdf")]
use crate::error::Result;
#[cfg(feature = "pdf")]
use crate::settings::{get_data_dir, load_settings};

#[cfg(feature = "pdf")]
fn date_range_label(month: &Option<String>, year: &Option<i32>) -> String {
    if let Some(m) = month {
        return m.clone();
    }
    if let Some(y) = year {
        return format!("FY {y}");
    }
    let y = chrono::Datelike::year(&chrono::Local::now());
    format!("FY {y}")
}

#[cfg(feature = "pdf")]
fn default_path(name: &str) -> PathBuf {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    get_data_dir().join("exports").join(format!("{name}-{date}.pdf"))
}

#[cfg(feature = "pdf")]
fn write_pdf(bytes: &[u8], path: &PathBuf) -> Result<String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    let display = format!("{}", path.display());
    println!("Wrote {display}");
    Ok(display)
}

/// Dispatch a report command as PDF export. Returns the written path(s).
#[cfg(feature = "pdf")]
pub fn dispatch_pdf(cmd: ReportCommands, output: Option<String>) -> Result<String> {
    match cmd {
        ReportCommands::Pnl { month, year, from_date, to_date, .. } => {
            pnl(month, year, from_date, to_date, output)
        }
        ReportCommands::Expenses { month, year, .. } => expenses(month, year, output),
        ReportCommands::Tax { year, .. } => tax(year, output),
        ReportCommands::Cashflow { month, year, .. } => cashflow(month, year, output),
        ReportCommands::Register { month, year, from_date, to_date, account, .. } => {
            register(month, year, from_date, to_date, account, output)
        }
        ReportCommands::Flagged { .. } => flagged(output),
        ReportCommands::Balance { .. } => balance(output),
        ReportCommands::K1 { year, .. } => k1(year, output),
        ReportCommands::All { year, output_dir, .. } => all(year, output_dir),
    }
}

#[cfg(feature = "pdf")]
pub fn pnl(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    output: Option<String>,
) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let report = crate::reports::get_pnl(&conn, y, mm, from_date.as_deref(), to_date.as_deref())?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my));
    let bytes = crate::pdf::render_pnl(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("pnl"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn expenses(month: Option<String>, year: Option<i32>, output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let report = crate::reports::get_expense_breakdown(&conn, year.or(my), mm)?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my));
    let bytes = crate::pdf::render_expenses(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("expenses"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn tax(year: Option<i32>, output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let report = crate::reports::get_tax_summary(&conn, year)?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let bytes = crate::pdf::render_tax(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("tax"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn cashflow(month: Option<String>, year: Option<i32>, output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let report = crate::reports::get_cashflow(&conn, year.or(my), mm)?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my));
    let bytes = crate::pdf::render_cashflow(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("cashflow"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn register(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    account: Option<String>,
    output: Option<String>,
) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let report = crate::reports::get_register(
        &conn, y, mm,
        from_date.as_deref(), to_date.as_deref(), account.as_deref(),
    )?;
    let company = load_settings().company_name;
    let range = date_range_label(&month, &year.or(my));
    let bytes = crate::pdf::render_register(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("register"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn flagged(output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let rows = crate::reports::get_flagged(&conn)?;
    let company = load_settings().company_name;
    let bytes = crate::pdf::render_flagged(&rows, &company)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("flagged"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn balance(output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let report = crate::reports::get_balance(&conn)?;
    let company = load_settings().company_name;
    let bytes = crate::pdf::render_balance(&report, &company)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("balance"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn k1(year: Option<i32>, output: Option<String>) -> Result<String> {
    let conn = crate::db::get_connection(&get_data_dir().join("nigel.db"))?;
    let report = crate::reports::get_k1_prep(&conn, year)?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let bytes = crate::pdf::render_k1(&report, &company, &range)?;
    let path = output.map(PathBuf::from).unwrap_or_else(|| default_path("k1-prep"));
    write_pdf(&bytes, &path)
}

#[cfg(feature = "pdf")]
pub fn all(year: Option<i32>, output_dir: Option<String>) -> Result<String> {
    let data_dir = get_data_dir();
    let conn = crate::db::get_connection(&data_dir.join("nigel.db"))?;
    let company = load_settings().company_name;
    let range = date_range_label(&None, &year);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("exports"));
    std::fs::create_dir_all(&dir)?;

    let path = |name: &str| dir.join(format!("{name}-{date}.pdf"));

    let report = crate::reports::get_pnl(&conn, year, None, None, None)?;
    write_pdf(&crate::pdf::render_pnl(&report, &company, &range)?, &path("pnl"))?;

    let report = crate::reports::get_expense_breakdown(&conn, year, None)?;
    write_pdf(&crate::pdf::render_expenses(&report, &company, &range)?, &path("expenses"))?;

    let report = crate::reports::get_tax_summary(&conn, year)?;
    write_pdf(&crate::pdf::render_tax(&report, &company, &range)?, &path("tax"))?;

    let report = crate::reports::get_cashflow(&conn, year, None)?;
    write_pdf(&crate::pdf::render_cashflow(&report, &company, &range)?, &path("cashflow"))?;

    let register = crate::reports::get_register(&conn, year, None, None, None, None)?;
    write_pdf(&crate::pdf::render_register(&register, &company, &range)?, &path("register"))?;

    let rows = crate::reports::get_flagged(&conn)?;
    write_pdf(&crate::pdf::render_flagged(&rows, &company)?, &path("flagged"))?;

    let report = crate::reports::get_balance(&conn)?;
    write_pdf(&crate::pdf::render_balance(&report, &company)?, &path("balance"))?;

    let report = crate::reports::get_k1_prep(&conn, year)?;
    write_pdf(&crate::pdf::render_k1(&report, &company, &range)?, &path("k1-prep"))?;

    Ok(format!("All reports exported to {}", dir.display()))
}

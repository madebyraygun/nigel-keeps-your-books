pub mod text;
pub mod view;

use std::io::IsTerminal;
use std::path::PathBuf;

use crate::cli::ReportOutputArgs;
use crate::error::Result;

use super::ReportCommands;

pub fn dispatch(cmd: ReportCommands) -> Result<()> {
    let args = cmd.output_args();

    // `report all` is always an export
    if matches!(cmd, ReportCommands::All { .. }) {
        return dispatch_export(cmd, args);
    }

    if args.output.is_some() || args.mode.as_deref() == Some("export") {
        dispatch_export(cmd, args)
    } else if std::io::stdout().is_terminal() {
        dispatch_view(cmd)
    } else {
        // Non-TTY: plain text to stdout
        let s = dispatch_text(&cmd)?;
        println!("{s}");
        Ok(())
    }
}

/// Interactive ratatui view (Phase 2 — currently falls back to text)
fn dispatch_view(cmd: ReportCommands) -> Result<()> {
    view::dispatch(cmd)
}

pub(crate) fn dispatch_text(cmd: &ReportCommands) -> Result<String> {
    match cmd {
        ReportCommands::Pnl { month, year, from_date, to_date, .. } => {
            text::pnl(month.clone(), *year, from_date.clone(), to_date.clone())
        }
        ReportCommands::Expenses { month, year, .. } => text::expenses(month.clone(), *year),
        ReportCommands::Tax { year, .. } => text::tax(*year),
        ReportCommands::Cashflow { month, year, .. } => text::cashflow(month.clone(), *year),
        ReportCommands::Register { month, year, from_date, to_date, account, .. } => {
            text::register(month.clone(), *year, from_date.clone(), to_date.clone(), account.clone())
        }
        ReportCommands::Flagged { .. } => text::flagged(),
        ReportCommands::Balance { .. } => text::balance(),
        ReportCommands::K1 { year, .. } => text::k1(*year),
        ReportCommands::All { .. } => {
            Err(crate::error::NigelError::Other("`report all` is export-only".into()))
        }
    }
}

fn dispatch_export(cmd: ReportCommands, args: ReportOutputArgs) -> Result<()> {
    let is_text = args.format.as_deref() == Some("text");

    if is_text {
        return export_text(cmd, args.output);
    }

    // PDF export
    dispatch_pdf_export(cmd, args.output)
}

fn export_text(cmd: ReportCommands, output: Option<String>) -> Result<()> {
    if let ReportCommands::All { year, output_dir, .. } = cmd {
        return export_all_text(year, output_dir);
    }

    let name = cmd.report_name();
    let s = dispatch_text(&cmd)?;
    let path = output.unwrap_or_else(|| default_text_path(name));
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, &s)?;
    println!("Wrote {}", p.display());
    Ok(())
}

fn export_all_text(year: Option<i32>, output_dir: Option<String>) -> Result<()> {
    let data_dir = crate::settings::get_data_dir();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("exports"));
    std::fs::create_dir_all(&dir)?;

    let reports: Vec<(&str, Result<String>)> = vec![
        ("pnl", text::pnl(None, year, None, None)),
        ("expenses", text::expenses(None, year)),
        ("tax", text::tax(year)),
        ("cashflow", text::cashflow(None, year)),
        ("register", text::register(None, year, None, None, None)),
        ("flagged", text::flagged()),
        ("balance", text::balance()),
        ("k1-prep", text::k1(year)),
    ];

    for (name, result) in reports {
        match result {
            Ok(content) => {
                let path = dir.join(format!("{name}-{date}.txt"));
                std::fs::write(&path, content)?;
                println!("Wrote {}", path.display());
            }
            Err(e) => eprintln!("Skipping {name}: {e}"),
        }
    }
    Ok(())
}

fn dispatch_pdf_export(cmd: ReportCommands, output: Option<String>) -> Result<()> {
    #[cfg(not(feature = "pdf"))]
    {
        let _ = (cmd, output);
        return Err(crate::error::NigelError::Other(
            "PDF export requires the 'pdf' feature — build with `cargo build --features pdf`".into(),
        ));
    }

    #[cfg(feature = "pdf")]
    {
        crate::cli::export::dispatch_pdf(cmd, output)
    }
}

fn default_text_path(name: &str) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    crate::settings::get_data_dir()
        .join("exports")
        .join(format!("{name}-{date}.txt"))
        .to_string_lossy()
        .into_owned()
}

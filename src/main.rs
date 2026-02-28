mod browser;
mod categorizer;
mod cli;
mod db;
mod error;
mod fmt;
mod importer;
mod models;
mod tui;
#[cfg(feature = "pdf")]
mod pdf;
mod reconciler;
mod reports;
mod reviewer;
mod settings;

use clap::Parser;

#[cfg(feature = "pdf")]
use cli::ExportCommands;
use cli::{AccountsCommands, BrowseCommands, Cli, Commands, ReportCommands, RulesCommands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { data_dir } => cli::init::run(data_dir),
        Commands::Accounts { command } => match command {
            AccountsCommands::Add {
                name,
                account_type,
                institution,
                last_four,
            } => cli::accounts::add(&name, &account_type, institution.as_deref(), last_four.as_deref()),
            AccountsCommands::List => cli::accounts::list(),
        },
        Commands::Import {
            file,
            account,
            format,
        } => cli::import::run(&file, &account, format.as_deref()),
        Commands::Categorize => cli::categorize::run(),
        Commands::Demo => cli::demo::run(),
        Commands::Rules { command } => match command {
            RulesCommands::Add {
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::add(&pattern, &category, vendor.as_deref(), &match_type, priority),
            RulesCommands::List => cli::rules::list(),
            RulesCommands::Update {
                id,
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::update(id, pattern, category, vendor, match_type, priority),
            RulesCommands::Delete { id } => cli::rules::delete(id),
        },
        Commands::Review => cli::review::run(),
        Commands::Report { command } => match command {
            ReportCommands::Pnl {
                month,
                year,
                from_date,
                to_date,
            } => cli::report::pnl(month, year, from_date, to_date),
            ReportCommands::Expenses { month, year } => cli::report::expenses(month, year),
            ReportCommands::Tax { year } => cli::report::tax(year),
            ReportCommands::Cashflow { month, year } => cli::report::cashflow(month, year),
            ReportCommands::Register {
                month,
                year,
                from_date,
                to_date,
                account,
            } => cli::report::register(month, year, from_date, to_date, account),
            ReportCommands::Flagged => cli::report::flagged(),
            ReportCommands::Balance => cli::report::balance(),
            ReportCommands::K1 { year } => cli::report::k1(year),
        },
        Commands::Browse { command } => match command {
            BrowseCommands::Register {
                month,
                year,
                from_date,
                to_date,
                account,
            } => cli::browse::register(month, year, from_date, to_date, account),
        },
        #[cfg(feature = "pdf")]
        Commands::Export { command } => match command {
            ExportCommands::Pnl {
                month,
                year,
                from_date,
                to_date,
                output,
            } => cli::export::pnl(month, year, from_date, to_date, output),
            ExportCommands::Expenses {
                month,
                year,
                output,
            } => cli::export::expenses(month, year, output),
            ExportCommands::Tax { year, output } => cli::export::tax(year, output),
            ExportCommands::Cashflow {
                month,
                year,
                output,
            } => cli::export::cashflow(month, year, output),
            ExportCommands::Register {
                month,
                year,
                from_date,
                to_date,
                account,
                output,
            } => cli::export::register(month, year, from_date, to_date, account, output),
            ExportCommands::Flagged { output } => cli::export::flagged(output),
            ExportCommands::Balance { output } => cli::export::balance(output),
            ExportCommands::K1 { year, output } => cli::export::k1(year, output),
            ExportCommands::All { year, output_dir } => cli::export::all(year, output_dir),
        },
        Commands::Reconcile {
            account,
            month,
            balance,
        } => cli::reconcile::run(&account, &month, balance),
        Commands::Load { path } => cli::load::run(&path),
        Commands::Backup { output } => cli::backup::run(output),
        Commands::Status => cli::status::run(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

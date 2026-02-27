mod categorizer;
mod cli;
mod db;
mod error;
mod fmt;
mod importer;
mod models;
mod reconciler;
mod reports;
mod reviewer;
mod settings;

use clap::Parser;

use cli::{AccountsCommands, Cli, Commands, ReportCommands, RulesCommands};

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
        Commands::Rules { command } => match command {
            RulesCommands::Add {
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::add(&pattern, &category, vendor.as_deref(), &match_type, priority),
            RulesCommands::List => cli::rules::list(),
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
            ReportCommands::Flagged => cli::report::flagged(),
            ReportCommands::Balance => cli::report::balance(),
        },
        Commands::Reconcile {
            account,
            month,
            balance,
        } => cli::reconcile::run(&account, &month, balance),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

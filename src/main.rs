mod browser;
mod categorizer;
mod cli;
mod db;
mod effects;
mod error;
mod fmt;
mod importer;
mod migrations;
mod models;
#[cfg(feature = "pdf")]
mod pdf;
mod reconciler;
mod reports;
mod reviewer;
mod settings;
#[cfg(feature = "totp")]
mod totp;
mod tui;

use clap::{CommandFactory, Parser};

use cli::{
    AccountsCommands, BrowseCommands, CategoriesCommands, Cli, Commands, PasswordCommand,
    RulesCommands,
};

fn main() {
    // Install ratatui panic hook once — restores terminal on panic for all TUI screens
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let cli = Cli::parse();

    let result = match cli.command {
        // Dashboard handles missing init via its own onboarding flow
        None => cli::dashboard::run(),
        Some(command) => dispatch(command),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn dispatch(command: Commands) -> error::Result<()> {
    // Check that nigel has been initialized (skip for init/demo which create new DBs, and load which switches directories)
    if !matches!(
        command,
        Commands::Init { .. } | Commands::Demo | Commands::Load { .. }
    ) {
        let data_dir = crate::settings::get_data_dir();
        let db_path = data_dir.join("nigel.db");
        if !db_path.exists() {
            return Err(error::NigelError::NotInitialized);
        }
    }

    // Prompt for password if database is encrypted (skip for init/demo which may create new DBs)
    if !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Password { .. }
            | Commands::Completions { .. }
    ) {
        let data_dir = crate::settings::get_data_dir();
        let db_path = data_dir.join("nigel.db");
        if db_path.exists() {
            crate::db::prompt_password_if_needed(&db_path)?;
        }
    }

    match command {
        Commands::Init { data_dir } => cli::init::run(data_dir),
        Commands::Accounts { command } => match command {
            AccountsCommands::Add {
                name,
                account_type,
                institution,
                last_four,
            } => cli::accounts::add(
                &name,
                &account_type,
                institution.as_deref(),
                last_four.as_deref(),
            ),
            AccountsCommands::List => cli::accounts::list(),
            AccountsCommands::Rename { id, name } => cli::accounts::rename(id, &name),
            AccountsCommands::Delete { id } => cli::accounts::delete(id),
        },
        Commands::Categories { command } => match command {
            CategoriesCommands::List => cli::categories::list(),
            CategoriesCommands::Add {
                name,
                category_type,
                tax_line,
                form_line,
            } => cli::categories::add(
                &name,
                &category_type,
                tax_line.as_deref(),
                form_line.as_deref(),
            ),
            CategoriesCommands::Rename { id, name } => cli::categories::rename(id, &name),
            CategoriesCommands::Update {
                id,
                name,
                category_type,
                tax_line,
                form_line,
            } => cli::categories::update(
                id,
                &name,
                &category_type,
                tax_line.as_deref(),
                form_line.as_deref(),
            ),
            CategoriesCommands::Delete { id } => cli::categories::delete(id),
        },
        Commands::Import {
            file,
            account,
            format,
            dry_run,
            date_col,
            desc_col,
            amount_col,
            date_format,
            save_profile,
        } => cli::import::run(
            &file,
            &account,
            cli::import::ImportOpts {
                format: format.as_deref(),
                dry_run,
                date_col,
                desc_col,
                amount_col,
                date_format: date_format.as_deref(),
                save_profile: save_profile.as_deref(),
            },
        ),
        Commands::Categorize => cli::categorize::run(),
        Commands::Demo => cli::demo::run(),
        Commands::Rules { command } => match command {
            RulesCommands::Add {
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::add(
                &pattern,
                &category,
                vendor.as_deref(),
                &match_type,
                priority,
            ),
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
            RulesCommands::Test {
                pattern,
                match_type,
            } => cli::rules::test(&pattern, &match_type),
        },
        Commands::Review { id } => cli::review::run(id),
        Commands::Report { command } => cli::report::dispatch(command),
        Commands::Browse { command } => match command {
            BrowseCommands::Register {
                month,
                year,
                from_date,
                to_date,
                account,
            } => cli::browse::register(month, year, from_date, to_date, account),
        },
        Commands::Reconcile {
            account,
            month,
            balance,
        } => cli::reconcile::run(&account, &month, balance),
        Commands::Load { path } => cli::load::run(&path),
        Commands::Backup { output } => cli::backup::run(output),
        Commands::Restore { path } => cli::restore::run(&path),
        Commands::Undo => cli::undo::run(),
        Commands::Status => cli::status::run(),
        Commands::Password { command } => match command {
            PasswordCommand::Set => cli::password::run_set(),
            PasswordCommand::Change => cli::password::run_change(),
            PasswordCommand::Remove => cli::password::run_remove(),
            #[cfg(feature = "totp")]
            PasswordCommand::TotpEnable => cli::password::run_totp_enable(),
            #[cfg(feature = "totp")]
            PasswordCommand::TotpDisable => cli::password::run_totp_disable(),
            #[cfg(feature = "totp")]
            PasswordCommand::TotpRecover => cli::password::run_totp_recover(),
        },
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut cli::Cli::command(),
                "nigel",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}

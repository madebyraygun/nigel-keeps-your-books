mod browser;
mod categorizer;
mod cli;
mod db;
mod effects;
mod error;
mod fmt;
mod importer;
mod invoicing;
mod migrations;
mod models;
#[cfg(feature = "pdf")]
mod pdf;
mod reconciler;
mod reports;
mod reviewer;
mod settings;
mod tui;

use clap::{CommandFactory, Parser};

use cli::{
    AccountsCommands, BrowseCommands, CategoriesCommands, Cli, ClientCommands, Commands,
    InvoiceCommands, PasswordCommand, RulesCommands,
};

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Reconcile Stripe payments before a data-bearing command runs. Best-effort:
/// with no Stripe key configured it does nothing, and any failure prints a
/// notice instead of failing the command the user actually asked for.
fn sync_invoice_payments() {
    let Some(secret_key) = settings::invoicing_config().stripe_secret_key else {
        return;
    };
    let gateway = invoicing::stripe::StripeClient { secret_key };
    let db_path = settings::get_data_dir().join("nigel.db");
    let result = db::get_connection(&db_path)
        .and_then(|conn| invoicing::sync::sync_all(&conn, &today(), &gateway));

    match result {
        Ok(0) => {}
        Ok(n) => eprintln!("notice: recorded {n} new invoice payment(s)"),
        Err(e) => eprintln!("notice: invoice sync skipped: {e}"),
    }
}

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
        Some(command) => {
            // Non-blocking update check for CLI subcommands (dashboard does its own).
            // Skip when running `nigel update` since it does its own check.
            if !matches!(command, Commands::Update) {
                if let Some(msg) = cli::update::check_and_notify() {
                    eprintln!("notice: {msg}");
                }
            }
            dispatch(command)
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn dispatch(command: Commands) -> error::Result<()> {
    // Check that nigel has been initialized (skip for init/demo which create new DBs, load which switches directories, and update which needs no DB)
    if !matches!(
        command,
        Commands::Init { .. } | Commands::Demo | Commands::Load { .. } | Commands::Update
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
            | Commands::Update
    ) {
        let data_dir = crate::settings::get_data_dir();
        let db_path = data_dir.join("nigel.db");
        if db_path.exists() {
            crate::db::prompt_password_if_needed(&db_path)?;
        }
    }

    // Reconcile Stripe payments for commands that read or write the books.
    // `invoice sync` is excluded because it does the same work itself.
    if !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Load { .. }
            | Commands::Update
            | Commands::Completions { .. }
            | Commands::Password { .. }
            | Commands::Invoice {
                command: InvoiceCommands::Sync
            }
    ) {
        sync_invoice_payments();
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
        Commands::Client { command } => match command {
            ClientCommands::Add {
                name,
                email,
                address,
            } => cli::client::add(&name, email.as_deref(), address.as_deref()),
            ClientCommands::List => cli::client::list(),
        },
        Commands::Invoice { command } => match command {
            InvoiceCommands::New {
                client,
                issue_date,
                due_date,
                currency,
                items,
            } => cli::invoice::new(client, &issue_date, due_date.as_deref(), &currency, &items),
            InvoiceCommands::List => cli::invoice::list(),
            InvoiceCommands::Show { number } => cli::invoice::show(number),
            InvoiceCommands::Send { number } => cli::invoice::send(number, &today()),
            InvoiceCommands::Sync => cli::invoice::sync(&today()),
            InvoiceCommands::Pay {
                number,
                amount,
                date,
                method,
            } => cli::invoice::pay(number, amount, &date, &method),
            InvoiceCommands::Aging => cli::invoice::aging(&today()),
            InvoiceCommands::Import { db } => cli::invoice::import(&db),
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
        Commands::Update => cli::update::run(),
        Commands::Status => cli::status::run(),
        Commands::Password { command } => match command {
            PasswordCommand::Set => cli::password::run_set(),
            PasswordCommand::Change => cli::password::run_change(),
            PasswordCommand::Remove => cli::password::run_remove(),
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

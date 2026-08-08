use clap::{CommandFactory, Parser};

use nigel::cli::{
    self, AccountsCommands, BrowseCommands, CategoriesCommands, Cli, ClientCommands, Commands,
    InvoiceCommands, PasswordCommand, RulesCommands,
};
use nigel::error;

/// Reconcile Stripe payments before a data-bearing command runs. Best-effort:
/// with no Stripe key configured it does nothing, and any failure prints a
/// notice instead of failing the command the user actually asked for.
fn sync_invoice_payments() {
    let Some(secret_key) = nigel::settings::invoicing_config().stripe_secret_key else {
        return;
    };
    let gateway = nigel::invoicing::stripe::StripeClient { secret_key };
    let db_path = nigel::settings::get_data_dir().join("nigel.db");
    let result = nigel::db::get_connection(&db_path)
        .and_then(|conn| nigel::invoicing::sync::sync_all(&conn, &cli::today(), &gateway));

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
    // Commands that need an already-initialized database (skip for init/demo which create
    // new DBs, load which switches directories, and update which needs no DB)
    let needs_existing_db = !matches!(
        command,
        Commands::Init { .. } | Commands::Demo | Commands::Load { .. } | Commands::Update
    );

    // Commands that need the encryption password up front. `password` does its own
    // prompting as part of set/change/remove, `completions` never touches the DB, and
    // `serve` has no stdin to prompt on — its clients unlock over HTTP instead.
    let needs_password = !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Password { .. }
            | Commands::Completions { .. }
            | Commands::Serve { .. }
            | Commands::Update
    );

    let db_path = nigel::settings::get_data_dir().join("nigel.db");

    if needs_existing_db && !db_path.exists() {
        return Err(error::NigelError::NotInitialized);
    }

    if needs_password && db_path.exists() {
        nigel::db::prompt_password_if_needed(&db_path)?;
    }

    // `restore` overwrites the database file and then migrates the restored copy itself,
    // so migrating the outgoing one first is wasted work that could abort the very
    // recovery meant to repair it.
    let replaces_db = matches!(command, Commands::Restore { .. });

    // Bring the schema up to date before any command reads or writes data. The
    // intersection of the two guards above is exactly the set of commands that open the
    // existing database with a usable password; init/demo/restore migrate via their own
    // init_db() call, the dashboard migrates in its own pre-flight, and serve migrates
    // whatever it can reach without a password.
    if needs_existing_db && needs_password && !replaces_db {
        let conn = nigel::db::get_connection(&db_path)?;
        nigel::db::init_db(&conn)?;
    }

    // Reconcile Stripe payments for commands that read or write the books.
    // `restore` is excluded because it overwrites the database a sync would
    // write to, `invoice sync` because it does the same work itself, and
    // `serve` because its database may still be locked (no stdin to prompt on)
    // and its startup shouldn't block on a network poll. `invoice preview` is
    // defined to make no network call at all, and a launch sync would make that
    // false on any machine with a Stripe key configured.
    if !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Load { .. }
            | Commands::Update
            | Commands::Completions { .. }
            | Commands::Password { .. }
            | Commands::Restore { .. }
            | Commands::Serve { .. }
            | Commands::Invoice {
                command: InvoiceCommands::Sync | InvoiceCommands::Preview { .. }
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
            ClientCommands::Show { id } => cli::client::show(id),
            ClientCommands::Edit {
                id,
                name,
                email,
                address,
                notes,
            } => cli::client::edit(id, name, email, address, notes),
            ClientCommands::List => cli::client::list(),
        },
        Commands::Invoice { command } => match command {
            InvoiceCommands::New {
                client,
                issue_date,
                due_date,
                currency,
                items,
                notes,
                terms,
            } => cli::invoice::new(
                client,
                &issue_date,
                due_date.as_deref(),
                &currency,
                &items,
                notes.as_deref(),
                terms.as_deref(),
            ),
            InvoiceCommands::Edit {
                number,
                issue_date,
                due_date,
                clear_due,
                currency,
                notes,
                terms,
                items,
            } => cli::invoice::edit(
                number, issue_date, due_date, clear_due, currency, notes, terms, &items,
            ),
            InvoiceCommands::Void { number, yes } => cli::invoice::void(number, yes, &cli::today()),
            InvoiceCommands::List => cli::invoice::list(),
            InvoiceCommands::Show { number } => cli::invoice::show(number),
            InvoiceCommands::Preview { number, output_dir } => {
                cli::invoice::preview(number, output_dir)
            }
            InvoiceCommands::Send { number } => cli::invoice::send(number, &cli::today()),
            InvoiceCommands::Sync => cli::invoice::sync(&cli::today()),
            InvoiceCommands::Pay {
                number,
                amount,
                date,
                method,
            } => cli::invoice::pay(number, amount, &date, &method),
            InvoiceCommands::Aging => cli::invoice::aging(&cli::today()),
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
        Commands::Recategorize { args } => cli::recategorize::run(args),
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
        Commands::Serve { port, no_open } => cli::serve::run(port, no_open),
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

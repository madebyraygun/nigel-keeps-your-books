pub mod account_manager;
pub mod accounts;
pub mod backup;
pub mod browse;
pub mod categories;
pub mod categorize;
pub mod category_manager;
pub mod client;
pub mod dashboard;
pub mod demo;
pub mod export;
pub mod goodbye;
pub mod import;
pub mod import_manager;
pub mod init;
pub mod invoice;
pub mod load;
pub mod load_manager;
pub mod onboarding;
pub mod password;
pub mod password_manager;
pub mod recategorize;
pub mod reconcile;
pub mod reconcile_manager;
pub mod report;
pub mod restore;
pub mod review;
pub mod rules;
pub mod rules_manager;
pub mod serve;
pub mod settings_manager;
pub mod snake;
pub mod splash;
pub mod status;
pub mod undo;
pub mod undo_manager;
pub mod update;

use clap::{Args, Parser, Subcommand};

use crate::reports::ReportKind;

pub(crate) fn parse_month_opt(month: &Option<String>) -> (Option<i32>, Option<u32>) {
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

#[derive(Parser)]
#[command(
    name = "nigel",
    about = "Cash-basis bookkeeping CLI for small consultancies and personal finances."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set up Nigel: choose a data directory and initialize the database.
    Init {
        /// Path for Nigel data (default: ~/Documents/nigel)
        #[arg(long = "data-dir")]
        data_dir: Option<String>,
        /// Chart of accounts to seed a new database with: business (Schedule
        /// C / 1120-S mapping) or personal. Existing databases are unchanged.
        #[arg(long, default_value = "business")]
        profile: String,
    },
    /// Manage accounts.
    Accounts {
        #[command(subcommand)]
        command: AccountsCommands,
    },
    /// Manage the chart of accounts (categories).
    Categories {
        #[command(subcommand)]
        command: CategoriesCommands,
    },
    /// Manage clients.
    Client {
        #[command(subcommand)]
        command: ClientCommands,
    },
    /// Create, publish, and track invoices.
    Invoice {
        #[command(subcommand)]
        command: InvoiceCommands,
    },
    /// Import a CSV/XLSX file and auto-categorize transactions.
    Import {
        /// Path to CSV or XLSX file to import
        file: String,
        /// Account name to import into
        #[arg(long)]
        account: String,
        /// Importer format key (e.g. bofa_checking, or a saved profile name)
        #[arg(long)]
        format: Option<String>,
        /// Preview import without writing to database
        #[arg(long)]
        dry_run: bool,
        /// Column index for date (0-based, used with generic CSV)
        #[arg(long)]
        date_col: Option<usize>,
        /// Column index for description (0-based, used with generic CSV)
        #[arg(long)]
        desc_col: Option<usize>,
        /// Column index for amount (0-based, used with generic CSV)
        #[arg(long)]
        amount_col: Option<usize>,
        /// Date format string (default: %m/%d/%Y, used with generic CSV)
        #[arg(long)]
        date_format: Option<String>,
        /// Save column mapping as a reusable profile name
        #[arg(long)]
        save_profile: Option<String>,
    },
    /// Re-run categorization rules on uncategorized transactions.
    Categorize,
    /// Change the category of existing transactions by ID or filters.
    Recategorize {
        #[command(flatten)]
        args: recategorize::RecategorizeArgs,
    },
    /// Manage categorization rules.
    Rules {
        #[command(subcommand)]
        command: RulesCommands,
    },
    /// Interactively review flagged transactions.
    Review {
        /// Review a specific transaction by ID.
        #[arg(long)]
        id: Option<i64>,
    },
    /// Generate, view, or export reports.
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
    /// Load sample data (account, transactions, rules) to explore Nigel.
    Demo,
    /// Reconcile an account against a statement balance.
    Reconcile {
        /// Account name
        account: String,
        /// Month: YYYY-MM
        #[arg(long)]
        month: String,
        /// Statement ending balance
        #[arg(long)]
        balance: f64,
    },
    /// Switch to an existing Nigel data directory.
    Load {
        /// Path to data directory containing nigel.db
        path: String,
    },
    /// Back up the database.
    Backup {
        /// Output path (default: <data_dir>/backups/nigel-YYYYMMDD-HHMMSS.db)
        #[arg(long)]
        output: Option<String>,
    },
    /// Restore a database from a backup file.
    Restore {
        /// Path to the backup file to restore
        path: String,
    },
    /// Interactively browse data.
    Browse {
        #[command(subcommand)]
        command: BrowseCommands,
    },
    /// Serve the web UI and JSON API on localhost.
    Serve {
        /// Port to bind on 127.0.0.1 (0 picks an ephemeral port)
        #[arg(long, default_value_t = 5731)]
        port: u16,
        /// Don't open a browser window
        #[arg(long)]
        no_open: bool,
    },
    /// Show current database and summary statistics.
    Status,
    /// Manage database password (encrypt, change, or remove).
    Password {
        #[command(subcommand)]
        command: PasswordCommand,
    },
    /// Undo the last import (delete its transactions and import record).
    Undo,
    /// Check for and install updates from GitHub Releases.
    Update,
    /// Generate shell completions script.
    Completions {
        /// Shell: bash, zsh, fish, powershell
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum PasswordCommand {
    /// Set a password on an unencrypted database.
    Set,
    /// Change the password on an encrypted database.
    Change,
    /// Remove the password (decrypt the database).
    Remove,
}

#[derive(Subcommand)]
pub enum AccountsCommands {
    /// Add a new account.
    Add {
        /// Account name, e.g. 'BofA Checking'
        name: String,
        /// Account type: checking, credit_card, line_of_credit, payroll
        #[arg(long = "type")]
        account_type: String,
        /// Institution name
        #[arg(long)]
        institution: Option<String>,
        /// Last 4 digits of account number
        #[arg(long = "last-four")]
        last_four: Option<String>,
    },
    /// List all accounts.
    List,
    /// Rename an account by ID.
    Rename {
        /// Account ID
        id: i64,
        /// New name
        name: String,
    },
    /// Delete an account by ID (blocked if account has transactions).
    Delete {
        /// Account ID
        id: i64,
    },
}

#[derive(Subcommand)]
pub enum CategoriesCommands {
    /// List all categories.
    List,
    /// Add a new category.
    Add {
        /// Category name
        name: String,
        /// Category type: income or expense
        #[arg(long = "type")]
        category_type: String,
        /// IRS tax line mapping
        #[arg(long)]
        tax_line: Option<String>,
        /// Form 1120-S line mapping
        #[arg(long = "form-line")]
        form_line: Option<String>,
    },
    /// Rename a category by ID.
    Rename {
        /// Category ID
        id: i64,
        /// New name
        name: String,
    },
    /// Update a category's fields by ID.
    Update {
        /// Category ID
        id: i64,
        /// Category name
        name: String,
        /// Category type: income or expense
        #[arg(long = "type")]
        category_type: String,
        /// IRS tax line mapping
        #[arg(long)]
        tax_line: Option<String>,
        /// Form 1120-S line mapping
        #[arg(long = "form-line")]
        form_line: Option<String>,
    },
    /// Delete (deactivate) a category by ID.
    Delete {
        /// Category ID
        id: i64,
    },
}

#[derive(Subcommand)]
pub enum ClientCommands {
    /// Add a client.
    Add {
        /// Client name, e.g. 'Acme Co'
        name: String,
        /// Billing email (required before an invoice can be sent)
        #[arg(long)]
        email: Option<String>,
        /// Billing address
        #[arg(long)]
        address: Option<String>,
    },
    /// List all clients.
    List,
}

#[derive(Subcommand)]
pub enum InvoiceCommands {
    /// Create a draft invoice. Line items as "desc:qty:unit", repeatable.
    New {
        /// Client ID (shown in `nigel client list`)
        #[arg(long)]
        client: i64,
        /// Issue date: YYYY-MM-DD
        #[arg(long = "issue")]
        issue_date: String,
        /// Due date: YYYY-MM-DD
        #[arg(long = "due")]
        due_date: Option<String>,
        /// Currency code
        #[arg(long, default_value = "USD")]
        currency: String,
        /// Line item as "desc:qty:unit" (repeatable)
        #[arg(long = "item")]
        items: Vec<String>,
    },
    /// List invoices.
    List,
    /// Show one invoice by number.
    Show {
        /// Invoice number (shown in `nigel invoice list`)
        number: i64,
    },
    /// Render, publish to R2, and email an invoice.
    Send {
        /// Invoice number
        number: i64,
    },
    /// Poll Stripe and record any new payments.
    Sync,
    /// Manually record a payment (direct deposit, etc.).
    Pay {
        /// Invoice number
        number: i64,
        /// Amount paid (default: the full outstanding balance)
        #[arg(long)]
        amount: Option<f64>,
        /// Payment date: YYYY-MM-DD
        #[arg(long)]
        date: String,
        /// Payment method
        #[arg(long, default_value = "direct_deposit")]
        method: String,
    },
    /// A/R aging report.
    Aging,
    /// One-time import from an InvoiceShelf SQLite file.
    Import {
        /// Path to the InvoiceShelf SQLite database
        #[arg(long = "from-invoiceshelf")]
        db: String,
    },
}

#[derive(Subcommand)]
pub enum RulesCommands {
    /// Add a categorization rule.
    Add {
        /// Pattern to match against transaction descriptions
        pattern: String,
        /// Category name to assign
        #[arg(long)]
        category: String,
        /// Normalized vendor name
        #[arg(long)]
        vendor: Option<String>,
        /// Match type: contains, starts_with, regex
        #[arg(long = "match-type", default_value = "contains")]
        match_type: String,
        /// Rule priority (higher wins)
        #[arg(long, default_value = "0")]
        priority: i64,
    },
    /// List all categorization rules.
    List,
    /// Update an existing rule.
    Update {
        /// Rule ID (shown in `nigel rules list`)
        id: i64,
        /// New pattern
        #[arg(long)]
        pattern: Option<String>,
        /// New category name
        #[arg(long)]
        category: Option<String>,
        /// New vendor name
        #[arg(long)]
        vendor: Option<String>,
        /// New match type: contains, starts_with, regex
        #[arg(long = "match-type")]
        match_type: Option<String>,
        /// New priority
        #[arg(long)]
        priority: Option<i64>,
    },
    /// Delete (deactivate) a rule by ID.
    Delete {
        /// Rule ID (shown in `nigel rules list`)
        id: i64,
    },
    /// Test a pattern against existing transactions without creating a rule.
    Test {
        /// Pattern to match against transaction descriptions
        pattern: String,
        /// Match type: contains, starts_with, regex
        #[arg(long = "match-type", default_value = "contains")]
        match_type: String,
    },
}

/// Shared output arguments for report subcommands.
#[derive(Args, Clone, Default)]
pub struct ReportOutputArgs {
    /// Mode: view (default, interactive) or export (write to file)
    #[arg(long)]
    pub mode: Option<String>,
    /// Export format: pdf (default) or text
    #[arg(long)]
    pub format: Option<String>,
    /// Output file path (implies --mode export)
    #[arg(long)]
    pub output: Option<String>,
}

#[derive(Subcommand)]
pub enum ReportCommands {
    /// Profit & Loss report.
    Pnl {
        /// Month filter: YYYY-MM
        #[arg(long)]
        month: Option<String>,
        /// Year filter: YYYY
        #[arg(long)]
        year: Option<i32>,
        /// Start date: YYYY-MM-DD
        #[arg(long = "from")]
        from_date: Option<String>,
        /// End date: YYYY-MM-DD
        #[arg(long = "to")]
        to_date: Option<String>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Expense breakdown report.
    Expenses {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Tax summary organized by IRS line items.
    Tax {
        #[arg(long)]
        year: Option<i32>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Cash flow report with monthly inflows/outflows.
    Cashflow {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Transaction register — all transactions for a date period.
    Register {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long = "from")]
        from_date: Option<String>,
        #[arg(long = "to")]
        to_date: Option<String>,
        /// Filter by account name
        #[arg(long)]
        account: Option<String>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Show all flagged/uncategorized transactions.
    Flagged {
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Cash position snapshot.
    Balance {
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// K-1 preparation worksheet (Form 1120-S).
    K1 {
        #[arg(long)]
        year: Option<i32>,
        #[command(flatten)]
        output: ReportOutputArgs,
    },
    /// Export all reports (export-only).
    /// Note: All uses top-level fields instead of ReportOutputArgs because it has
    /// output_dir (not output) and is always export mode (no --mode flag needed).
    All {
        #[arg(long)]
        year: Option<i32>,
        /// Output directory
        #[arg(long = "output-dir")]
        output_dir: Option<String>,
        /// Export format: pdf (default) or text
        #[arg(long)]
        format: Option<String>,
    },
}

impl ReportCommands {
    pub fn output_args(&self) -> ReportOutputArgs {
        match self {
            Self::Pnl { output, .. } => output.clone(),
            Self::Expenses { output, .. } => output.clone(),
            Self::Tax { output, .. } => output.clone(),
            Self::Cashflow { output, .. } => output.clone(),
            Self::Register { output, .. } => output.clone(),
            Self::Flagged { output, .. } => output.clone(),
            Self::Balance { output, .. } => output.clone(),
            Self::K1 { output, .. } => output.clone(),
            Self::All { format, .. } => ReportOutputArgs {
                mode: Some("export".to_string()),
                format: format.clone(),
                output: None,
            },
        }
    }

    pub fn kind(&self) -> ReportKind {
        match self {
            Self::Pnl { .. } => ReportKind::Pnl,
            Self::Expenses { .. } => ReportKind::Expenses,
            Self::Tax { .. } => ReportKind::Tax,
            Self::Cashflow { .. } => ReportKind::Cashflow,
            Self::Register { .. } => ReportKind::Register,
            Self::Flagged { .. } => ReportKind::Flagged,
            Self::Balance { .. } => ReportKind::Balance,
            Self::K1 { .. } => ReportKind::K1,
            Self::All { .. } => ReportKind::All,
        }
    }

    pub fn report_name(&self) -> &'static str {
        self.kind().as_str()
    }
}

#[derive(Subcommand)]
pub enum BrowseCommands {
    /// Interactive transaction register browser.
    Register {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long = "from")]
        from_date: Option<String>,
        #[arg(long = "to")]
        to_date: Option<String>,
        /// Filter by account name
        #[arg(long)]
        account: Option<String>,
    },
}

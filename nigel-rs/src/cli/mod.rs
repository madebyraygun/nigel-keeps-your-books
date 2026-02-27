pub mod accounts;
pub mod categorize;
pub mod import;
pub mod init;
pub mod reconcile;
pub mod report;
pub mod review;
pub mod rules;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nigel", about = "Cash-basis bookkeeping CLI for small consultancies.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set up Nigel: choose a data directory and initialize the database.
    Init {
        /// Path for Nigel data (default: ~/Documents/nigel)
        #[arg(long = "data-dir")]
        data_dir: Option<String>,
    },
    /// Manage accounts.
    Accounts {
        #[command(subcommand)]
        command: AccountsCommands,
    },
    /// Import a CSV/XLSX file and auto-categorize transactions.
    Import {
        /// Path to CSV or XLSX file to import
        file: String,
        /// Account name to import into
        #[arg(long)]
        account: String,
        /// Importer format key (e.g. bofa_checking)
        #[arg(long)]
        format: Option<String>,
    },
    /// Re-run categorization rules on uncategorized transactions.
    Categorize,
    /// Manage categorization rules.
    Rules {
        #[command(subcommand)]
        command: RulesCommands,
    },
    /// Interactively review flagged transactions.
    Review,
    /// Generate reports.
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
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
    },
    /// Expense breakdown report.
    Expenses {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
    },
    /// Tax summary organized by IRS line items.
    Tax {
        #[arg(long)]
        year: Option<i32>,
    },
    /// Cash flow report with monthly inflows/outflows.
    Cashflow {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
    },
    /// Show all flagged/uncategorized transactions.
    Flagged,
    /// Cash position snapshot.
    Balance,
}

pub mod accounts;
pub mod backup;
pub mod browse;
pub mod categorize;
pub mod dashboard;
pub mod demo;
#[cfg(feature = "pdf")]
pub mod export;
pub mod import;
pub mod init;
pub mod load;
pub mod onboarding;
pub mod reconcile;
pub mod report;
pub mod review;
pub mod rules;
pub mod status;

use clap::{Parser, Subcommand};

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
#[command(name = "nigel", about = "Cash-basis bookkeeping CLI for small consultancies.")]
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
    /// Export reports to PDF.
    #[cfg(feature = "pdf")]
    Export {
        #[command(subcommand)]
        command: ExportCommands,
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
    /// Interactively browse data.
    Browse {
        #[command(subcommand)]
        command: BrowseCommands,
    },
    /// Show current database and summary statistics.
    Status,
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
}

#[cfg(feature = "pdf")]
#[derive(Subcommand)]
pub enum ExportCommands {
    /// Export Profit & Loss to PDF.
    Pnl {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long = "from")]
        from_date: Option<String>,
        #[arg(long = "to")]
        to_date: Option<String>,
        /// Output file path
        #[arg(long)]
        output: Option<String>,
    },
    /// Export expense breakdown to PDF.
    Expenses {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        output: Option<String>,
    },
    /// Export tax summary to PDF.
    Tax {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        output: Option<String>,
    },
    /// Export cash flow to PDF.
    Cashflow {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        output: Option<String>,
    },
    /// Export transaction register to PDF.
    Register {
        #[arg(long)]
        month: Option<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long = "from")]
        from_date: Option<String>,
        #[arg(long = "to")]
        to_date: Option<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        output: Option<String>,
    },
    /// Export flagged transactions to PDF.
    Flagged {
        #[arg(long)]
        output: Option<String>,
    },
    /// Export cash position to PDF.
    Balance {
        #[arg(long)]
        output: Option<String>,
    },
    /// Export K-1 preparation worksheet to PDF.
    K1 {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        output: Option<String>,
    },
    /// Export all reports to PDF.
    All {
        #[arg(long)]
        year: Option<i32>,
        /// Output directory
        #[arg(long = "output-dir")]
        output_dir: Option<String>,
    },
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
    },
    /// Show all flagged/uncategorized transactions.
    Flagged,
    /// Cash position snapshot.
    Balance,
    /// K-1 preparation worksheet (Form 1120-S).
    K1 {
        #[arg(long)]
        year: Option<i32>,
    },
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

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NigelError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Not initialized. Run `nigel init` first to set up your data directory.")]
    NotInitialized,

    #[error("Account '{0}' not found. Run `nigel accounts list` to see available accounts, or `nigel accounts add` to create one.")]
    UnknownAccount(String),

    #[error("Unknown format: '{0}'. Run `nigel import --help` for supported formats.")]
    UnknownFormat(String),

    #[error("Couldn't detect the format of this file for account type '{0}'. Use `--format <key>` to specify. Run `nigel import --help` for supported formats.")]
    NoImporter(String),

    #[error("No transactions found for {account} in {month}.")]
    NoTransactions { account: String, month: String },

    #[error("Unknown category: {0}")]
    UnknownCategory(String),

    #[error("Settings error: {0}")]
    Settings(String),

    #[cfg(feature = "pdf")]
    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NigelError>;

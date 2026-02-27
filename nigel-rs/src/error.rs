use thiserror::Error;

#[derive(Error, Debug)]
pub enum NigelError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Unknown account: {0}")]
    UnknownAccount(String),

    #[error("Unknown format: {0}")]
    UnknownFormat(String),

    #[error("No importer for account type: {0}")]
    NoImporter(String),

    #[error("Unknown category: {0}")]
    UnknownCategory(String),

    #[error("Settings error: {0}")]
    Settings(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NigelError>;

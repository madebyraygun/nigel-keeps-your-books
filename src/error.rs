use std::fmt;

use thiserror::Error;

/// Why a delete was refused. The variants are the vocabulary the API publishes
/// as `details.reason`, so the client can render its own wording instead of
/// parsing ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    HasTransactions,
    HasActiveRules,
    HasInvoices,
}

/// A refused delete: what was being deleted, why, and how much of it there is.
///
/// `Display` is the message the CLI and the TUI have always printed; the parts
/// stay separately readable so the API can answer with a code and a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteBlock {
    /// The noun in the message: "account" or "category".
    pub subject: &'static str,
    pub reason: BlockReason,
    pub count: i64,
}

impl DeleteBlock {
    pub fn transactions(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasTransactions,
            count,
        }
    }

    pub fn active_rules(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasActiveRules,
            count,
        }
    }

    pub fn invoices(subject: &'static str, count: i64) -> Self {
        Self {
            subject,
            reason: BlockReason::HasInvoices,
            count,
        }
    }

    pub fn reason_code(&self) -> &'static str {
        match self.reason {
            BlockReason::HasTransactions => "has_transactions",
            BlockReason::HasActiveRules => "has_active_rules",
            BlockReason::HasInvoices => "has_invoices",
        }
    }
}

impl fmt::Display for DeleteBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let plural = if self.count == 1 { "" } else { "s" };
        let Self { subject, count, .. } = self;
        match self.reason {
            BlockReason::HasTransactions => {
                write!(
                    f,
                    "Cannot delete: {subject} has {count} transaction{plural}"
                )
            }
            BlockReason::HasActiveRules => {
                write!(
                    f,
                    "Cannot delete: {subject} has {count} active rule{plural}"
                )
            }
            BlockReason::HasInvoices => {
                write!(f, "Cannot delete: {subject} has {count} invoice{plural}")
            }
        }
    }
}

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

    /// A record that was addressed by id or name is not there.
    #[error("{0}")]
    NotFound(String),

    /// The caller's input is wrong: an empty name, an unknown type, a pattern
    /// that will not compile.
    #[error("{0}")]
    Invalid(String),

    /// A name that has to be unique is taken. `kind` is capitalized because it
    /// opens the sentence.
    #[error("{kind} name already exists: {name}")]
    DuplicateName { kind: &'static str, name: String },

    /// A delete refused by a guardrail.
    #[error("{0}")]
    Blocked(DeleteBlock),

    /// Any other state conflict, carrying the machine-readable reason the API
    /// publishes alongside the message.
    #[error("{message}")]
    Conflict { code: &'static str, message: String },

    #[cfg(feature = "pdf")]
    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NigelError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// These strings are printed by the CLI and the TUI. They are asserted here
    /// so a future edit to the structured form cannot quietly reword them.
    #[test]
    fn delete_blocks_read_exactly_as_they_always_have() {
        let cases = [
            (
                DeleteBlock::transactions("account", 1),
                "Cannot delete: account has 1 transaction",
            ),
            (
                DeleteBlock::transactions("account", 12),
                "Cannot delete: account has 12 transactions",
            ),
            (
                DeleteBlock::transactions("category", 1),
                "Cannot delete: category has 1 transaction",
            ),
            (
                DeleteBlock::active_rules("category", 1),
                "Cannot delete: category has 1 active rule",
            ),
            (
                DeleteBlock::active_rules("category", 3),
                "Cannot delete: category has 3 active rules",
            ),
            (
                DeleteBlock::invoices("client", 1),
                "Cannot delete: client has 1 invoice",
            ),
            (
                DeleteBlock::invoices("client", 3),
                "Cannot delete: client has 3 invoices",
            ),
        ];
        for (block, expected) in cases {
            assert_eq!(block.to_string(), expected);
            assert_eq!(NigelError::Blocked(block).to_string(), expected);
        }
    }

    #[test]
    fn block_reasons_have_stable_wire_codes() {
        assert_eq!(
            DeleteBlock::transactions("account", 1).reason_code(),
            "has_transactions"
        );
        assert_eq!(
            DeleteBlock::active_rules("category", 1).reason_code(),
            "has_active_rules"
        );
        assert_eq!(
            DeleteBlock::invoices("client", 1).reason_code(),
            "has_invoices"
        );
    }

    #[test]
    fn duplicate_name_opens_with_the_kind() {
        let err = NigelError::DuplicateName {
            kind: "Account",
            name: "BofA Checking".into(),
        };
        assert_eq!(
            err.to_string(),
            "Account name already exists: BofA Checking"
        );
    }
}

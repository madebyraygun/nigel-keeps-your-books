//! Nigel — cash-basis bookkeeping for small consultancies.
//!
//! This library holds the whole implementation: the SQLite data layer, importers,
//! the rules engine, reports, and the CLI/TUI modules. The `nigel` binary
//! (`src/main.rs`) is a thin shell over it — clap parsing, the dispatch pre-flight,
//! and the terminal-restoring panic hook.

pub mod browser;
pub mod categorizer;
pub mod cli;
pub mod db;
pub mod effects;
pub mod error;
pub mod fmt;
pub mod importer;
pub mod migrations;
pub mod models;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod reconciler;
pub mod reports;
pub mod reviewer;
pub mod settings;
pub mod tui;

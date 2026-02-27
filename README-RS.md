# Nigel (Rust)

*A reliable bloke who does his job well.*

Nigel is a cash-basis bookkeeping CLI for small consultancies. Replace QuickBooks with a simple, local-first workflow: import bank CSVs and payroll reports, auto-categorize transactions via rules, review flagged items, and generate reports — all from the terminal.

This is the Rust implementation of Nigel. It reads the same SQLite database and settings.json as the Python version.

## Features

- **Bank imports** — CSV/XLSX parsers with format auto-detection (Bank of America checking, credit card, line of credit)
- **Gusto payroll** — XLSX payroll importer with auto-categorization (enabled by default via `gusto` feature)
- **Duplicate detection** — file-level checksums and transaction-level matching prevent double-imports
- **Rules engine** — pattern-based auto-categorization (contains, starts_with, regex) with priority ordering
- **Interactive review** — step through flagged transactions, assign categories, and create rules on the fly
- **Reports** — Profit & Loss, expense breakdown, tax summary (IRS Schedule C / 1120-S), cash flow, balance
- **Monthly reconciliation** — compare calculated balances against bank statements
- **SQLite storage** — single portable database, no server required

## Install

Download a pre-built binary from [GitHub Releases](https://github.com/madebyraygun/nigel-keeps-your-books/releases), or build from source:

```bash
cargo install --path nigel-rs
```

## Quick Start

```bash
# Initialize — prompts for data directory on first run
nigel init

# Or specify a custom data directory
nigel init --data-dir ~/my-books

# Add an account
nigel accounts add "BofA Checking" --type checking --institution "Bank of America"

# Import transactions
nigel import statement.csv --account "BofA Checking"

# Add a categorization rule
nigel rules add "ADOBE" --category "Software & Subscriptions" --vendor "Adobe"

# Re-run categorization
nigel categorize

# Review flagged transactions
nigel review

# Generate reports
nigel report pnl --year 2025
nigel report expenses --month 2025-03
nigel report tax --year 2025
nigel report cashflow
nigel report balance
nigel report flagged

# Reconcile against a bank statement
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67
```

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `gusto` | Yes | Gusto payroll XLSX importer + auto-categorization |

Build without Gusto support:

```bash
cargo build --release --no-default-features
```

## Compatibility

The Rust version reads and writes the same SQLite database (`nigel.db`) and configuration file (`~/.config/nigel/settings.json`) as the Python version. You can switch between them freely.

## Development

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test --no-default-features  # Test without gusto feature
```

## License

MIT

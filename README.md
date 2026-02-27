# Nigel

*A reliable bloke who does his job well.*

Nigel is a cash-basis bookkeeping CLI for small consultancies. Replace QuickBooks with a simple, local-first workflow: import bank CSVs and payroll reports, auto-categorize transactions via rules, review flagged items, and generate reports — all from the terminal.

## Features

- **Bank imports** — CSV/XLSX parsers with format auto-detection
- **Payroll import** — XLSX payroll importer with auto-categorization
- **Duplicate detection** — file-level checksums and transaction-level matching prevent double-imports
- **Rules engine** — pattern-based auto-categorization (contains, starts_with, regex) with priority ordering
- **Interactive review** — step through flagged transactions with a pinned category chart, assign categories, and create rules on the fly
- **Reports** — Profit & Loss, expense breakdown, tax summary (IRS Schedule C / 1120-S), cash flow, balance
- **Monthly reconciliation** — compare calculated balances against bank statements
- **SQLite storage** — single portable database, no server required

Importers currently include Bank of America and Gusto, but adding new importers is straightforward. See [docs/importers.md] for more information. The repository also contains a Claude skill that can create an importer from any data file.

Nigel also includes a demo mode** — `nigel demo` which loads sample data so you can explore every feature without requiring any personal data.

## Install

Download a pre-built binary from [GitHub Releases](https://github.com/madebyraygun/nigel-keeps-your-books/releases), or build from source:

```bash
cargo install --path .
```

## Quick Start

```bash
# Initialize — prompts for data directory on first run
nigel init

# Load sample data to explore
nigel demo

# Or set up your own accounts
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

## Walkthrough

See [docs/walkthrough.md](docs/walkthrough.md) for a guided tour using demo data: explore accounts and rules, review flagged transactions, add new rules, and run every report.

## Configuration

Settings are stored in `~/.config/nigel/settings.json`. The data directory (database, imports) defaults to `~/Documents/nigel/` and can be changed by re-running `nigel init --data-dir <path>`.

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `gusto` | Yes | Gusto payroll XLSX importer + auto-categorization |

Build without Gusto support:

```bash
cargo build --release --no-default-features
```

## Development

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test --no-default-features  # Test without gusto feature
```

## License

MIT

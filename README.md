# Nigel

*A reliable bloke who does his job well.*

Nigel is a cash-basis bookkeeping CLI for small consultancies. Replace QuickBooks with a simple, local-first workflow: import bank CSVs and payroll reports, auto-categorize transactions via rules, review flagged items, and generate reports — all from the terminal.

Nigel is designed for humans but works extremely well with AI agents. The repo includes [Claude skills](docs/skills.md) to add new importers and intelligently create new rules from your statements before importing into Nigel. With a tool like Claude Cowork, point it at your CSV statement and say "Import my latest statements into Nigel and generate my monthly P&L" or "Generate a Schedule K-1 prep report for 2026."

## Features

- **Interactive dashboard** — run `nigel` with no arguments to see YTD financials, account balances, a monthly income/expense chart, and a command menu; browse, review, import, reconcile, view rules, run reports, export PDFs, and switch data files — all from the dashboard
- **Bank imports** — CSV/XLSX parsers with format auto-detection
- **Payroll import** — XLSX payroll importer with auto-categorization
- **Duplicate detection** — file-level checksums and transaction-level matching prevent double-imports
- **Auto-snapshot** — automatic database snapshot before every import for easy rollback
- **Rules engine** — pattern-based auto-categorization (contains, starts_with, regex) with priority ordering
- **Interactive review** — step through flagged transactions with a pinned category chart, assign categories, and create rules on the fly; press Esc to go back and redo previous transactions
- **Reports** — Profit & Loss, expense breakdown, tax summary (IRS Schedule C / 1120-S), cash flow, balance
- **Interactive browser** — paginated register browser with keyboard navigation, jump-to-date, and transaction search
- **PDF export** — export any report to PDF, individually or all at once
- **Monthly reconciliation** — compare calculated balances against bank statements
- **SQLite storage** — single portable database, no server required

Importers currently include Bank of America and Gusto, but adding new importers is straightforward. See [docs/importers.md](docs/importers.md) for more information. The repository also contains a Claude skill that can create an importer from any data file. Contributions for importers for widely used import formats are welcome.

Nigel also includes a demo mode** — `nigel demo` which loads sample data so you can explore every feature without requiring any personal data. Explore the demo data by taking a guided tour with the [docs/walkthrough.md](docs/walkthrough.md): explore accounts and rules, review flagged transactions, add new rules, and run every report.

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

# Launch the interactive dashboard
nigel

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

# Interactive register browser
nigel browse register --year 2025
nigel browse register --account "BofA Checking"

# Export reports to PDF
nigel export pnl --year 2025
nigel export all --year 2025 --output-dir ~/exports/

# Reconcile against a bank statement
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67

# See what's active
nigel status

# Switch between data directories
nigel load ~/other-books

# Back up your database
nigel backup
nigel backup --output /tmp/nigel-backup.db
```

## Configuration

Settings are stored in `~/.config/nigel/settings.json`. The data directory defaults to `~/Documents/nigel/` and can be changed by re-running `nigel init --data-dir <path>`. Use `nigel load <path>` to switch between existing data directories without reinitializing. `nigel status` shows the active database and summary statistics.

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `gusto` | Yes | Gusto payroll XLSX importer + auto-categorization |
| `pdf` | Yes | PDF export via printpdf (built-in Helvetica, no font files needed) |

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

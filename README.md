# Nigel

*A reliable bloke who does his job well.*

Nigel is a cash-basis bookkeeping CLI for small consultancies. Replace QuickBooks with a simple, local-first workflow: import bank CSVs and payroll reports, auto-categorize transactions via rules, review flagged items, and generate reports — all from the terminal.

## Features

- **Bank imports** — CSV/XLSX parsers with format auto-detection
- **Plugin architecture** — importers, reports, and exports are plugins; add new institutions or output formats without touching core
- **Format auto-detection** — importers inspect file headers to pick the right parser; override with `--format` when needed
- **PDF export** — export any report to print-ready PDF via WeasyPrint
- **Duplicate detection** — file-level checksums and transaction-level matching prevent double-imports
- **Rules engine** — pattern-based auto-categorization (contains, starts_with, regex) with priority ordering
- **Interactive review** — step through flagged transactions, assign categories, and create rules on the fly so Nigel learns from every correction
- **Reports** — Profit & Loss, expense breakdown, tax summary (IRS Schedule C / 1120-S), cash flow, balance
- **Monthly reconciliation** — compare calculated balances against bank statements
- **SQLite storage** — single portable database, no server required

## Install

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/madebyraygun/nigel-keeps-your-books.git
cd nigel-keeps-your-books
uv sync
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

## Walkthrough

See [docs/walkthrough.md](docs/walkthrough.md) for a complete month-end cycle: import a bank statement, categorize, review, reconcile, and run reports.

## Configuration

Settings are stored in `~/.config/nigel/settings.json`. The data directory (database, imports, exports) defaults to `~/Documents/nigel/` and can be changed by re-running `nigel init --data-dir <path>`.

## Plugins

Nigel supports plugins via Python entry points. Install a plugin package and run `nigel init` to apply its migrations.

| Plugin | Description | Install |
|--------|-------------|---------|
| **nigel-bofa** | Bank of America importers (checking, credit card, line of credit) | `uv pip install -e plugins/nigel-bofa` |
| **nigel-gusto** | Gusto payroll XLSX importer with auto-categorization | `uv pip install -e plugins/nigel-gusto` |
| **nigel-k1** | K-1 prep report (1120-S / Schedule K) | `uv pip install -e plugins/nigel-k1` |
| **nigel-export-pdf** | Export any report to print-ready PDF via WeasyPrint | `uv pip install -e plugins/nigel-export-pdf` |

### K-1 Prep Report

```bash
uv pip install -e plugins/nigel-k1   # Install plugin
nigel init                             # Run plugin migrations
nigel k1 setup                         # Configure entity & shareholders
nigel report k1-prep --year 2025       # Generate K-1 prep worksheet
```

### PDF Export

```bash
uv pip install -e plugins/nigel-export-pdf
nigel export pnl --year 2025                    # Single report
nigel export all --year 2025 --company "Acme"   # All reports
```

## Development

```bash
uv sync                    # Install dependencies
uv run pytest -v           # Run tests
uv run nigel --help        # CLI help
```

## License

MIT

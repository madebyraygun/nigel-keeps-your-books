# Nigel

**Nigel keeps your books so you don't have to**

Nigel is a cash-basis bookkeeping CLI for small consultancies. Replaces QuickBooks with a simple, local-first workflow: import bank CSVs, auto-categorize transactions via rules, review flagged items, and generate reports — all from the terminal.

## Features

- **Bank imports** — CSV/XLSX parsers for industry standard bank and payroll service providers
- **Duplicate detection** — file-level checksums and transaction-level matching prevent double-imports
- **Rules engine** — pattern-based auto-categorization (contains, starts_with, regex) with priority ordering
- **Interactive review** — step through flagged transactions, assign categories, create rules on the fly
- **Reports** — Profit & Loss, expense breakdown, tax summary (IRS Schedule C), cash flow, balance
- **Monthly reconciliation** — compare calculated balances against bank statements
- **SQLite storage** — single portable database, no server required

## Install

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/madebyraygun/nigel.git
cd nigel
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

## Configuration

Settings are stored in `~/.config/nigel/settings.json`. The data directory (database, imports, exports) defaults to `~/Documents/nigel/` and can be changed by re-running `nigel init --data-dir <path>`.

## Development

```bash
uv sync                    # Install dependencies
uv run pytest -v           # Run tests
uv run nigel --help        # CLI help
```

## License

MIT

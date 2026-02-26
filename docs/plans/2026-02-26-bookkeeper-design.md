# Raygun Bookkeeper — Design Document

## Overview

A Python CLI tool (`bookkeeper`) replacing QuickBooks for Raygun. Cash-basis, single-entry accounting with bank CSV imports, rules-based categorization, and SQLite storage. No AI API calls — Claude Code serves as the conversational interface when needed.

## Architecture

### Tech Stack
- **Language:** Python (managed via uv)
- **CLI:** Typer
- **Database:** SQLite (stdlib `sqlite3`)
- **Terminal UI:** rich
- **XLSX parsing:** openpyxl
- **Testing:** pytest

### Project Structure
```
bookkeeper/
├── pyproject.toml
├── src/
│   └── bookkeeper/
│       ├── __init__.py
│       ├── cli.py           # Typer app, subcommand definitions
│       ├── db.py            # SQLite connection, schema init
│       ├── models.py        # Dataclasses for transactions, rules, categories, etc.
│       ├── importer.py      # CSV/XLSX parsing, duplicate detection, normalization
│       ├── categorizer.py   # Rules engine: match, apply, flag unknowns
│       ├── reviewer.py      # Interactive review flow (terminal-based)
│       ├── reports.py       # Report generation
│       └── reconciler.py    # Monthly reconciliation logic
├── tests/
│   ├── conftest.py          # Fixtures (in-memory SQLite, sample data)
│   ├── test_importer.py
│   ├── test_categorizer.py
│   ├── test_reports.py
│   └── test_reconciler.py
├── data/                    # .gitignored — local dev only
└── docs/
```

### Data Directory
All persistent data lives at `~/Documents/bookkeeper/` (backed up daily):
- `raygun.db` — SQLite database
- `imports/` — archived import files
- `exports/` — generated reports

Configurable via `--data-dir` flag or `BOOKKEEPER_DATA_DIR` env var.

### Design Principles
- Business logic in focused modules, CLI is a thin layer
- `db.py` owns the connection and schema; other modules take a connection as parameter
- No ORM — raw SQL with dataclasses for structure
- All financial modifications require user confirmation

## Database Schema

Six tables as defined in `raygun-bookkeeper-plan.md`: accounts, categories, transactions, rules, imports, reconciliations. No changes from the plan.

Account types: `checking`, `credit_card`, `line_of_credit`, `payroll`.

## CLI Subcommands

```
bookkeeper init                              # Create DB, seed categories
bookkeeper import <file> --account <name>    # Import CSV/XLSX, auto-categorize
bookkeeper review                            # Interactive review of flagged transactions
bookkeeper categorize                        # Re-run rules on uncategorized transactions
bookkeeper report pnl [--month|--year|--range]
bookkeeper report expenses [--month|--year]
bookkeeper report tax [--year]
bookkeeper report cashflow [--month|--year]
bookkeeper report flagged
bookkeeper report balance                    # Cash position snapshot
bookkeeper reconcile <account> --month <YYYY-MM> --balance <amount>
bookkeeper accounts list
bookkeeper accounts add <name> --type <type> --institution <name> --last-four <digits>
bookkeeper rules list
bookkeeper rules add <pattern> --category <name> --vendor <name>
```

All reports accept `--month YYYY-MM`, `--year YYYY`, or `--from/--to YYYY-MM-DD`. Default is current YTD. Optional `--csv` flag exports to `~/Documents/bookkeeper/exports/`.

## Import Parsers

### Shared BofA Handling
All three BofA formats have a metadata preamble (4-6 rows) before the real header. Parser detects and skips to the actual data.

### BofA Checking
- **Columns:** Date, Description, Amount, Running Bal.
- **Dates:** MM/DD/YYYY
- **Amounts:** Signed with comma thousands separators (e.g., `-2,500.00`). Strip commas before parsing.
- **Preamble:** ~6 rows of summary (total credits/debits, beginning balance, blank line).

### BofA Credit Card
- **Columns:** CardHolder Name, Account/Card Number - last 4 digits, Posting Date, Trans. Date, Reference ID, Description, Amount, MCC, Merchant Category, Transaction Type, Expense Category
- **Dates:** MM/DD/YYYY (Posting Date and Trans. Date — use Trans. Date)
- **Amounts:** Always positive. Direction from Transaction Type: `D` = charge (negate), `C` = credit/payment (keep positive).
- **Preamble:** 4 rows of summary metadata.

### BofA Line of Credit
- **Columns:** Same 11 columns as credit card (identical header).
- **Dates:** MM/DD/YYYY
- **Amounts:** Pre-signed. Positive = debit/charge, negative = credit/payment. Transaction Type column is redundant but consistent.
- **Preamble:** 4 rows of summary metadata.
- **Quirks:** MCC always `0000`, Merchant Category and Expense Category always empty.

### Gusto Payroll (XLSX)
- **Format:** XLSX with 7 sheets (employees, employee_jobs, work_locations, payrolls, earnings, taxes, deductions)
- **Key sheets:**
  - `payrolls` — one row per employee per pay run. Sum `Gross pay` per check date → "Payroll — Wages".
  - `taxes` — multiple rows per employee per run. Filter `Type = 'Employer'`, sum per period → "Payroll — Taxes".
- **Dates:** Excel serial numbers (convert via `datetime(1900,1,1) + timedelta(days=serial-2)`).
- **Aggregation:** Sum across all employees per pay period. Discard individual employee data.
- **Quirks:** Deductions sheet is header-only. If populated later, map to "Payroll — Benefits".

### Duplicate Detection
1. **File-level:** SHA-256 checksum against imports table. Rejects exact re-imports.
2. **Row-level:** Same date + amount + description + account → skip row. Handles overlapping date ranges.

## Categorization Engine

### Rules
- Stored in `rules` table with pattern, match_type (contains/starts_with/regex), vendor, category_id, priority.
- Applied in priority DESC order — first match wins.
- `hit_count` incremented on each match.

### Auto-categorize Flow
`bookkeeper import` runs categorization automatically after import. `bookkeeper categorize` re-runs on uncategorized transactions (useful after adding new rules).

1. For each uncategorized transaction, run against rules by priority.
2. Match found → assign category and vendor, increment hit_count.
3. No match → flag transaction (`is_flagged = 1`, `flag_reason = "No matching rule"`).

## Review Flow

`bookkeeper review` walks through flagged transactions interactively:

1. Display transaction (date, description, amount, account) via rich formatting.
2. User selects a category (fuzzy search or pick from list).
3. Prompt: "Create a rule for future transactions matching `<pattern>`?" (y/n).
4. If yes, suggest pattern and vendor name — user confirms or edits.
5. Each transaction committed individually. `skip` moves on, `quit` exits with progress saved.

## Reports

### P&L (`report pnl`)
Total income by category, total expenses by category, net profit/loss.

### Expense Breakdown (`report expenses`)
Expenses by category with percentages, top vendors by spend.

### Tax Summary (`report tax`)
Income and expenses organized by IRS tax line item. Flags items with special treatment (meals at 50%, home office).

### Cash Flow (`report cashflow`)
Monthly inflows vs outflows, running balance over time.

### Flagged Transactions (`report flagged`)
All uncategorized/flagged transactions.

### Balance (`report balance`)
Cash position snapshot — current balance per account, total across accounts, net income YTD.

## Dependencies

```
typer
rich
openpyxl
pytest
```

CSV parsing and SQLite are stdlib.

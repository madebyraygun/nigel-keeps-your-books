# CLAUDE.md

**IMPORTANT**: before you do anything else, run the `beans prime` command and heed its output.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Python CLI bookkeeping tool replacing QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Typer app in `src/nigel/cli.py` — subcommands: init, import, categorize, review, reconcile, accounts, rules, report
- **Database:** SQLite via `src/nigel/db.py` — tables: accounts, categories, transactions, rules, imports, reconciliations
- **Modules:** `importer.py` (CSV/XLSX parsing), `categorizer.py` (rules engine), `reviewer.py` (interactive review), `reports.py` (P&L, expenses, tax, cashflow, balance), `reconciler.py` (monthly reconciliation)
- **Data flow:** CSV/XLSX import → duplicate detection → auto-categorize via rules → flag unknowns for human review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C line items.
- **Settings:** `~/.config/nigel/settings.json` — stores `data_dir`, `company_name`, `fiscal_year_start`
- **Data directory:** `~/Documents/nigel/` by default, configurable via `nigel init --data-dir`

## Commands

```bash
uv sync                                           # Install dependencies
uv run pytest -v                                  # Run all tests
uv run pytest tests/test_importer.py -v           # Run single test file
uv run pytest tests/test_importer.py::test_name   # Run single test
uv run nigel --help                          # CLI help
uv run nigel init                            # Initialize (prompts for data dir on first run)
uv run nigel init --data-dir ~/my-books      # Initialize with custom data dir
uv run nigel import <file> --account <name>  # Import CSV/XLSX
uv run nigel categorize                      # Re-run rules on uncategorized
uv run nigel review                          # Interactive review
uv run nigel report pnl                      # Profit & Loss
uv run nigel report balance                  # Cash position
```

## Documentation Policy

Every feature change must update the relevant documentation before the work is considered complete:

- **CLAUDE.md** — update Architecture, Commands, Project Structure, and Key Design Constraints sections when adding/changing CLI commands, modules, data flow, or settings
- **README.md** — update Quick Start, Features, and Configuration sections for user-facing changes
- **Beans** — update or close related beans when completing work tracked there

Do not merge or mark work complete if docs are stale.

## Key Design Constraints

- All financial modifications require user confirmation — auto-categorizes but never silently changes confirmed data
- Duplicate detection uses file checksums (imports table) and transaction-level matching (date + amount + description + account)
- Rules are ordered by priority DESC; first match wins
- Gusto imports extract only aggregate totals, never individual employee data
- BofA CSV formats vary by account type (checking, credit_card, line_of_credit) — each has its own parser
- Gusto payroll transactions are pre-categorized on import (Wages, Taxes, Benefits)

## Project Structure

```
src/nigel/          # Python package
  cli.py            # Typer CLI entry point
  db.py             # SQLite schema, connection, category seeding
  models.py         # Dataclasses
  importer.py       # CSV/XLSX parsers, import_file()
  categorizer.py    # Rules engine
  reviewer.py       # Interactive review flow
  reports.py        # Report data functions
  reconciler.py     # Monthly reconciliation
tests/              # pytest tests
  fixtures/         # Sample CSV/XLSX files
```

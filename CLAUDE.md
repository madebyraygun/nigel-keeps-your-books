# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Raygun Bookkeeper — a Python CLI bookkeeping tool replacing QuickBooks for Raygun. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Typer app in `src/bookkeeper/cli.py` — subcommands: init, import, categorize, review, reconcile, accounts, rules, report
- **Database:** SQLite via `src/bookkeeper/db.py` — tables: accounts, categories, transactions, rules, imports, reconciliations
- **Modules:** `importer.py` (CSV/XLSX parsing), `categorizer.py` (rules engine), `reviewer.py` (interactive review), `reports.py` (P&L, expenses, tax, cashflow, balance), `reconciler.py` (monthly reconciliation)
- **Data flow:** CSV/XLSX import → duplicate detection → auto-categorize via rules → flag unknowns for human review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C line items.
- **Data directory:** `~/Documents/bookkeeper/` (configurable via `BOOKKEEPER_DATA_DIR` env var)

## Commands

```bash
uv sync                                           # Install dependencies
uv run pytest -v                                  # Run all tests
uv run pytest tests/test_importer.py -v           # Run single test file
uv run pytest tests/test_importer.py::test_name   # Run single test
uv run bookkeeper --help                          # CLI help
uv run bookkeeper init                            # Initialize database
uv run bookkeeper import <file> --account <name>  # Import CSV/XLSX
uv run bookkeeper categorize                      # Re-run rules on uncategorized
uv run bookkeeper review                          # Interactive review
uv run bookkeeper report pnl                      # Profit & Loss
uv run bookkeeper report balance                  # Cash position
```

## Key Design Constraints

- All financial modifications require user confirmation — auto-categorizes but never silently changes confirmed data
- Duplicate detection uses file checksums (imports table) and transaction-level matching (date + amount + description + account)
- Rules are ordered by priority DESC; first match wins
- Gusto imports extract only aggregate totals, never individual employee data
- BofA CSV formats vary by account type (checking, credit_card, line_of_credit) — each has its own parser
- Gusto payroll transactions are pre-categorized on import (Wages, Taxes, Benefits)

## Project Structure

```
src/bookkeeper/     # Python package
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
docs/plans/         # Design and implementation docs
```

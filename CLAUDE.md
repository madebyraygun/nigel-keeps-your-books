# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Raygun Bookkeeper — a Claude-powered bookkeeping system replacing QuickBooks for Raygun. Cash-basis, single-entry accounting with bank CSV imports, rules-based categorization, and SQLite storage. See `raygun-bookkeeper-plan.md` for the full system plan.

## Architecture

- **Database:** SQLite (`data/raygun.db`) — tables: accounts, categories, transactions, rules, imports, reconciliations
- **Scripts:** Python scripts in `scripts/` — init_db, import_csv, categorize, review, reports
- **Data flow:** CSV import → duplicate detection → auto-categorize via rules → flag unknowns for human review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C line items.

## Key Design Constraints

- All financial modifications require user confirmation — Claude auto-categorizes but never silently changes confirmed data
- Duplicate detection uses file checksums (imports table) and transaction-level matching (date + amount + description + account)
- Rules are ordered by priority DESC; first match wins
- Gusto imports extract only aggregate totals, never individual employee data
- BofA CSV formats vary by account type — import script must handle variations

## Project Structure

```
data/             # SQLite DB and archived import CSVs
scripts/          # Python CLI scripts
exports/          # Generated reports
```

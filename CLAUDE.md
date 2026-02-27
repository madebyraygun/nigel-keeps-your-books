# CLAUDE.md

**IMPORTANT**: before you do anything else, run the `beans prime` command and heed its output.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Python CLI bookkeeping tool to replace QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Typer app in `src/nigel/cli.py` — subcommands: init, import, categorize, review, reconcile, accounts, rules, report, export
- **Database:** SQLite via `src/nigel/db.py` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations
- **Registry:** `src/nigel/registry.py` — ImporterRegistry with `get_by_key()`, `get_for_file()` (auto-detect via `detect()` callbacks), `get_for_account_type()` (fallback)
- **Plugins:** `src/nigel/plugins.py` — entry-point plugin discovery via `importlib.metadata.entry_points("nigel.plugins")`; hooks for importers, CLI commands, DB migrations, category seeds
- **Modules:** `importer.py` (import_file() + shared helpers; parsers live in plugins), `categorizer.py` (rules engine), `reviewer.py` (interactive review), `reports.py` (P&L, expenses, tax, cashflow, balance), `reconciler.py` (monthly reconciliation)
- **Data flow:** CSV/XLSX import → format auto-detect → duplicate detection → auto-categorize via rules + post_import hooks → flag unknowns for human review → generate reports (terminal or PDF)
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C / Form 1120-S line items via `tax_line` and `form_line` columns.
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
uv run nigel import <file> --account <name>  # Import CSV/XLSX (auto-detects format)
uv run nigel import <file> --account <name> --format bofa_checking  # Import with explicit format
uv run nigel categorize                      # Re-run rules on uncategorized
uv run nigel review                          # Interactive review
uv run nigel report pnl                      # Profit & Loss
uv run nigel report balance                  # Cash position
uv run nigel report k1-prep --year 2025      # K-1 prep worksheet (requires nigel-k1 plugin)
uv run nigel k1 setup                        # Configure entity/shareholders (nigel-k1 plugin)
uv run nigel export pnl --year 2025          # Export P&L to PDF (requires nigel-export-pdf plugin)
uv run nigel export all --year 2025          # Export all reports to PDF
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
- Bank CSV formats vary by account type (checking, credit_card, line_of_credit) — each has its own parser in the nigel-bofa plugin
- Importer plugins use `detect()` callbacks for format auto-detection; `--format` CLI flag overrides auto-detect
- Post-import hooks (e.g., Gusto payroll auto-categorization) run after row insertion via `ImporterInfo.post_import`
- Plugins discovered via `importlib.metadata.entry_points("nigel.plugins")`; plugin migrations and category seeds run during `nigel init`
- ImporterRegistry manages all parsers; core `importer.py` has no parsers — only `import_file()` and shared helpers
- PDF export uses same data functions as terminal reports (get_pnl, etc.) rendered via Jinja2 + WeasyPrint

## Project Structure

```
src/nigel/          # Python package
  cli.py            # Typer CLI entry point
  db.py             # SQLite schema, connection, category seeding
  models.py         # Dataclasses (ParsedRow, ImporterInfo, Category)
  registry.py       # ImporterRegistry — parser registration
  plugins.py        # Plugin discovery and hooks (PluginHooks, load_plugins)
  settings.py       # Settings management (~/.config/nigel/)
  importer.py       # import_file() + shared helpers (parse_amount, parse_date_mdy, excel_serial_to_date)
  categorizer.py    # Rules engine
  reviewer.py       # Interactive review flow
  reports.py        # Report data functions
  reconciler.py     # Monthly reconciliation
plugins/            # First-party plugin packages
  nigel-bofa/       # BofA CSV importers (checking, credit card, line of credit) with detect()
  nigel-gusto/      # Gusto XLSX payroll importer + auto-categorization post_import hook
  nigel-k1/         # K-1 prep report plugin (1120-S / Schedule K)
  nigel-export-pdf/ # PDF export for all reports via Jinja2 + WeasyPrint
tests/              # pytest tests (synthetic fixtures; institution-specific tests in plugin packages)
```

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Clap derive app in `src/cli/mod.rs` — subcommands are optional; running `nigel` with no arguments launches the interactive dashboard. Subcommands: init, demo, import, categorize, review, reconcile, accounts, rules, report, browse, load, backup, status
- **Database:** SQLite via rusqlite in `src/db.rs` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations, metadata (key-value store for per-database settings like company_name)
- **Importers:** `src/importer.rs` — `ImporterKind` enum dispatch (bofa_checking, bofa_credit_card, bofa_loc, gusto_payroll); each variant implements `detect()` and `parse()`; no plugin registry
- **TUI:** `tui.rs` — shared ratatui helpers (style constants, `money_span`, `wrap_text`, `ReportView` trait, `run_report_view()`) for interactive screens; `browser.rs`, `cli/review.rs`, `cli/report/view.rs`, and `cli/dashboard.rs` use ratatui `Terminal::draw()` render loop
- **Dashboard:** `cli/dashboard.rs` — single-struct state machine with `DashboardScreen` enum; Home screen shows YTD P&L, account balances, monthly income/expense bar chart, and a command chooser menu; inline transitions to Browse and Review screens; merged View/Export picker screen for reports; Import, Rules, Reconcile, Load use terminal-mode (break TUI → run CLI command → return); outer loop re-initializes TUI after each terminal command
- **Reports:** `cli/report/` — unified report command with `--mode view|export`, `--format pdf|text`, and `--output` flags; `mod.rs` dispatches to `view.rs` (interactive ratatui views), `text.rs` (comfy_table formatting), or `export.rs` (PDF export); non-TTY automatically falls back to plain text stdout
- **Modules:** `categorizer.rs` (rules engine), `reviewer.rs` (review data layer), `reports.rs` (P&L, expenses, tax, cashflow, balance, flagged, register, K-1 prep), `browser.rs` (interactive register browser via ratatui with row selection, inline category/vendor editing, flag toggling, scroll navigation, and text wrapping), `reconciler.rs` (monthly reconciliation), `pdf.rs` (PDF rendering via printpdf, feature-gated)
- **Data flow:** CSV/XLSX import → automatic pre-import DB snapshot (`<data_dir>/snapshots/`) → format auto-detect via `ImporterKind::detect()` → duplicate detection → auto-categorize via rules → flag unknowns for review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C / Form 1120-S line items via `tax_line` and `form_line` columns.
- **Settings:** `~/.config/nigel/settings.json` — stores `data_dir`, `user_name`, `fiscal_year_start`; `nigel load` switches between existing data directories without reinitializing. Per-database settings (e.g. `company_name`) are stored in the `metadata` table.
- **Onboarding:** `cli/onboarding.rs` — full-screen TUI shown on first launch (when settings.json doesn't exist); collects user name and business name, then offers demo/fresh/load options
- **Data directory:** `~/Documents/nigel/` by default, configurable via `nigel init --data-dir`; switch with `nigel load <path>`. Contains `backups/` (manual backups) and `snapshots/` (automatic pre-import snapshots)
- **Demo:** `nigel demo` inserts 44 sample transactions + 9 rules directly into the DB (no CSV files), then runs categorization

## Commands

```bash
cargo build                                       # Debug build
cargo build --release                             # Release build
cargo test                                        # Run all tests
cargo test --no-default-features                  # Test without gusto/pdf features
nigel                                             # Interactive dashboard (default)
nigel --help                                      # CLI help
nigel init                                        # Initialize (prompts for data dir on first run)
nigel init --data-dir ~/my-books                  # Initialize with custom data dir
nigel demo                                        # Load sample data to explore
nigel import <file> --account <name>              # Import CSV/XLSX (auto-detects format)
nigel import <file> --account <name> --format bofa_checking  # Import with explicit format
nigel rules update 1 --priority 10                # Update a rule field
nigel rules update 5 --category "Rent / Lease"    # Reassign rule category
nigel rules delete 3                              # Deactivate a rule (soft-delete)
nigel categorize                                  # Re-run rules on uncategorized
nigel review                                      # Interactive review
nigel review --id 185                             # Re-review a specific transaction by ID
nigel report pnl --year 2025                      # Interactive view (ratatui)
nigel report expenses --month 2025-03             # Expense breakdown
nigel report tax --year 2025                      # Tax summary
nigel report cashflow                             # Cash flow
nigel report balance                              # Cash position
nigel report register --year 2025                 # Interactive register browser
nigel report register --account "BofA Checking"   # Filter by account
nigel report flagged                              # Flagged transactions
nigel report k1 --year 2025                       # K-1 prep worksheet (1120-S)
nigel report pnl --year 2025 --mode export        # Export as PDF
nigel report pnl --year 2025 --mode export --format text  # Export as text file
nigel report pnl --year 2025 --output ~/report.pdf  # --output implies export
nigel report all --year 2025                      # Bulk export all reports (PDF)
nigel report all --year 2025 --format text        # Bulk export as text files
nigel report all --year 2025 --output-dir ~/exports/  # Custom output directory
nigel browse register                            # All transactions, starts at today
nigel browse register --year 2025                 # Filter to a specific year
nigel browse register --account "BofA Checking"   # Browse filtered by account
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67
nigel status                                      # Show active DB and summary stats
nigel load ~/other-books                          # Switch to a different data directory
nigel backup                                      # Back up DB to <data_dir>/backups/
nigel backup --output /tmp/nigel-backup.db        # Back up to custom path
```

## Documentation Policy

Every feature change must update the relevant documentation before the work is considered complete:

- **CLAUDE.md** — update Architecture, Commands, Project Structure, and Key Design Constraints sections when adding/changing CLI commands, modules, data flow, or settings
- **README.md** — update Quick Start, Features, and Configuration sections for user-facing changes

Do not merge or mark work complete if docs are stale.

## Key Design Constraints

- All financial modifications require user confirmation — auto-categorizes but never silently changes confirmed data
- Interactive review supports back navigation: Esc goes back to re-review the previous transaction (undoing its categorization and any created rule), Tab skips forward
- Duplicate detection uses file checksums (imports table) and transaction-level matching (date + amount + description + account)
- Rules are ordered by priority DESC; first match wins
- Gusto imports extract only aggregate totals, never individual employee data
- Bank CSV formats vary by account type (checking, credit_card, line_of_credit) — each has its own variant in `ImporterKind`
- `ImporterKind::detect()` inspects file headers for format auto-detection; `--format` CLI flag overrides auto-detect
- Demo data is inserted directly into the DB (no CSV files); idempotency guard checks for existing account
- Cash amounts are plain `f64` — negative = expense, positive = income
- Date filters `--from`/`--to` must be supplied as a pair; providing only one is a hard error
- Browse register and reports with no date flags show all transactions (no implicit year filter); the browse view scrolls to today on load
- Database row deserialization errors are propagated, never silently discarded

## Project Structure

```
src/
  main.rs               # Entry point, command dispatch
  cli/                  # CLI subcommands
    mod.rs              # Clap structs (Cli, Commands, subcommands), shared helpers
    dashboard.rs        # nigel (no args) — interactive dashboard with inline screen transitions
    init.rs             # nigel init
    demo.rs             # nigel demo (sample data + setup_demo for isolated demo DB)
    onboarding.rs       # First-run onboarding TUI (animated logo, name collection, action picker)
    accounts.rs         # nigel accounts add/list
    import.rs           # nigel import
    categorize.rs       # nigel categorize
    rules.rs            # nigel rules add/list
    review.rs           # nigel review
    report/             # nigel report (unified view/export command)
      mod.rs            # Dispatch: view vs export, TTY detection, text export
      text.rs           # comfy_table text formatters (used for stdout + text file export)
      view.rs           # Ratatui interactive report views (scrollable, colored)
    browse.rs           # nigel browse (interactive browsers)
    export.rs           # PDF export helpers (per-function feature-gated behind "pdf")
    reconcile.rs        # nigel reconcile
    load.rs             # nigel load (switch data directory)
    backup.rs           # nigel backup (database backup)
    status.rs           # nigel status (show active DB + stats)
  db.rs                 # SQLite schema, connection, category seeding
  models.rs             # Structs (Account, Transaction, Rule, ParsedRow, etc.)
  importer.rs           # ImporterKind enum, format detection, CSV/XLSX parsing
  categorizer.rs        # Rules engine (categorize_transactions)
  reviewer.rs           # Interactive review flow
  reports.rs            # Report data functions (pnl, expenses, tax, cashflow, balance, flagged, k1_prep)
  browser.rs            # Interactive register browser (ratatui, row selection, inline editing, flag toggle, scroll navigation)
  tui.rs                # Shared ratatui helpers (styles, money_span, wrap_text, ReportView trait, run_report_view)
  pdf.rs                # PDF rendering engine (feature-gated behind "pdf")
  reconciler.rs         # Monthly reconciliation
  settings.rs           # Settings management (~/.config/nigel/)
  fmt.rs                # Number formatting helpers
  error.rs              # Error types
docs/
  walkthrough.md        # Guided tour using demo data
  skills.md             # Claude skills documentation
```

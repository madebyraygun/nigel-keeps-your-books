# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Clap derive app in `src/cli/mod.rs` — subcommands are optional; running `nigel` with no arguments launches the interactive dashboard. Subcommands: init, demo, import, undo, categorize, review, reconcile, accounts, categories, rules, report, browse, load, backup, restore, status, password, completions
- **Database:** SQLite via rusqlite (bundled-sqlcipher) in `src/db.rs` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations, metadata (key-value store for per-database settings like company_name). Optional SQLCipher encryption via `PRAGMA key`; password stored in runtime global `Mutex<Option<String>>` (`set_db_password`/`get_db_password`); `get_connection()` reads it internally so zero call-site changes needed; `open_connection()` for explicit password; `is_encrypted()` probes a DB file; `prompt_password_if_needed()` prompts via rpassword with 3 retries
- **Importers:** `src/importer.rs` — `ImporterKind` enum dispatch (bofa_checking, bofa_credit_card, bofa_line_of_credit, gusto_payroll); each variant implements `detect()` and `parse()`; `GenericCsvConfig` supports user-defined column mappings stored as profiles in `csv_profiles` table; malformed CSV rows are counted and reported in import output
- **TUI:** `tui.rs` — shared ratatui helpers (style constants, `money_span`, `wrap_text`, `ReportView` trait with `date_params()`, `run_report_view()`) for interactive screens; `ReportViewAction` enum includes `Continue`, `Close`, and `Reload` (for date navigation); `browser.rs`, `cli/review.rs`, `cli/report/view.rs`, and `cli/dashboard.rs` use ratatui `Terminal::draw()` render loop
- **Dashboard:** `cli/dashboard.rs` — single-struct state machine with `DashboardScreen` enum; Home screen shows YTD P&L, account balances, monthly income/expense bar chart, and a command chooser menu with single-key shortcuts (b=Browse, i=Import, r=Review, c=Reconcile, a=Accounts, t=caTegorize, u=rUles, z=Undo, v=View report, e=Export report, l=Load, p=Settings, s=Snake); all commands render as inline TUI screens; outer loop only re-initializes when Load changes the data directory. F5 refreshes dashboard data.
- **Account Manager:** `cli/account_manager.rs` — inline TUI screen for managing accounts (list, add, rename, delete); uses form sub-screens for add/rename with text input and type selector; delete blocks if account has transactions
- **Category Manager:** `cli/category_manager.rs` — inline TUI screen for managing the chart of accounts (categories); list/add/edit/delete with form sub-screens for name, type (income/expense selector), tax line, and form line; soft-delete blocked if category has transactions or active rules; data layer in `cli/categories.rs`
- **Rules Manager:** `cli/rules_manager.rs` — inline TUI screen for viewing and deleting categorization rules; scrollable list with soft-delete confirmation
- **Import Screen:** `cli/import_manager.rs` — inline TUI form for importing bank statements; file path input + account selector; runs import + auto-categorization and shows results
- **Undo Screen:** `cli/undo_manager.rs` — inline TUI screen for undoing the last import; shows import details (filename, account, date, transaction count) and confirms before deleting; data layer in `cli/undo.rs`
- **Reconcile Screen:** `cli/reconcile_manager.rs` — inline TUI form for account reconciliation; account selector + month/balance input; shows reconciled/discrepancy result
- **Load Screen:** `cli/load_manager.rs` — inline TUI form for switching data directories; validates path and triggers dashboard reload
- **Reports:** `cli/report/` — unified report command with `--mode view|export`, `--format pdf|text`, and `--output` flags; `mod.rs` dispatches to `view.rs` (interactive ratatui views), `text.rs` (comfy_table formatting), or `export.rs` (PDF export); non-TTY automatically falls back to plain text stdout. `TableReportView` supports interactive date navigation: Left/Right arrows page between periods, `m` toggles month/year granularity; each report declares its `DateGranularity` (MonthAndYear, YearOnly, or None)
- **Effects:** `effects.rs` — shared pastel rainbow gradient palette, `gradient_color()` interpolation, `Particle` struct with `new()`/`seeded()`/`tick()`/`is_dead()`, `pre_seed_particles()`, and `tick_particles()` helpers; used by splash, onboarding, and snake screens
- **Splash:** `cli/splash.rs` — 1.5-second splash screen shown on app launch (skipped during first-run onboarding); displays Nigel ASCII logo with rainbow gradient text and pre-seeded floating particle background; dismissable by any keypress
- **Modules:** `categorizer.rs` (rules engine), `reviewer.rs` (review data layer), `reports.rs` (P&L, expenses, tax, cashflow, balance, flagged, register, K-1 prep), `browser.rs` (interactive register browser via ratatui with row selection, inline category/vendor editing, flag toggling, scroll navigation, text wrapping, and incremental text search), `reconciler.rs` (monthly reconciliation), `pdf.rs` (PDF rendering via printpdf, feature-gated)
- **Migrations:** `migrations.rs` — sequential schema migration runner; `MIGRATIONS` array of `(version, description, up_fn)`; runs inside `init_db()` after table creation; each migration executes in a savepoint transaction; version tracked in `metadata` table under `schema_version` key; v1 is the no-op baseline for existing 0.1.x databases; v2 adds `csv_profiles` table for generic CSV column mappings
- **Data flow:** CSV/XLSX import → automatic pre-import DB snapshot (`<data_dir>/snapshots/`) → format auto-detect via `ImporterKind::detect()` → duplicate detection → auto-categorize via rules → flag unknowns for review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C / Form 1120-S line items via `tax_line` and `form_line` columns.
- **Settings:** `~/.config/nigel/settings.json` — stores `data_dir`, `user_name`; `nigel load` switches between existing data directories without reinitializing. Per-database settings (e.g. `company_name`) are stored in the `metadata` table. Database password is runtime-only (never persisted to disk).
- **Settings Manager:** `cli/settings_manager.rs` — inline TUI screen for managing app settings; shows editable business name (saved to DB metadata as `company_name`) and password management; password sub-screen delegates to `PasswordManager`
- **Password Manager:** `cli/password_manager.rs` — TUI screen for managing database encryption; detects current encryption state and shows set/change/remove options; masked password input with confirmation; used as sub-screen within Settings Manager
- **Onboarding:** `cli/onboarding.rs` — full-screen TUI shown on first launch (when settings.json doesn't exist); collects user name, business name, and optional password (masked input), then offers demo/fresh/load options
- **Data directory:** `~/Documents/nigel/` by default, configurable via `nigel init --data-dir`; switch with `nigel load <path>`. Contains `backups/` (manual backups) and `snapshots/` (automatic pre-import snapshots)
- **Demo:** `nigel demo` dynamically generates 18 months of sample transactions (counting backwards from the current date) + 9 rules directly into the DB (no CSV files), then runs categorization; dates are computed at runtime so reports always show current-year data

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
nigel import <file> --account <name> --dry-run           # Preview without importing
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3  # Generic CSV
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3 --save-profile chase  # Save profile
nigel import <file> --account <name> --format chase      # Use saved profile
nigel undo                                        # Undo the last import (with confirmation)
nigel accounts rename 1 "New Name"                # Rename account by ID
nigel accounts delete 3                           # Delete account by ID (blocked if has transactions)
nigel categories list                             # List all categories
nigel categories add "Consulting" --type income   # Add a category
nigel categories rename 5 "Professional Fees"     # Rename a category
nigel categories update 5 "Fees" --type income --tax-line "Gross receipts"  # Update all fields
nigel categories delete 30                        # Soft-delete a category
nigel rules test "ADOBE" --match-type contains    # Test pattern against transactions (dry run)
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
nigel restore ~/backups/nigel-20250301-120000.db  # Restore from a backup file
nigel password set                                # Encrypt an unencrypted database
nigel password change                             # Change password on encrypted database
nigel password remove                             # Decrypt database (remove password)
nigel completions bash                            # Generate shell completions (bash, zsh, fish, powershell)
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
- Demo data is generated dynamically (18 months of transactions counting back from today) and inserted directly into the DB (no CSV files); idempotency guard checks for existing account
- Cash amounts are plain `f64` — negative = expense, positive = income. This is a known precision limitation: `f64` is not suitable for sub-cent accuracy, but is acceptable for the cash-basis bookkeeping use case where all amounts are rounded to cents on import
- Date filters `--from`/`--to` must be supplied as a pair; providing only one is a hard error
- Browse register and reports with no date flags show all transactions (no implicit year filter); the browse view scrolls to today on load
- Database row deserialization errors are propagated, never silently discarded
- Database password is never persisted to disk — stored only in runtime `Mutex<Option<String>>` and prompted via rpassword on each launch
- Demo databases are always unencrypted; `init` and `demo` subcommands skip password detection
- Backups and snapshots preserve the encryption state of the source database
- Cross-encryption-state operations (encrypt/decrypt) use `sqlcipher_export` via ATTACH DATABASE; same-encryption operations (backup, rekey) use SQLite backup API or `PRAGMA rekey`
- Schema migrations run on every `init_db()` call; each migration is transactional (savepoint); to add a migration: append to `MIGRATIONS` array in `migrations.rs`, bump `LATEST_VERSION`, implement `up()` function with SQL statements
- Generic CSV profiles are stored in `csv_profiles` table; `--format <name>` resolves built-in importers first, then csv_profiles; generic CSV is never auto-detected
- `--dry-run` skips snapshot creation, imports table insertion, and transaction insertion; still runs full parse and duplicate detection

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
    account_manager.rs  # TUI account management screen (list, add, rename, delete)
    accounts.rs         # nigel accounts add/list/rename/delete + data-layer functions for TUI
    categories.rs       # nigel categories list/add/rename/delete + data-layer functions for TUI
    category_manager.rs # TUI category management screen (list, add, edit, delete)
    import.rs           # nigel import
    import_manager.rs   # TUI import screen (file path + account selector + result)
    undo.rs             # nigel undo (undo last import, data-layer + CLI)
    undo_manager.rs     # TUI undo screen (confirm + execute from dashboard)
    categorize.rs       # nigel categorize
    rules.rs            # nigel rules add/list/update/delete/test
    rules_manager.rs    # TUI rules screen (scrollable list + delete)
    password.rs         # nigel password set/change/remove (encrypt/decrypt/rekey)
    password_manager.rs # TUI password management screen (set/change/remove via settings)
    settings_manager.rs # TUI settings screen (business name + password management)
    reconcile_manager.rs # TUI reconcile screen (account/month/balance form + result)
    load_manager.rs     # TUI load screen (data directory switcher with reload)
    review.rs           # nigel review
    report/             # nigel report (unified view/export command)
      mod.rs            # Dispatch: view vs export, TTY detection, text export
      text.rs           # comfy_table text formatters (used for stdout + text file export)
      view.rs           # Ratatui interactive report views (scrollable, colored)
    browse.rs           # nigel browse (interactive browsers)
    snake.rs            # Snake game easter egg (ratatui, accessible from dashboard)
    splash.rs           # Splash screen (1.5s animated logo + particles, shown on launch)
    export.rs           # PDF export helpers (per-function feature-gated behind "pdf")
    reconcile.rs        # nigel reconcile
    load.rs             # nigel load (switch data directory)
    backup.rs           # nigel backup (database backup)
    restore.rs          # nigel restore (restore database from backup)
    status.rs           # nigel status (show active DB + stats)
  db.rs                 # SQLite schema, connection, category seeding
  migrations.rs          # Schema migration runner (version tracking, sequential up() functions)
  models.rs             # Structs (Account, Transaction, Rule, ParsedRow, etc.)
  importer.rs           # ImporterKind enum, format detection, CSV/XLSX parsing
  categorizer.rs        # Rules engine (categorize_transactions)
  reviewer.rs           # Interactive review flow
  reports.rs            # Report data functions (pnl, expenses, tax, cashflow, balance, flagged, k1_prep)
  browser.rs            # Interactive register browser (ratatui, row selection, inline editing, flag toggle, scroll navigation)
  effects.rs            # Shared gradient/particle effects (used by splash, onboarding, snake)
  tui.rs                # Shared ratatui helpers (styles, money_span, wrap_text, ReportView trait, run_report_view)
  pdf.rs                # PDF rendering engine (feature-gated behind "pdf")
  reconciler.rs         # Monthly reconciliation
  settings.rs           # Settings management (~/.config/nigel/)
  fmt.rs                # Number formatting helpers
  error.rs              # Error types
docs/
  importers.md          # Importer format specifications and authoring guide
  walkthrough.md        # Guided tour using demo data
  skills.md             # Claude skills documentation
```

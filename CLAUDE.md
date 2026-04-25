# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **CLI:** Clap derive app in `src/cli/mod.rs` — subcommands are optional; running `nigel` with no arguments launches the interactive dashboard. Subcommands: init, demo, import, undo, categorize, review, reconcile, accounts, categories, rules, report, browse, load, backup, restore, status, password, update, completions
- **Database:** SQLite via rusqlite (bundled-sqlcipher) in `src/db.rs` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations, metadata (key-value store for per-database settings like company_name). Optional SQLCipher encryption via `PRAGMA key`; password stored in runtime global `Mutex<Option<String>>` (`set_db_password`/`get_db_password`); `get_connection()` reads it internally so zero call-site changes needed; `open_connection()` for explicit password; `is_encrypted()` probes a DB file; `validate_password()` tests a password without side effects; `prompt_password_if_needed()` prompts via rpassword with 3 retries (used by CLI subcommands)
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
- **Effects:** `effects.rs` — shared pastel rainbow gradient palette, `gradient_color()` interpolation, `Particle` struct with `new()`/`seeded()`/`tick()`/`is_dead()`, `pre_seed_particles()`, and `tick_particles()` helpers; used by splash, goodbye, onboarding, and snake screens
- **Splash:** `cli/splash.rs` — 1.5-second splash screen shown on app launch (skipped during first-run onboarding); displays Nigel ASCII logo with rainbow gradient text and pre-seeded floating particle background; dismissable by any keypress. For encrypted databases, the splash holds indefinitely (no auto-fade) and displays an inline masked password input below the logo; supports up to 3 attempts with error feedback; `run()` for unencrypted, `run_with_password(db_path)` for encrypted
- **Goodbye:** `cli/goodbye.rs` — 1.2-second farewell screen shown when quitting the dashboard; displays Nigel ASCII logo with "Goodbye!" text, plays the reverse of the splash reveal animation (characters disappear), with particle background; dismissable by any keypress
- **Updater:** `cli/update.rs` — `nigel update` command and launch-time version check; queries GitHub Releases API for latest version, compares via `semver`, downloads correct platform binary, and self-replaces via `self_replace` crate; `check_and_notify()` runs on launch with 24-hour cooldown (stored in `last_update_check` in settings.json); opt-out via `update_check: false` in settings; dashboard shows yellow notification bar; CLI prints to stderr
- **Settings Manager:** `cli/settings_manager.rs` — inline TUI screen for managing app settings; shows editable business name (saved to DB metadata as `company_name`), password management, and auto-update check toggle; password sub-screen delegates to `PasswordManager`
- **Modules:** `categorizer.rs` (rules engine), `reviewer.rs` (review data layer), `reports.rs` (P&L, expenses, tax, cashflow, balance, flagged, register, K-1 prep), `browser.rs` (interactive register browser via ratatui with row selection, inline category/vendor editing, flag toggling, scroll navigation, text wrapping, and incremental text search), `reconciler.rs` (monthly reconciliation), `pdf.rs` (PDF rendering via printpdf, feature-gated)
- **Migrations:** `migrations.rs` — sequential schema migration runner; `MIGRATIONS` array of `(version, description, up_fn)`; runs inside `init_db()` after table creation; each migration executes in a savepoint transaction; version tracked in `metadata` table under `schema_version` key; v1 is the no-op baseline for existing 0.1.x databases; v2 adds `csv_profiles` table for generic CSV column mappings
- **Data flow:** CSV/XLSX import → automatic pre-import DB snapshot (`<data_dir>/snapshots/`) → format auto-detect via `ImporterKind::detect()` → duplicate detection → auto-categorize via rules → flag unknowns for review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C / Form 1120-S line items via `tax_line` and `form_line` columns.
- **Settings:** `~/.config/nigel/settings.json` — stores `data_dir`, `user_name`, `update_check` (bool, default true), `last_update_check` (ISO 8601 timestamp); `nigel load` switches between existing data directories without reinitializing. Per-database settings (e.g. `company_name`) are stored in the `metadata` table. Database password is runtime-only (never persisted to disk).
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
nigel update                                      # Check for and install the latest version
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
- Database password is never persisted to disk — stored only in runtime `Mutex<Option<String>>`; for the dashboard, password is collected inline on the splash screen (TUI masked input); for CLI subcommands, prompted via rpassword
- Demo databases are always unencrypted; `init` and `demo` subcommands skip password detection
- Backups and snapshots preserve the encryption state of the source database
- Cross-encryption-state operations (encrypt/decrypt) use `sqlcipher_export` via ATTACH DATABASE; same-encryption operations (backup, rekey) use SQLite backup API or `PRAGMA rekey`
- Schema migrations run on every `init_db()` call; each migration is transactional (savepoint); to add a migration: append to `MIGRATIONS` array in `migrations.rs`, bump `LATEST_VERSION`, implement `up()` function with SQL statements
- Generic CSV profiles are stored in `csv_profiles` table; `--format <name>` resolves built-in importers first, then csv_profiles; generic CSV is never auto-detected
- `--dry-run` skips snapshot creation, imports table insertion, and transaction insertion; still runs full parse and duplicate detection
- Auto-update check runs once per 24 hours on launch (both dashboard and CLI); respects `update_check: false` in settings.json; silently skips on network failure; `nigel update` command always checks and can be exempt from init/password checks
- Platform binary detection: macOS = `nigel-universal-apple-darwin`, Linux x86_64 = `nigel-x86_64-unknown-linux-gnu`, Windows x86_64 = `nigel-x86_64-pc-windows-msvc.exe`

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
    goodbye.rs          # Goodbye screen (reverse logo animation + particles, shown on quit)
    export.rs           # PDF export helpers (per-function feature-gated behind "pdf")
    reconcile.rs        # nigel reconcile
    load.rs             # nigel load (switch data directory)
    backup.rs           # nigel backup (database backup)
    restore.rs          # nigel restore (restore database from backup)
    status.rs           # nigel status (show active DB + stats)
    update.rs           # nigel update (version check + self-replace from GitHub Releases)
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

<!-- BACKLOG.MD GUIDELINES START -->
# Instructions for the usage of Backlog.md CLI Tool

## Backlog.md: Comprehensive Project Management Tool via CLI

### Assistant Objective

Efficiently manage all project tasks, status, and documentation using the Backlog.md CLI, ensuring all project metadata
remains fully synchronized and up-to-date.

### Core Capabilities

- ✅ **Task Management**: Create, edit, assign, prioritize, and track tasks with full metadata
- ✅ **Search**: Fuzzy search across tasks, documents, and decisions with `backlog search`
- ✅ **Acceptance Criteria**: Granular control with add/remove/check/uncheck by index
- ✅ **Definition of Done checklists**: Per-task DoD items with add/remove/check/uncheck
- ✅ **Board Visualization**: Terminal-based Kanban board (`backlog board`) and web UI (`backlog browser`)
- ✅ **Git Integration**: Automatic tracking of task states across branches
- ✅ **Dependencies**: Task relationships and subtask hierarchies
- ✅ **Documentation & Decisions**: Structured docs and architectural decision records
- ✅ **Export & Reporting**: Generate markdown reports and board snapshots
- ✅ **AI-Optimized**: `--plain` flag provides clean text output for AI processing

### Why This Matters to You (AI Agent)

1. **Comprehensive system** - Full project management capabilities through CLI
2. **The CLI is the interface** - All operations go through `backlog` commands
3. **Unified interaction model** - You can use CLI for both reading (`backlog task 1 --plain`) and writing (
   `backlog task edit 1`)
4. **Metadata stays synchronized** - The CLI handles all the complex relationships

### Key Understanding

- **Tasks** live in `backlog/tasks/` as `task-<id> - <title>.md` files
- **You interact via CLI only**: `backlog task create`, `backlog task edit`, etc.
- **Use `--plain` flag** for AI-friendly output when viewing/listing
- **Never bypass the CLI** - It handles Git, metadata, file naming, and relationships

---

# ⚠️ CRITICAL: NEVER EDIT TASK FILES DIRECTLY. Edit Only via CLI

**ALL task operations MUST use the Backlog.md CLI commands**

- ✅ **DO**: Use `backlog task edit` and other CLI commands
- ✅ **DO**: Use `backlog task create` to create new tasks
- ✅ **DO**: Use `backlog task edit <id> --check-ac <index>` to mark acceptance criteria
- ❌ **DON'T**: Edit markdown files directly
- ❌ **DON'T**: Manually change checkboxes in files
- ❌ **DON'T**: Add or modify text in task files without using CLI

**Why?** Direct file editing breaks metadata synchronization, Git tracking, and task relationships.

---

## 1. Source of Truth & File Structure

### 📖 **UNDERSTANDING** (What you'll see when reading)

- Markdown task files live under **`backlog/tasks/`** (drafts under **`backlog/drafts/`**)
- Files are named: `task-<id> - <title>.md` (e.g., `task-42 - Add GraphQL resolver.md`)
- Project documentation is in **`backlog/docs/`**
- Project decisions are in **`backlog/decisions/`**

### 🔧 **ACTING** (How to change things)

- **All task operations MUST use the Backlog.md CLI tool**
- This ensures metadata is correctly updated and the project stays in sync
- **Always use `--plain` flag** when listing or viewing tasks for AI-friendly text output

---

## 2. Common Mistakes to Avoid

### ❌ **WRONG: Direct File Editing**

```markdown
# DON'T DO THIS:

1. Open backlog/tasks/task-7 - Feature.md in editor
2. Change "- [ ]" to "- [x]" manually
3. Add notes or final summary directly to the file
4. Save the file
```

### ✅ **CORRECT: Using CLI Commands**

```bash
# DO THIS INSTEAD:
backlog task edit 7 --check-ac 1  # Mark AC #1 as complete
backlog task edit 7 --notes "Implementation complete"  # Add notes
backlog task edit 7 --final-summary "PR-style summary"  # Add final summary
backlog task edit 7 -s "In Progress" -a @agent-k  # Multiple commands: change status and assign the task when you start working on the task
```

---

## 3. Understanding Task Format (Read-Only Reference)

⚠️ **FORMAT REFERENCE ONLY** - The following sections show what you'll SEE in task files.
**Never edit these directly! Use CLI commands to make changes.**

### Task Structure You'll See

```markdown
---
id: task-42
title: Add GraphQL resolver
status: To Do
assignee: [@sara]
labels: [backend, api]
---

## Description

Brief explanation of the task purpose.

## Acceptance Criteria

<!-- AC:BEGIN -->

- [ ] #1 First criterion
- [x] #2 Second criterion (completed)
- [ ] #3 Third criterion

<!-- AC:END -->

## Definition of Done

<!-- DOD:BEGIN -->

- [ ] #1 Tests pass
- [ ] #2 Docs updated

<!-- DOD:END -->

## Implementation Plan

1. Research approach
2. Implement solution

## Implementation Notes

Progress notes captured during implementation.

## Final Summary

PR-style summary of what was implemented.
```

### How to Modify Each Section

| What You Want to Change | CLI Command to Use                                       |
|-------------------------|----------------------------------------------------------|
| Title                   | `backlog task edit 42 -t "New Title"`                    |
| Status                  | `backlog task edit 42 -s "In Progress"`                  |
| Assignee                | `backlog task edit 42 -a @sara`                          |
| Labels                  | `backlog task edit 42 -l backend,api`                    |
| Description             | `backlog task edit 42 -d "New description"`              |
| Add AC                  | `backlog task edit 42 --ac "New criterion"`              |
| Add DoD                 | `backlog task edit 42 --dod "Ship notes"`                |
| Check AC #1             | `backlog task edit 42 --check-ac 1`                      |
| Check DoD #1            | `backlog task edit 42 --check-dod 1`                     |
| Uncheck AC #2           | `backlog task edit 42 --uncheck-ac 2`                    |
| Uncheck DoD #2          | `backlog task edit 42 --uncheck-dod 2`                   |
| Remove AC #3            | `backlog task edit 42 --remove-ac 3`                     |
| Remove DoD #3           | `backlog task edit 42 --remove-dod 3`                    |
| Add Plan                | `backlog task edit 42 --plan "1. Step one\n2. Step two"` |
| Add Notes (replace)     | `backlog task edit 42 --notes "What I did"`              |
| Append Notes            | `backlog task edit 42 --append-notes "Another note"` |
| Add Final Summary       | `backlog task edit 42 --final-summary "PR-style summary"` |
| Append Final Summary    | `backlog task edit 42 --append-final-summary "Another detail"` |
| Clear Final Summary     | `backlog task edit 42 --clear-final-summary` |

---

## 4. Defining Tasks

### Creating New Tasks

**Always use CLI to create tasks:**

```bash
# Example
backlog task create "Task title" -d "Description" --ac "First criterion" --ac "Second criterion"
```

### Title (one liner)

Use a clear brief title that summarizes the task.

### Description (The "why")

Provide a concise summary of the task purpose and its goal. Explains the context without implementation details.

### Acceptance Criteria (The "what")

**Understanding the Format:**

- Acceptance criteria appear as numbered checkboxes in the markdown files
- Format: `- [ ] #1 Criterion text` (unchecked) or `- [x] #1 Criterion text` (checked)

**Managing Acceptance Criteria via CLI:**

⚠️ **IMPORTANT: How AC Commands Work**

- **Adding criteria (`--ac`)** accepts multiple flags: `--ac "First" --ac "Second"` ✅
- **Checking/unchecking/removing** accept multiple flags too: `--check-ac 1 --check-ac 2` ✅
- **Mixed operations** work in a single command: `--check-ac 1 --uncheck-ac 2 --remove-ac 3` ✅

```bash
# Examples

# Add new criteria (MULTIPLE values allowed)
backlog task edit 42 --ac "User can login" --ac "Session persists"

# Check specific criteria by index (MULTIPLE values supported)
backlog task edit 42 --check-ac 1 --check-ac 2 --check-ac 3  # Check multiple ACs
# Or check them individually if you prefer:
backlog task edit 42 --check-ac 1    # Mark #1 as complete
backlog task edit 42 --check-ac 2    # Mark #2 as complete

# Mixed operations in single command
backlog task edit 42 --check-ac 1 --uncheck-ac 2 --remove-ac 3

# ❌ STILL WRONG - These formats don't work:
# backlog task edit 42 --check-ac 1,2,3  # No comma-separated values
# backlog task edit 42 --check-ac 1-3    # No ranges
# backlog task edit 42 --check 1         # Wrong flag name

# Multiple operations of same type
backlog task edit 42 --uncheck-ac 1 --uncheck-ac 2  # Uncheck multiple ACs
backlog task edit 42 --remove-ac 2 --remove-ac 4    # Remove multiple ACs (processed high-to-low)
```

### Definition of Done checklist (per-task)

Definition of Done items are a second checklist in each task. Defaults come from `definition_of_done` in the project config file (`backlog/config.yml`, `.backlog/config.yml`, or `backlog.config.yml`) or from Web UI Settings, and can be disabled per task.

**Managing Definition of Done via CLI:**

```bash
# Add DoD items (MULTIPLE values allowed)
backlog task edit 42 --dod "Run tests" --dod "Update docs"

# Check/uncheck DoD items by index (MULTIPLE values supported)
backlog task edit 42 --check-dod 1 --check-dod 2
backlog task edit 42 --uncheck-dod 1

# Remove DoD items by index
backlog task edit 42 --remove-dod 2

# Create without defaults
backlog task create "Feature" --no-dod-defaults
```

**Key Principles for Good ACs:**

- **Outcome-Oriented:** Focus on the result, not the method.
- **Testable/Verifiable:** Each criterion should be objectively testable
- **Clear and Concise:** Unambiguous language
- **Complete:** Collectively cover the task scope
- **User-Focused:** Frame from end-user or system behavior perspective

Good Examples:

- "User can successfully log in with valid credentials"
- "System processes 1000 requests per second without errors"
- "CLI preserves literal newlines in description/plan/notes/final summary; `\\n` sequences are not auto‑converted"

Bad Example (Implementation Step):

- "Add a new function handleLogin() in auth.ts"
- "Define expected behavior and document supported input patterns"

### Task Breakdown Strategy

1. Identify foundational components first
2. Create tasks in dependency order (foundations before features)
3. Ensure each task delivers value independently
4. Avoid creating tasks that block each other

### Task Requirements

- Tasks must be **atomic** and **testable** or **verifiable**
- Each task should represent a single unit of work for one PR
- **Never** reference future tasks (only tasks with id < current task id)
- Ensure tasks are **independent** and don't depend on future work

---

## 5. Implementing Tasks

### 5.1. First step when implementing a task

The very first things you must do when you take over a task are:

* set the task in progress
* assign it to yourself

```bash
# Example
backlog task edit 42 -s "In Progress" -a @{myself}
```

### 5.2. Review Task References and Documentation

Before planning, check if the task has any attached `references` or `documentation`:
- **References**: Related code files, GitHub issues, or URLs relevant to the implementation
- **Documentation**: Design docs, API specs, or other materials for understanding context

These are visible in the task view output. Review them to understand the full context before drafting your plan.

### 5.3. Create an Implementation Plan (The "how")

Previously created tasks contain the why and the what. Once you are familiar with that part you should think about a
plan on **HOW** to tackle the task and all its acceptance criteria. This is your **Implementation Plan**.
First do a quick check to see if all the tools that you are planning to use are available in the environment you are
working in.
When you are ready, write it down in the task so that you can refer to it later.

```bash
# Example
backlog task edit 42 --plan "1. Research codebase for references\n2Research on internet for similar cases\n3. Implement\n4. Test"
```

## 5.4. Implementation

Once you have a plan, you can start implementing the task. This is where you write code, run tests, and make sure
everything works as expected. Follow the acceptance criteria one by one and MARK THEM AS COMPLETE as soon as you
finish them.

### 5.5 Implementation Notes (Progress log)

Use Implementation Notes to log progress, decisions, and blockers as you work.
Append notes progressively during implementation using `--append-notes`:

```
backlog task edit 42 --append-notes "Investigated root cause" --append-notes "Added tests for edge case"
```

```bash
# Example
backlog task edit 42 --notes "Initial implementation done; pending integration tests"
```

### 5.6 Final Summary (PR description)

When you are done implementing a task you need to prepare a PR description for it.
Because you cannot create PRs directly, write the PR as a clean summary in the Final Summary field.

**Quality bar:** Write it like a reviewer will see it. A one‑liner is rarely enough unless the change is truly trivial.
Include the key scope so someone can understand the impact without reading the whole diff.

```bash
# Example
backlog task edit 42 --final-summary "Implemented pattern X because Reason Y; updated files Z and W; added tests"
```

**IMPORTANT**: Do NOT include an Implementation Plan when creating a task. The plan is added only after you start the
implementation.

- Creation phase: provide Title, Description, Acceptance Criteria, and optionally labels/priority/assignee.
- When you begin work, switch to edit, set the task in progress and assign to yourself
  `backlog task edit <id> -s "In Progress" -a "..."`.
- Think about how you would solve the task and add the plan: `backlog task edit <id> --plan "..."`.
- After updating the plan, share it with the user and ask for confirmation. Do not begin coding until the user approves the plan or explicitly tells you to skip the review.
- Append Implementation Notes during implementation using `--append-notes` as progress is made.
- Add Final Summary only after completing the work: `backlog task edit <id> --final-summary "..."` (replace) or append using `--append-final-summary`.

## Phase discipline: What goes where

- Creation: Title, Description, Acceptance Criteria, labels/priority/assignee.
- Implementation: Implementation Plan (after moving to In Progress and assigning to yourself) + Implementation Notes (progress log, appended as you work).
- Wrap-up: Final Summary (PR description), verify AC and Definition of Done checks.

**IMPORTANT**: Only implement what's in the Acceptance Criteria. If you need to do more, either:

1. Update the AC first: `backlog task edit 42 --ac "New requirement"`
2. Or create a new follow up task: `backlog task create "Additional feature"`

---

## 6. Typical Workflow

```bash
# 1. Identify work
backlog task list -s "To Do" --plain

# 2. Read task details
backlog task 42 --plain

# 3. Start work: assign yourself & change status
backlog task edit 42 -s "In Progress" -a @myself

# 4. Add implementation plan
backlog task edit 42 --plan "1. Analyze\n2. Refactor\n3. Test"

# 5. Share the plan with the user and wait for approval (do not write code yet)

# 6. Work on the task (write code, test, etc.)

# 7. Mark acceptance criteria as complete (supports multiple in one command)
backlog task edit 42 --check-ac 1 --check-ac 2 --check-ac 3  # Check all at once
# Or check them individually if preferred:
# backlog task edit 42 --check-ac 1
# backlog task edit 42 --check-ac 2
# backlog task edit 42 --check-ac 3

# 8. Add Final Summary (PR Description)
backlog task edit 42 --final-summary "Refactored using strategy pattern, updated tests"

# 9. Mark task as done
backlog task edit 42 -s Done
```

---

## 7. Definition of Done (DoD)

A task is **Done** only when **ALL** of the following are complete:

### ✅ Via CLI Commands:

1. **All acceptance criteria checked**: Use `backlog task edit <id> --check-ac <index>` for each
2. **All Definition of Done items checked**: Use `backlog task edit <id> --check-dod <index>` for each
3. **Final Summary added**: Use `backlog task edit <id> --final-summary "..."`
4. **Status set to Done**: Use `backlog task edit <id> -s Done`

### ✅ Via Code/Testing:

5. **Tests pass**: Run test suite and linting
6. **Documentation updated**: Update relevant docs if needed
7. **Code reviewed**: Self-review your changes
8. **No regressions**: Performance, security checks pass

⚠️ **NEVER mark a task as Done without completing ALL items above**

---

## 8. Finding Tasks and Content with Search

When users ask you to find tasks related to a topic, use the `backlog search` command with `--plain` flag:

```bash
# Search for tasks about authentication
backlog search "auth" --plain

# Search only in tasks (not docs/decisions)
backlog search "login" --type task --plain

# Search with filters
backlog search "api" --status "In Progress" --plain
backlog search "bug" --priority high --plain
```

**Key points:**
- Uses fuzzy matching - finds "authentication" when searching "auth"
- Searches task titles, descriptions, and content
- Also searches documents and decisions unless filtered with `--type task`
- Always use `--plain` flag for AI-readable output

---

## 9. Quick Reference: DO vs DON'T

### Viewing and Finding Tasks

| Task         | ✅ DO                        | ❌ DON'T                         |
|--------------|-----------------------------|---------------------------------|
| View task    | `backlog task 42 --plain`   | Open and read .md file directly |
| List tasks   | `backlog task list --plain` | Browse backlog/tasks folder     |
| Check status | `backlog task 42 --plain`   | Look at file content            |
| Find by topic| `backlog search "auth" --plain` | Manually grep through files |

### Modifying Tasks

| Task          | ✅ DO                                 | ❌ DON'T                           |
|---------------|--------------------------------------|-----------------------------------|
| Check AC      | `backlog task edit 42 --check-ac 1`  | Change `- [ ]` to `- [x]` in file |
| Add notes     | `backlog task edit 42 --notes "..."` | Type notes into .md file          |
| Add final summary | `backlog task edit 42 --final-summary "..."` | Type summary into .md file |
| Change status | `backlog task edit 42 -s Done`       | Edit status in frontmatter        |
| Add AC        | `backlog task edit 42 --ac "New"`    | Add `- [ ] New` to file           |

---

## 10. Complete CLI Command Reference

### Task Creation

| Action           | Command                                                                             |
|------------------|-------------------------------------------------------------------------------------|
| Create task      | `backlog task create "Title"`                                                       |
| With description | `backlog task create "Title" -d "Description"`                                      |
| With AC          | `backlog task create "Title" --ac "Criterion 1" --ac "Criterion 2"`                 |
| With final summary | `backlog task create "Title" --final-summary "PR-style summary"`                 |
| With references  | `backlog task create "Title" --ref src/api.ts --ref https://github.com/issue/123`   |
| With documentation | `backlog task create "Title" --doc https://design-docs.example.com`               |
| With all options | `backlog task create "Title" -d "Desc" -a @sara -s "To Do" -l auth --priority high --ref src/api.ts --doc docs/spec.md` |
| Create draft     | `backlog task create "Title" --draft`                                               |
| Create subtask   | `backlog task create "Title" -p 42`                                                 |

### Task Modification

| Action           | Command                                     |
|------------------|---------------------------------------------|
| Edit title       | `backlog task edit 42 -t "New Title"`       |
| Edit description | `backlog task edit 42 -d "New description"` |
| Change status    | `backlog task edit 42 -s "In Progress"`     |
| Assign           | `backlog task edit 42 -a @sara`             |
| Add labels       | `backlog task edit 42 -l backend,api`       |
| Set priority     | `backlog task edit 42 --priority high`      |

### Acceptance Criteria Management

| Action              | Command                                                                     |
|---------------------|-----------------------------------------------------------------------------|
| Add AC              | `backlog task edit 42 --ac "New criterion" --ac "Another"`                  |
| Remove AC #2        | `backlog task edit 42 --remove-ac 2`                                        |
| Remove multiple ACs | `backlog task edit 42 --remove-ac 2 --remove-ac 4`                          |
| Check AC #1         | `backlog task edit 42 --check-ac 1`                                         |
| Check multiple ACs  | `backlog task edit 42 --check-ac 1 --check-ac 3`                            |
| Uncheck AC #3       | `backlog task edit 42 --uncheck-ac 3`                                       |
| Mixed operations    | `backlog task edit 42 --check-ac 1 --uncheck-ac 2 --remove-ac 3 --ac "New"` |

### Task Content

| Action           | Command                                                  |
|------------------|----------------------------------------------------------|
| Add plan         | `backlog task edit 42 --plan "1. Step one\n2. Step two"` |
| Add notes        | `backlog task edit 42 --notes "Implementation details"`  |
| Add final summary | `backlog task edit 42 --final-summary "PR-style summary"` |
| Append final summary | `backlog task edit 42 --append-final-summary "More details"` |
| Clear final summary | `backlog task edit 42 --clear-final-summary` |
| Add dependencies | `backlog task edit 42 --dep task-1 --dep task-2`         |
| Add references   | `backlog task edit 42 --ref src/api.ts --ref https://github.com/issue/123` |
| Add documentation | `backlog task edit 42 --doc https://design-docs.example.com --doc docs/spec.md` |

### Multi‑line Input (Description/Plan/Notes/Final Summary)

The CLI preserves input literally. Shells do not convert `\n` inside normal quotes. Use one of the following to insert real newlines:

- Bash/Zsh (ANSI‑C quoting):
  - Description: `backlog task edit 42 --desc $'Line1\nLine2\n\nFinal'`
  - Plan: `backlog task edit 42 --plan $'1. A\n2. B'`
  - Notes: `backlog task edit 42 --notes $'Done A\nDoing B'`
  - Append notes: `backlog task edit 42 --append-notes $'Progress update line 1\nLine 2'`
  - Final summary: `backlog task edit 42 --final-summary $'Shipped A\nAdded B'`
  - Append final summary: `backlog task edit 42 --append-final-summary $'Added X\nAdded Y'`
- POSIX portable (printf):
  - `backlog task edit 42 --notes "$(printf 'Line1\nLine2')"`
- PowerShell (backtick n):
  - `backlog task edit 42 --notes "Line1`nLine2"`

Do not expect `"...\n..."` to become a newline. That passes the literal backslash + n to the CLI by design.

Descriptions support literal newlines; shell examples may show escaped `\\n`, but enter a single `\n` to create a newline.

### Implementation Notes Formatting

- Keep implementation notes concise and time-ordered; focus on progress, decisions, and blockers.
- Use short paragraphs or bullet lists instead of a single long line.
- Use Markdown bullets (`-` for unordered, `1.` for ordered) for readability.
- When using CLI flags like `--append-notes`, remember to include explicit
  newlines. Example:

  ```bash
  backlog task edit 42 --append-notes $'- Added new API endpoint\n- Updated tests\n- TODO: monitor staging deploy'
  ```

### Final Summary Formatting

- Treat the Final Summary as a PR description: lead with the outcome, then add key changes and tests.
- Keep it clean and structured so it can be pasted directly into GitHub.
- Prefer short paragraphs or bullet lists and avoid raw progress logs.
- Aim to cover: **what changed**, **why**, **user impact**, **tests run**, and **risks/follow‑ups** when relevant.
- Avoid single‑line summaries unless the change is truly tiny.

**Example (good, not rigid):**
```
Added Final Summary support across CLI/MCP/Web/TUI to separate PR summaries from progress notes.

Changes:
- Added `finalSummary` to task types and markdown section parsing/serialization (ordered after notes).
- CLI/MCP/Web/TUI now render and edit Final Summary; plain output includes it.

Tests:
- bun test src/test/final-summary.test.ts
- bun test src/test/cli-final-summary.test.ts
```

### Task Operations

| Action             | Command                                      |
|--------------------|----------------------------------------------|
| View task          | `backlog task 42 --plain`                    |
| List tasks         | `backlog task list --plain`                  |
| Search tasks       | `backlog search "topic" --plain`              |
| Search with filter | `backlog search "api" --status "To Do" --plain` |
| Filter by status   | `backlog task list -s "In Progress" --plain` |
| Filter by assignee | `backlog task list -a @sara --plain`         |
| Archive task       | `backlog task archive 42`                    |
| Demote to draft    | `backlog task demote 42`                     |

---

## Common Issues

| Problem              | Solution                                                           |
|----------------------|--------------------------------------------------------------------|
| Task not found       | Check task ID with `backlog task list --plain`                     |
| AC won't check       | Use correct index: `backlog task 42 --plain` to see AC numbers     |
| Changes not saving   | Ensure you're using CLI, not editing files                         |
| Metadata out of sync | Re-edit via CLI to fix: `backlog task edit 42 -s <current-status>` |

---

## Remember: The Golden Rule

**🎯 If you want to change ANYTHING in a task, use the `backlog task edit` command.**
**📖 Use CLI to read tasks, exceptionally READ task files directly, never WRITE to them.**

Full help available: `backlog --help`

<!-- BACKLOG.MD GUIDELINES END -->

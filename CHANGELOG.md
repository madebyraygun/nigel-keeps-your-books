# Changelog

## [Unreleased]

### Changed
- **Report exports moved into the report viewer** — `e` (PDF) and `t` (text) export exactly the period on screen, so any year or month you can navigate to can be exported, not just the current year
- **Single "View Reports" picker** — the standalone export report/format screens are gone; the picker's last entry, "Export All Reports", exports every report at once (Enter for PDF, `t` for text)
- **Register exports from the dashboard's browser** — picking "Transaction Register" opens the interactive browser, where `x` (PDF) and `t` (text) export every transaction on screen; the register exports over an open date range instead of being clipped to the current year. `nigel browse register` is unchanged and binds neither key

## [1.0.1] - 2026-08-05

### Fixed
- **Missing K-1 worksheet income** — income categories in the stock chart of accounts carried no `form_line` mapping, which prevented them from showing up on the K1 report. Income categories without a `form_line` now count toward gross receipts automatically, flagged with an `(auto)` note
- **K-1 meals limit applied inconsistently** — the headline Total Deductions now uses the 50%-limited meals figure, matching the Other Deductions sub-table
- **Clippy `collapsible_match` failures blocking CI** — four match-arm `if`s collapsed into match guards

### Added
- **Needs-mapping section on the K-1 worksheet** — expense categories with activity but no recognized `form_line` are listed with their totals instead of being silently excluded; a reserved `excluded` value marks categories deliberately outside the return (e.g. transfers)
- **`form_line` vocabulary** — `1120S-1a` (gross receipts), `1120S-2` (cost of goods sold), `1120S-5` (other income), alongside the existing `1120S-N`, `K-N`, and new `excluded` values; a schema migration backfills the stock chart-of-accounts categories

### Changed
- **Schema migrations run before any data-bearing command** — existing databases pick up migrations during normal use instead of only on `init`/`demo`/`restore`. The first command after upgrading prints a one-line migration notice; encrypted databases prompt for the password as usual

## [1.0.0] - 2026-03-02

### Added
- **Interactive TUI dashboard** — running `nigel` with no arguments launches a full-screen dashboard with YTD P&L, account balances, monthly income/expense bar chart, and single-key command menu
- **First-run onboarding** — guided setup screen with animated logo, collects user name, business name, optional password, and offers demo/fresh/load options
- **Splash screen** — rainbow gradient ASCII logo with floating particle effects on launch
- **Goodbye screen** — reverse logo animation with "Goodbye!" text and particle effects on dashboard quit
- **Database encryption** — SQLCipher encryption with `nigel password set/change/remove` commands; password prompted at launch, never persisted to disk
- **Schema migration system** — sequential versioned migrations run automatically on startup with savepoint transactions
- **Import enhancements** — `--dry-run` preview mode, malformed row tracking, generic CSV format auto-detection
- **`nigel undo`** — rolls back the last import by removing its transactions and import record
- **`nigel restore`** — recovers a database from a backup file
- **Interactive register browser** — scrollable transaction list with inline category/vendor editing, flag toggling, and text search (`/` to search)
- **Unified report command** — `nigel report <type>` with `--mode view|export`, `--format pdf|text`, and `--output` flags; interactive ratatui views with date navigation (Left/Right arrows, `m` toggles month/year)
- **Settings screen** — manage application settings from the dashboard TUI
- **Editable chart of accounts** — `nigel categories add/rename/update/delete` with TUI management screen
- **Account management** — `nigel accounts add/rename/delete` with TUI screen; delete blocked if account has transactions
- **`nigel rules test`** — dry-run pattern matching against existing transactions
- **Shell completions** — `nigel completions bash|zsh|fish|powershell`
- **Back navigation in review** — Esc undoes previous categorization, Tab skips forward
- **`nigel review --id`** — re-review a specific transaction by ID
- **`nigel rules delete`** — soft-delete categorization rules
- **Business name header** in text file exports
- **Page title headers** on all TUI screens
- **Keyboard shortcuts** on dashboard menu items
- **Snake game** easter egg accessible from dashboard
- **Version display** — version number shown at the bottom center of splash and onboarding screens
- **GitHub Actions CI** workflow
- **Integration tests** for CLI dispatch paths

### Changed
- Export picker shows format selection step (PDF / Text) instead of defaulting to PDF
- Demo transactions generated dynamically (18 months from current date) instead of hardcoded dates
- Browse register shows all transactions by default (no implicit year filter)
- Review screen migrated to ratatui from raw crossterm
- BofA importers refactored to share parsing helpers

### Fixed
- Splash screen no longer dissolves out before transitioning to dashboard — logo stays solid after reveal
- BofA CSV parsing when cardholder names contain commas
- Scroll-to-today bounds in short terminals
- Report parameter panics on empty date filters
- DB reliability: `last_insert_rowid()` correctness and SQLite busy timeout
- Import: `import_id` now populated on transactions
- Importer safety: `parse_amount` returns `Option`, streaming credit card detection
- Report bugs: `fiscal_year_start`, cashflow balance, K-1 sign corrections
- `account_names()` no longer silently discards errors
- Demo data balanced for realistic income/expense ratios
- Hardened error handling, file permissions, and first-run messages
- Migration edge cases and password trim warnings
- Compiler warnings without default features
- Date filter error handling and result collection

## [0.1.1] - 2026-02-27

### Added
- **Transaction register report** — `nigel report register` and `nigel export register` show all transactions for a date period with category, vendor, and account details. Supports `--year`, `--month`, `--from`/`--to`, and `--account` filters. Included in `nigel export all`.

### Fixed
- PDF table layout: header separator lines no longer overlap first data row text
- PDF spacing: tighter header-to-line gap, better separation between data rows and totals

## [0.1.0] - 2026-02-25

Initial release.

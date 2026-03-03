# Changelog

## [1.0.0] - 2026-03-02

### Added
- **Interactive TUI dashboard** — running `nigel` with no arguments launches a full-screen dashboard with YTD P&L, account balances, monthly income/expense bar chart, and single-key command menu
- **First-run onboarding** — guided setup screen with animated logo, collects user name, business name, optional password, and offers demo/fresh/load options
- **Splash screen** — rainbow gradient ASCII logo with floating particle effects on launch
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
- **GitHub Actions CI** workflow
- **Integration tests** for CLI dispatch paths

### Changed
- Demo transactions generated dynamically (18 months from current date) instead of hardcoded dates
- Browse register shows all transactions by default (no implicit year filter)
- Review screen migrated to ratatui from raw crossterm
- BofA importers refactored to share parsing helpers

### Fixed
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

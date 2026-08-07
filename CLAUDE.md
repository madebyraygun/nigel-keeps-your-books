# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage.

## Architecture

- **Crate layout:** lib + bin. `src/lib.rs` exposes every module (`db`, `models`, `reports`, `reviewer`, `importer`, `categorizer`, `reconciler`, `migrations`, `settings`, `error`, `fmt`, `browser`, `tui`, `effects`, `pdf`, `cli`) as the `nigel` library; `src/main.rs` is the `nigel` binary and holds only clap parsing, the ratatui panic hook, and the dispatch pre-flight, calling into the library via `nigel::`
- **CLI:** Clap derive app in `src/cli/mod.rs` — subcommands are optional; running `nigel` with no arguments launches the interactive dashboard. Subcommands: init, demo, import, undo, categorize, review, reconcile, accounts, categories, rules, report, browse, load, backup, restore, serve, status, password, update, completions
- **Database:** SQLite via rusqlite (bundled-sqlcipher) in `src/db.rs` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations, metadata (key-value store for per-database settings like company_name). Optional SQLCipher encryption via `PRAGMA key`; password stored in runtime global `Mutex<Option<String>>` (`set_db_password`/`get_db_password`); `get_connection()` reads it internally so zero call-site changes needed; `open_connection()` for explicit password; `is_encrypted()` probes a DB file; `validate_password()` tests a password without side effects; `prompt_password_if_needed()` prompts via rpassword with 3 retries (used by CLI subcommands)
- **Importers:** `src/importer.rs` — `ImporterKind` enum dispatch (bofa_checking, bofa_credit_card, bofa_line_of_credit, gusto_payroll); each variant implements `detect()` and `parse()`; `GenericCsvConfig` supports user-defined column mappings stored as profiles in `csv_profiles` table (`save_csv_profile`/`load_csv_profile`/`list_csv_profiles`, the last returning `CsvProfile { name, config }` for the API); malformed CSV rows are counted and reported in import output; `built_in_formats()` lists the compiled-in importers (`ImporterFormat { key, name, account_types }`) for the API's format picker; `ImportResult` reports `format` (the resolved importer key, a profile name, or `generic` — `None` for a duplicate file, which is answered before resolution) and `import_id` (the `imports` row created, `None` for a dry run)
- **TUI:** `tui.rs` — shared ratatui helpers (style constants, `money_span`, `wrap_text`, `ReportView` trait with `date_params()`, `run_report_view()`) for interactive screens; `ReportViewAction` enum includes `Continue`, `Close`, and `Reload` (for date navigation); `browser.rs`, `cli/review.rs`, `cli/report/view.rs`, and `cli/dashboard.rs` use ratatui `Terminal::draw()` render loop
- **Dashboard:** `cli/dashboard.rs` — single-struct state machine with `DashboardScreen` enum; Home screen shows YTD P&L, account balances, monthly income/expense bar chart, and a command chooser menu with single-key shortcuts (b=Browse, i=Import, r=Review, c=Reconcile, a=Accounts, t=caTegorize, u=rUles, z=Undo, v=View report, e=Export report, l=Load, p=Settings, s=Snake); all commands render as inline TUI screens; outer loop only re-initializes when Load changes the data directory. F5 refreshes dashboard data.
- **Account Manager:** `cli/account_manager.rs` — inline TUI screen for managing accounts (list, add, rename, delete); uses form sub-screens for add/rename with text input and type selector; delete blocks if account has transactions
- **Guardrail reasons:** `error.rs` carries `DeleteBlock` (`subject`, `BlockReason`, `count`) and the `NotFound`/`Invalid`/`DuplicateName`/`Blocked`/`Conflict` `NigelError` variants. `accounts::delete_blocker` and `categories::delete_blocker` return the structured block; `categories::blocking_reason` is a thin `to_string()` wrapper over it for the TUI status line. `Display` reproduces the CLI's original wording verbatim, so the same error reads identically in a terminal and carries a machine code over HTTP
- **Category Manager:** `cli/category_manager.rs` — inline TUI screen for managing the chart of accounts (categories); list/add/edit/delete with form sub-screens for name, type (income/expense selector), tax line, and form line; soft-delete blocked if category has transactions or active rules; data layer in `cli/categories.rs`
- **Rules Manager:** `cli/rules_manager.rs` — inline TUI screen for viewing and deleting categorization rules; scrollable list with soft-delete confirmation; reads through the shared `cli::rules::list_rules` data layer and deletes through `cli::rules::deactivate_rule`
- **Rules data layer:** `cli/rules.rs` holds the whole rules surface as `&Connection` functions — `list_rules`/`get_rule` (`RuleRow`), `add_rule` (`NewRule`), `update_rule` (`RuleUpdate`, partial, `vendor: Some(None)` clears), `deactivate_rule` (soft), `test_pattern` (`RuleTestResult`, the dry run `nigel rules test` prints), `validate_match_type`, and `resolve_category_id`. The CLI subcommands are wrappers that resolve a category name to an id and print; the API passes ids straight through
- **Import Screen:** `cli/import_manager.rs` — inline TUI form for importing bank statements; file path input + account selector; runs import + auto-categorization and shows results
- **Undo Screen:** `cli/undo_manager.rs` — inline TUI screen for undoing the last import; shows import details (filename, account, date, transaction count) and confirms before deleting; data layer in `cli/undo.rs` (`list_imports` returns the full history newest-first; `get_last_import` is its first row)
- **Reconcile Screen:** `cli/reconcile_manager.rs` — inline TUI form for account reconciliation; account selector + month/balance input; shows reconciled/discrepancy result
- **Load Screen:** `cli/load_manager.rs` — inline TUI form for switching data directories; validates path and triggers dashboard reload
- **Reports:** `cli/report/` — unified report command with `--mode view|export`, `--format pdf|text`, and `--output` flags; `mod.rs` dispatches to `view.rs` (interactive ratatui views), `text.rs` (comfy_table formatting), or `export.rs` (PDF export); non-TTY automatically falls back to plain text stdout. `TableReportView` supports interactive date navigation: Left/Right arrows page between periods, `m` toggles month/year granularity. `reports.rs` owns the report vocabulary: `ReportKind` (one variant per report, `as_str()` giving the CLI/export slug) and `DateGranularity` (MonthAndYear, YearOnly, None), with `ReportKind::granularity()` as the single mapping between them; `ReportCommands::report_name()` delegates to it, and the views read their granularity from it. `PeriodMode` and the current-period state stay in `view.rs`
- **K-1 worksheet mapping:** `reports::resolve_k1_mapping()` maps each category to a 1120-S worksheet slot from its `form_line`. Vocabulary: `1120S-1a` (gross receipts), `1120S-2` (cost of goods sold), `1120S-5` (other income), `1120S-N` (deduction lines 7-19), `K-N` (Schedule K items), `excluded` (intentionally outside the worksheet, e.g. transfers). Income categories with no `form_line` fall back to gross receipts and are listed in the report's `auto_mapped` note; expense categories with no `form_line` are collected in `unmapped` and surfaced in a "Needs mapping" section, excluded from all totals. The worksheet reports Gross Receipts, Cost of Goods Sold, and Gross Profit; `total_deductions` is the sum of deductible amounts (meals limited to 50%).
- **Effects:** `effects.rs` — shared pastel rainbow gradient palette, `gradient_color()` interpolation, `Particle` struct with `new()`/`seeded()`/`tick()`/`is_dead()`, `pre_seed_particles()`, and `tick_particles()` helpers; used by splash, goodbye, onboarding, and snake screens
- **Splash:** `cli/splash.rs` — 1.5-second splash screen shown on app launch (skipped during first-run onboarding); displays Nigel ASCII logo with rainbow gradient text and pre-seeded floating particle background; dismissable by any keypress. For encrypted databases, the splash holds indefinitely (no auto-fade) and displays an inline masked password input below the logo; supports up to 3 attempts with error feedback; `run()` for unencrypted, `run_with_password(db_path)` for encrypted
- **Goodbye:** `cli/goodbye.rs` — 1.2-second farewell screen shown when quitting the dashboard; displays Nigel ASCII logo with "Goodbye!" text, plays the reverse of the splash reveal animation (characters disappear), with particle background; dismissable by any keypress
- **Updater:** `cli/update.rs` — `nigel update` command and launch-time version check; queries GitHub Releases API for latest version, compares via `semver`, downloads correct platform binary, and self-replaces via `self_replace` crate; `check_with_cooldown()` is the data-only check — opt-out, 24-hour cooldown (stored in `last_update_check` in settings.json), then `check_for_update()` — and `check_and_notify()` is the sentence `update_notice()` formats from it; opt-out via `update_check: false` in settings; dashboard shows yellow notification bar; CLI prints to stderr. `nigel serve` runs `check_with_cooldown` in a background task at startup and reports the version as `updateAvailable` on `/api/status`, so the web dashboard shows the same notice without the request path ever waiting on GitHub
- **Settings Manager:** `cli/settings_manager.rs` — inline TUI screen for managing app settings; shows editable business name (saved to DB metadata as `company_name`), password management, and auto-update check toggle; password sub-screen delegates to `PasswordManager`
- **Web server:** `src/server/` — `nigel serve` runs an axum app on 127.0.0.1 serving the JSON API and the embedded SPA from the same binary; behind the default-on `serve` feature (axum, tokio, tower, rust-embed, open, subtle). `mod.rs` owns the tokio runtime (the crate's only async entry point — `main` stays sync) and assembles the router; `auth.rs` holds the session token, Host/Origin validation, and cookie parsing; `error.rs` defines `ApiError`/`ApiErrorCode` and the `{"error": {...}}` envelope; `state.rs` holds `AppState` (db path, session token, build features, unlock gate, `db_gate`) — the db path is behind an `RwLock` because the settings screen can switch data directories while the server runs, and `db_gate` is a `tokio::sync::RwLock` readers hold while a connection is open so encrypt/decrypt/switch can rewrite the database file exclusively; `secret.rs` wraps password strings so they cannot be printed (redacted `Debug`, zeroized on drop); `extract.rs` wraps axum's `Json`/`Path` extractors as `ApiJson`/`ApiPath` so a malformed body or a non-numeric id answers in the error envelope instead of axum's plain text; `routes/` gets one module per domain (`status`, which owns `/status` and `/unlock`, the eight `reports` endpoints and their `exports` downloads, the `accounts`/`categories`/`rules`/`imports` lists and their writes, plus `transactions`, `review`, `reconcile`, and `settings`, and a JSON 404 fallback); `routes/mod.rs` layers the locked guard over the whole `/api` router, so a route added anywhere is guarded unless it is named in the short ungated list, and holds the shared helpers — `with_conn()` (the `spawn_blocking` + `db::get_connection` wrapper every handler runs its query through) and `with_conn_api()` (the same, for work that answers in `ApiError` — an export refusing a format this build cannot render), `ensure_account_exists()`, the `Deleted` response body, and `double_option()` for telling an absent PATCH field from an explicit `null`; `uploads.rs` owns the spool a browser import passes through (sanitized filename, one directory per upload, 0600/0700, resolve-by-id, hourly purge); `static_files.rs` serves `web/dist` via rust-embed with an index.html fallback for SPA routes. Handlers open their own connection per request — no pool
- **Read API:** `src/server/routes/reports.rs` exposes the eight reports at `/api/reports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}`, each wrapped as `{ granularity, report }` where `granularity` comes from `ReportKind::granularity()`. Which date parameters a route accepts mirrors its `nigel report` subcommand exactly, derived from that same granularity plus per-route `ranges`/`account` flags (`ParamSpec`); `RawQuery` takes every parameter as a string so validation errors land in the standard envelope instead of axum's plain-text `Query` rejection. `routes/{accounts,categories,rules,imports}.rs` serve the five list endpoints as bare JSON arrays
- **Write API:** `src/server/routes/transactions.rs` (`PATCH /api/transactions/:id`, `POST /api/categorize`), `review.rs` (queue, one-by-id, apply, undo), `reconcile.rs` (`POST /api/reconcile`, `GET /api/reconciliations`), and the create/update/delete halves of `accounts.rs`, `categories.rs`, `rules.rs` (plus `POST /api/rules/test`) and `imports.rs` (`DELETE /api/imports/:id`). Handlers validate, then call the same `&Connection` data layer the TUI uses; guardrail failures travel as typed `NigelError` variants and become 409s with a structured `details.reason`. Multi-field edits run in one `unchecked_transaction` so a rejected value leaves nothing half-applied
- **Export API:** `src/server/routes/exports.rs` serves the eight reports as downloads at `/api/exports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}` with a required `format=pdf|text` plus the same date/account parameters as the matching `/api/reports` route — validated by that module's `ParamSpec::for_kind()` + `ReportParams::parse()`, so the two route families cannot disagree. Handlers fetch through the same `reports::get_*` functions and render via `pdf::render_*` or `cli::report::text::format_*` + `with_header()`; the `pdf` feature is answered in one place (a `cfg`'d pair of `render_pdf` functions), so `format=pdf` without the feature is a 501 carrying `cli::report::PDF_DISABLED_MESSAGE` — the same sentence the CLI prints — while `format=text` always works. `Content-Disposition` uses `cli::report::export_file_stem()`, the CLI's own `<report>-<date>` naming. `GET /api/status` advertises the capability as `pdfExport` because a browser downloads an anchor without inspecting it. `reports::date_range_label()` (moved out of `cli/export.rs`) builds the period label both the CLI and the API print under a PDF's title. Bulk `report all` and `--output` file writing stay CLI-only
- **Settings API:** `src/server/routes/settings.rs` covers what `settings_manager.rs` does plus `nigel load` — `GET/PUT /api/settings/app` (only `updateCheck` is web-editable; the response struct is hand-written because `settings::Settings` has no `rename_all` and would put snake_case on the wire), `PUT /api/settings/company-name` (trimmed, empty clears it), `POST /api/settings/data-dir`, and `POST /api/settings/password/{set,change,remove}` wrapping `cli/password.rs`'s `&Path` functions. Every one of them is behind the locked guard — including the two that never open the database, because nothing on the unlock screen reads them and `change`/`remove` would otherwise be an unthrottled password oracle reachable without passing the gate
- **Web UI (SPA):** `web/` is an npm workspace (no turbo) with three packages built in order. `@nigel/theme` composes per-category Lit `css` token modules into one `CSSResult` plus a generated plain stylesheet; tokens shadow Web Awesome's `--wa-*` namespace (no WA stylesheet is loaded — theming rides custom properties) and nigel-specific tokens use `--nc-*`; the brand palette is derived from `src/effects.rs` (a parity test fails if they drift) and every solid color is held to WCAG AA in both light and dark by a contrast test. `@nigel/ui` holds the `wc-*` Lit components, `WcIconBase` + icons, and the preview harness (glob manifest + query-string router on port 9090); each component has a co-located `.preview.ts` and a test that runs axe over every state the preview declares. `@nigel/app` is composition only: `main.ts` (3-line bootstrap), `nigel-app` root container, the screen registry, the app store, and the api client. Cherry-picked Web Awesome imports only, never the autoloader. Build output goes to `web/dist`, which `rust-embed` bakes into the binary
- **SPA routing and api seam:** `web/apps/app/src/screens/registry.ts` is a `Record<ScreenId, ScreenDef>` describing every screen once — title, nav label, icon, `inNav`, `render()` — and the sidebar, header, and content area all derive from it, so a missing screen is a compile error. `location.hash` (`#/<screen>?<params>`) is the only writer of route state; `hash-route.ts` parses and serialises it. `web/apps/app/src/api/` is the only module that talks to the server: `types.ts` mirrors the serde structs by hand (camelCase), `client.ts` defines `ApiClient` (one typed method per endpoint, no generic `request()`) and `FetchApiClient`, and owns the `appLocked` (423) and `appUnauthorized` (401) transport signals. `request()` branches on a `FormData` body and omits the JSON content type for it, because only the browser can generate the multipart boundary — that is the whole of `uploadImport`'s special handling, and it takes no progress callback, since `fetch` cannot report upload progress and promising it would bind every future implementation to producing it. A guard test fails the build on any `fetch(`/XHR/EventSource/WebSocket/`sendBeacon` outside `src/api`
- **SPA dashboard:** `web/apps/app/src/screens/dashboard.ts` (`nigel-dashboard-screen`) mirrors the TUI home screen — year-to-date P&L, account balances, twelve months of cash flow, the flagged count, and the update notice. `state/dashboard-store.ts` fetches the four reports in parallel and holds each in its own `{ data, loading, error }` slot, so a failure lands on one card with an inline retry rather than blanking the screen; `screens/dashboard-data.ts` maps `CashflowReport` to chart buckets, taking the last twelve months **that have data** with no calendar zero-fill, exactly as `cli/dashboard.rs` does, so the two front ends chart the same books
- **SPA register:** `web/apps/app/src/screens/register.ts` (`nigel-register-screen`) is the web `browser.rs` — account filter, period nav, incremental search, row selection, inline category/vendor editing and flag toggling. Search is client-side and stays that way (`/api/reports/register` has no search parameter): `screens/register-data.ts` holds `rowMatches` (the TUI's `recompute_search_matches` — case-insensitive substring over description, vendor and category name, missing fields treated as empty), `indexOfToday` (`scroll_to_today` parity, last row dated on or before today, local date not UTC), `registerParamsFrom` (route to request; `from`/`to` only as a pair and winning over `year`/`month`) and `buildPatch` (only changed fields, since an empty PATCH is a 400 and `categoryId: null` is refused). Account and period changes navigate rather than mutate state, so filters are links; `?q=` seeds the search but is not written back per keystroke. Edits are optimistic, replaced by the row the server answers with, rolled back with a toast on failure; flags are sent as a state, never a toggle. `@nigel/ui` gains `wc-register-table` (roving-tabindex `role="grid"`, arrows/Home/End/PgUp/PgDn/Enter/Esc/`f`/`/`, hand-built category combobox because this Web Awesome build has no searchable select, and no `wa-*` in a row outside edit mode), `wc-register-toolbar`, and `wc-period-nav` — the granularity-driven pager the report screens share, with `allowAll` for the register's unfiltered default
- **SPA review:** `web/apps/app/src/screens/review.ts` (`nigel-review-screen`) is the web `reviewer.rs` — one flagged transaction at a time from `#/review`, or a single re-review from `#/review?id=185` (`nigel review --id`; the router has no path segments). Every applied decision is pushed onto a client-side stack of `{ transactionId, ruleId }` and Back pops one and calls `POST /api/review/:id/undo` with that rule id, which re-flags the transaction and deletes the rule outright; a skip pushes `null`, the same `Option<ReviewDecision>` the TUI stacks, so stepping back over a skip issues no request. A failed undo pushes its decision back, so the stack never claims a decision the server still holds, and the summary counts are derived from the stack (`screens/review-data.ts`: `summarize`, `toReviewItem`, `singleIdFrom`) rather than tallied, so a Back corrects them for free. A 404 on apply is read from its `details.reason`, because the route answers 404 for two opposite things: `transaction_not_found` advances with a toast and records a skip rather than wedging the queue, while `category_not_found` — and any 404 naming no reason — keeps the transaction on screen with the failure beside the form, since that decision is still waiting to be made. Two deliberate departures from the TUI: **Tab does not skip** (on the web it is the focus key — Skip is a button, and only Enter/apply and Esc/back are bound), and there is **no match-type choice**, because `apply_review` writes `contains` and the apply route has no field for anything else. The rule pattern prefills with the first two words of the description (TUI parity) and drives a debounced `POST /api/rules/test`. `@nigel/ui` gains `wc-review-card`, `wc-review-progress` (a labelled bar, not dots — a post-import queue is routinely 50-200 long), `wc-review-form`, `wc-rule-test-preview`, and `wc-category-picker` — the searchable combobox lifted out so the review form and the register's inline editor share one implementation and one `categoryLabel`
- **SPA import:** `web/apps/app/src/screens/import.ts` (`nigel-import-screen`) is the web `import_manager.rs` — one screen whose panels appear as the decision is made: choose a file, preview it, confirm. The upload is **lazy**: picking a file sends nothing, and Preview uploads and dry-runs in one action, so a file chosen and thought better of never reaches the server's spool and the account is known before any bytes move. The `uploadId` is cached against the chosen file, so correcting a column mapping and previewing again re-reads the copy the server already holds; an expired upload (404 `details.reason = upload_not_found`, an hour after the fact) is re-uploaded once, silently, because the file never left the browser. A **duplicate file blocks the confirm** rather than warning about it — the server would answer 200 with zero counts, so the button would offer a no-op. `screens/import-data.ts` holds the pure half: `importRequestBody`/`confirmRequestBody` derive `format` and `mapping` from one form field so sending both (a 400) is unrepresentable and `saveProfile` can never travel without the mapping it names, `previewCounts`/`resultCounts`, `formatLabel`, and `routeImportError`, which files each failure where its cause is — a 413 under the dropzone, a 501 and other 400s under the format select, a mapping 400 under the mapping form, an unknown account under the account select plus a toast, and 423/401 nowhere, because the shell gates those before a screen exists. `@nigel/ui` gains `wc-dropzone` (drag-and-drop plus picker; the well is a `<button>` so keyboard and mouse share one path, and it checks extension and size client-side because the server can only answer 413 after 25 MB have crossed the wire), `wc-import-form` (account, format and the generic column mapping in one component — the format list is flat because Web Awesome's select has no option-group element), `wc-sample-table`, and `wc-count-grid` (labelled integers, deliberately not the money-formatting `wc-stat-card`)
- **SPA reports:** `web/apps/app/src/screens/reports.ts` (`nigel-reports-screen`) serves all eight reports and the directory that lists them — `#/reports` for the landing, `#/reports?report=pnl&year=2025` for one (a query parameter, since the router has no path segments). `screens/reports-data.ts` holds the catalog (one `ReportDef` per slug: title, icon, and the date parameters that route accepts) plus the pure table mappers, shaped deliberately like `cli/report/text.rs` so both front ends print the same rows in the same order. `REPORTS[slug].supports` gates what reaches the request — the server answers 400 for a parameter its route does not take, so `year` on `/api/reports/balance` is an error, not a no-op. `wc-period-nav` takes its granularity from the `granularity` field on the response envelope rather than a client-side table, runs with `allowAll` off (an unfiltered view belongs to the register browser), and defaults to the current year, which is what `TableReportView::new` seeds the TUI's own date navigation with. The K-1 worksheet is **composed** from `wc-panel`/`wc-report-table`/`wc-notice-bar` rather than given a component of its own — a `wc-k1-worksheet` would have to take `K1PrepReport` as a property, dragging API types into `@nigel/ui` — and renders in `format_k1`'s order including the auto-mapped note and the needs-mapping section. The register view reuses `wc-register-table` in `readonly` mode and links to `#/register` for editing. `@nigel/ui` gains `wc-report-table` (one declarative table for every report section: columns and rows as data, section/subtotal/total emphasis, optional row links), `wc-export-links` and `wc-link-grid`
- **SPA exports and printing:** export buttons are plain `<a download>` links whose href comes from `ApiClient.exportUrl(slug, format, params)` — a download link is as much a hardcoded address as a `fetch`, so the api seam owns it, and the guard test now fails on any quoted `/api/` literal outside `src/api` (comment lines excepted, since documenting the seam is not routing around it). PDF is offered only when `/api/status` reports `pdfExport`: a link cannot inspect its response, so on a build without the pdf feature the button would otherwise save a 501 envelope as `pnl.pdf`. `@nigel/theme`'s `print.ts` composes last into `nigelTheme` and gives the page over to the report — shell chrome hidden through the parts `wc-app-shell` exposes, colours repainted by redefining tokens at `:root` (custom properties are what inherit through shadow boundaries), repeating table headings, 1.5cm margins
- **SPA managers:** `web/apps/app/src/screens/{accounts,categories,rules}.ts` are one screen three times over — list, Add, per-row Edit and Delete — sharing `wc-manager-layout`, `wc-manager-table` and `wc-manager-dialog` and differing only in columns and form. Together they are a superset of the TUI, which can only list and delete rules (`rules_manager.rs`). Editing is **in a dialog, not an inline panel**: the rule form is tall enough (pattern, match type, category, vendor, priority, live test preview) that inline it would push the list off screen, delete is already a dialog, and `wa-dialog` brings the focus trap with it. Guardrails are rendered from `details.reason`, never from the server's sentence — `screens/manager-errors.ts` is the whole table (`has_transactions`, `has_active_rules`, `duplicate_name`, `already_inactive`), with the count formatted client-side, which is what makes the strings translatable; a `400` and an unrecognized 409 reason are the two deliberate exceptions and render the server's message, because `Invalid regex: unclosed group` names the offending value and anything re-derived would drift. A failed **save** renders in the dialog beside the field; a failed **delete** renders in the layout's alert region, because `confirmDialog()` resolves and removes itself before the request is sent. Every mutation refetches its list (no optimistic splicing: a priority edit reorders the rules, a rename reorders the categories, and a category rename changes the name on every rule row). Accounts has no transaction-count column (`GET /api/accounts` does not carry one and a screen may not add an endpoint — the number appears in the blocked delete) and enforces the 4-digit last-four rule that lives only in `account_manager.rs`. Categories edits send only changed fields (an all-omitted PATCH is a 400), and document the K-1 form-line vocabulary beside the field with a runtime-derived datalist plus a non-blocking warning for a value `resolve_k1_mapping` matches literally. Rules keeps the server's priority order (it is the semantics), debounces `POST /api/rules/test` at 250 ms into the review screen's own `wc-rule-test-preview` (immediately on a match-type change — a click is a decision, not typing), does **no** client-side regex validation (JS `RegExp` and the Rust `regex` crate accept different languages), and filters client-side on `#/rules?categoryId=12`, which is where the categories guardrail points. `@nigel/ui` gains `wc-manager-layout`, `wc-manager-table`, `wc-manager-dialog`, `wc-account-form`, `wc-category-form`, `wc-rule-form`, and the `wc-icon-plus`/`edit`/`trash` glyphs
- **SPA reconcile and undo:** `web/apps/app/src/screens/reconcile.ts` (`nigel-reconcile-screen`) is the web `reconcile_manager.rs` — account, month and statement balance, the reconciled-or-discrepancy verdict, and the history below it. The history is refetched after **every** submit, not only a matching one, because `POST /api/reconcile` records the attempt whichever way it came out and that record is how the history knows which months have been checked. The verdict is held with the request that produced it, so editing the month afterwards cannot relabel February's figures as March's. `screens/reconcile-data.ts` files a failure under the control that caused it: a 409 `no_transactions` on the month and a 404 on the account, both in our own words — the 404 deliberately **not** the server's sentence, which tells you to run `nigel accounts list`, good advice in a terminal and useless beside an account picker. The typed figures survive a failed submit, since this is the one screen where the number was copied off a paper statement. `screens/undo.ts` (`nigel-undo-screen`) supersets `undo_manager.rs`, which can only offer the most recent import because a terminal has nothing to point at: `DELETE /api/imports/:id` has always taken an id, so the web lists every import and undoes the chosen one. Confirming names the count and the file; a 404 (another tab got there first) is reported rather than passed off as success, and either outcome refetches rather than splicing, because every other row's count is the server's to state. AC-level freshness needs no invalidation wiring — `src/__tests__/screen-freshness.test.ts` drives the whole app, undoes an import and navigates to the register and the dashboard, asserting the refetch lands after the delete; the property holds because there is no global cache and each screen is a distinct element Lit tears down on a route change. `@nigel/ui` gains `wc-reconcile-form` (which carries the app's only currency input: a rendered `$` prefix, `inputmode="decimal"`, commas stripped exactly as `reconcile_manager.rs` strips them, a tidy on blur through `Intl`, and `wa-input type="month"` — confirmed to survive jsdom, with the `YYYY-MM` check kept anyway because Safari degrades the control to text), `wc-reconcile-result` (the difference gets its own emphasised row rather than leaning on the red alone, the same reasoning that makes `wc-money` always print its sign, and it shows the statement figure beside the calculated one where the TUI prints only the calculated), `wc-import-history` and `wc-reconciliation-history` (a null balance renders as an em dash, never an invented `$0.00`)
- **Report figure parity:** `web/apps/app/src/screens/reports-parity.test.ts` compares every money figure the browser renders against every money figure in the CLI's own text export, per report, on absolute values (`wc-money` always renders the sign; the text report prints magnitudes and lets colour carry direction). Both sides are captured from one seeded database by `src/server/fixture_capture.rs` — an `#[ignore]`d test, run with `cargo test --features serve capture_web_report_fixtures -- --ignored`, writing `.json`/`.txt`/`manifest.json` into `web/apps/app/src/__fixtures__/reports/` plus a `needs-mapping-k1` pair from a second database that carries an unmapped category. It is a test rather than a script because a script driving `nigel serve` would have to run `nigel init --data-dir`, rewriting the developer's real settings.json
- **Modules:** `categorizer.rs` (rules engine), `reviewer.rs` (review data layer; `set_transaction_flag` sets an explicit state and `toggle_transaction_flag` is expressed in terms of it), `reports.rs` (P&L, expenses, tax, cashflow, balance, flagged, register, K-1 prep), `browser.rs` (interactive register browser via ratatui with row selection, inline category/vendor editing, flag toggling, scroll navigation, text wrapping, and incremental text search), `reconciler.rs` (monthly reconciliation), `pdf.rs` (PDF rendering via printpdf, feature-gated)
- **Migrations:** `migrations.rs` — sequential schema migration runner; `MIGRATIONS` array of `(version, description, up_fn)`; runs inside `init_db()` after table creation, which `main.rs` invokes in its dispatch pre-flight for every subcommand except `init`, `demo`, `load`, `update`, `password`, `completions`, and `restore`, and which the dashboard invokes in its own pre-flight, so every normal use of the app brings the schema up to date; each migration executes in a savepoint transaction; version tracked in `metadata` table under `schema_version` key; v1 is the no-op baseline for existing 0.1.x databases; v2 adds `csv_profiles` table for generic CSV column mappings; v3 backfills `form_line` on the stock chart-of-accounts categories
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
cargo test -- --test-threads=1                    # Run all tests (serial — the DB password is a process global)
cargo test --no-default-features -- --test-threads=1   # Test without gusto/pdf features
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
nigel serve                                       # Web UI + JSON API on 127.0.0.1:5731 (opens a browser)
nigel serve --port 8080                           # Bind a different port (0 = ephemeral)
nigel serve --no-open                             # Print the tokenized URL instead of opening a browser
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

### Web UI

Requires Node 20.19+ (22 recommended). All commands run from `web/`.

```bash
npm ci                                            # Install (committed lockfile)
npm run build                                     # theme -> ui -> app, output to web/dist
npm test                                          # vitest across all three packages
npm run lint                                      # eslint across all three packages
npm run typecheck                                 # tsc --noEmit across all three packages
npm run dev                                       # Vite dev server on :5173 (proxies to :5731)
npm run preview                                   # Component preview harness on :9090
```

Dev loop — run the backend and the dev server side by side, then open the
token URL **on the vite origin** so the session cookie lands there:

```bash
cargo run -- serve --no-open                      # terminal 1, prints /auth?token=<hex>
cd web && npm run dev                             # terminal 2
# browser: http://localhost:5173/auth?token=<hex>
```

`cargo build` works without node — `build.rs` seeds `web/dist` from
`web/placeholder/index.html` and the binary serves a "SPA not built" page. Run
`npm run build` in `web/` before `cargo build --release` to embed the real app.

## Component-First UI Workflow (MANDATORY)

Every visual change ships through `@nigel/ui`:

1. **The component lives in `web/packages/ui/src/components/`** as `wc-foo.ts`.
2. **A preview is co-located.** `wc-foo.preview.ts` covers the visible states (default, hover, disabled, loading, empty, dense — whichever apply).
3. **A11y passes.** `wc-foo.test.ts` calls `describePreviewA11y(preview)`, which runs `axe.run()` over every state the preview declares with zero violations. Adding a state adds its a11y test automatically — do not restate the states inside the test.
4. **Then it is consumed.** `web/apps/app` imports from `@nigel/ui`. **No bespoke component implementations in `web/apps/app/src/components/`** beyond the `nigel-app` root container.

The preview harness boots with `npm run preview` in `web/` at http://localhost:9090.

### Pre-merge checklist (visual changes)

- [ ] `wc-foo.preview.ts` exists with all visible states
- [ ] `describePreviewA11y` runs and passes with zero violations
- [ ] The component reads tokens from `@nigel/theme` — no inline brand values
- [ ] No styling logic for primitives lives in `web/apps/app/`

Pure logic, state, and service work is exempt.

### Component selection

- Use Web Awesome `<wa-*>` primitives unless behavior demands custom. Import them cherry-picked (`@awesome.me/webawesome/dist/components/<x>/<x>.js`) — never the autoloader, and never the WA stylesheet.
- A `wc-*` wrapper reads `@nigel/theme` tokens and exposes them as cascading variables; it never duplicates a brand value inline.

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
- Schema migrations run in the `dispatch()` pre-flight — after the initialization check and the password prompt — for every subcommand except `init`, `demo`, `load`, `update`, `password`, `completions`, `serve`, and `restore`; the dashboard runs the same pre-flight before its render loop. The exemptions: `init`, `demo`, and `restore` call `init_db()` themselves on the database they create or replace; `load` only rewrites `settings.json`; `update` needs no database; `password`, `completions`, and `serve` are password-exempt and so cannot open an encrypted database. A failed migration aborts the command. Each migration is transactional (savepoint); to add a migration: append to `MIGRATIONS` array in `migrations.rs`, bump `LATEST_VERSION`, implement `up()` function with SQL statements
- Generic CSV profiles are stored in `csv_profiles` table; `--format <name>` resolves built-in importers first, then csv_profiles; generic CSV is never auto-detected
- `--dry-run` skips snapshot creation, imports table insertion, and transaction insertion; still runs full parse and duplicate detection
- Auto-update check runs once per 24 hours on launch (both dashboard and CLI); respects `update_check: false` in settings.json; silently skips on network failure; `nigel update` command always checks and can be exempt from init/password checks
- Platform binary detection: macOS = `nigel-universal-apple-darwin`, Linux x86_64 = `nigel-x86_64-unknown-linux-gnu`, Windows x86_64 = `nigel-x86_64-pc-windows-msvc.exe`
- `serve` is password-exempt in the dispatch pre-flight — it has no stdin to prompt on. It runs migrations itself at startup, but only when the database is unencrypted; an encrypted database stays locked until a client unlocks it over HTTP, and migrations run then
- Localhost is not a trust boundary for `serve`. Three layers, in middleware order: bind 127.0.0.1 only; reject any request whose `Host` (or `Origin`, when present) is not exactly `localhost`/`127.0.0.1`/`[::1]` on any port (403, blocks DNS rebinding); require the `nigel_session` cookie on every `/api` route (401). The per-run 32-byte token is compared in constant time, is never persisted, and never appears in a response body. Static assets are session-exempt because they carry no data
- An encrypted database leaves `serve` locked: `GET /api/ping`, `GET /api/status`, and `POST /api/unlock` answer; every other `/api` path — including the 404 fallback — returns 423 `locked` until `POST /api/unlock` validates the password (`db::validate_password`), adopts it (`db::set_db_password`), and runs the deferred migrations (`init_db`). A migration failure re-locks the process rather than leaving it half-unlocked. Unlock is process-wide, so one server run serves one database. The guard is layered over the whole `/api` router and exempts the three gate paths by name (`UNGATED_PATHS`), so a new endpoint is guarded by default rather than by remembering where to mount it. It probes `is_encrypted` per request rather than caching it at startup, because password management over HTTP can change that state mid-run
- Failed unlock attempts are counted in memory (`AppState.unlock`): `attemptsRemaining` counts 3 down to 0 with no hard lockout, and from the third failure the server delays its response 1s, 2s, 4s… capped at 30s (`tokio::time::sleep`, never a thread sleep). `retryAfterMs` reports the delay already applied to that response. A success resets the counter; nothing is persisted
- Every `/api/settings/*` route is behind the locked guard, including `GET/PUT /api/settings/app`, which never opens the database. Nothing on the unlock screen reads app settings, and `password/change`/`password/remove` take the current password in the body, so an exemption would make them an unthrottled password oracle reachable without passing the gate. A wrong `currentPassword` draws down the *same* `AppState.unlock` budget as a failed unlock, so guessing costs the same either way. Forgotten-password recovery stays `nigel load` plus a restart
- Encrypting, decrypting, and switching data directory take `AppState.db_gate` for writing; every connection is opened under it for reading (`routes::with_conn`, and `routes::imports`'s own `blocking` helper, which opens connections itself). `encrypt_database`/`decrypt_database` finish by renaming the database file and deleting the `-wal`/`-shm` sidecars, which a live connection elsewhere would not survive. `AppState.db_path` is behind an `RwLock` for the same feature: a data-directory switch rebinds the running server, because rewriting settings.json alone would leave it serving the old books under the new directory's name. The switch also clears the password global (an encrypted target must come up locked), resets the unlock budget, and migrates an unencrypted target
- Guardrail failures carry a machine-readable reason alongside the human message. `NigelError::Blocked`/`DuplicateName`/`Conflict`/`NoTransactions` become 409s whose `details.reason` is one of `has_transactions`, `has_active_rules`, `duplicate_name`, `already_inactive`, `no_transactions`, with `count`/`name`/`month` where those apply; `NotFound` is 404 and `Invalid` is 400. The `Display` text is unchanged, so the CLI and TUI print exactly what they always did — the structure is additive, and clients render from the code rather than parsing the sentence
- The API's flag edit is idempotent where the TUI's is a toggle: `PATCH /api/transactions/:id` takes `flag: bool` and settles on that state, because a toggle sent twice over an unreliable connection lands somewhere nobody chose. `reviewer::toggle_transaction_flag` (the register's `f` key) is now expressed in terms of `set_transaction_flag`
- A browser import is three calls — `POST /api/imports/upload` (multipart, 25 MB `DefaultBodyLimit`, `.csv`/`.xlsx`/`.xls` only), `preview` (`import_file` with `dry_run`), `confirm` (snapshot → import → categorize → optional `saveProfile`, the same sequence as `import_manager.rs`). Uploads spool to `<data_dir>/tmp/uploads/<32 hex>/<sanitized name>` (dirs 0700, file 0600) so the `imports` row records the user's filename, not an id; they are purged after an hour at startup and on each upload, deleted after a successful confirm, and kept after a failed one so the same `uploadId` can be retried. `format` and `mapping` are mutually exclusive (400); a duplicate file and malformed rows are data, not errors; a missing cargo feature named as a format is 501, and the routes map parse failures (`NigelError::Csv`/`Other`) to 400 rather than the global mapping's 500
- PATCH bodies distinguish an absent field from an explicit `null` (`double_option`): absent leaves a column alone, `null` clears it. `categoryId: null` is the exception — a 400, since uncategorizing is what review undo is for
- Rule `match_type` and `regex` patterns are validated in the data layer (`rules::validate_match_type`), so `nigel rules add --match-type bogus` and an uncompilable regex now error instead of saving a rule the categorizer can never match. `accounts::add_account` likewise validates the account type and rejects duplicate names for the CLI, which previously bypassed both by inserting directly
- The HTTP API is stricter with date parameters than the CLI is. `cli::parse_month_opt` answers a malformed `--month` with `(None, None)`, which on a report endpoint would silently widen the query to the whole database; over HTTP `month` must be `YYYY-MM`, `from`/`to` must be zero-padded `YYYY-MM-DD`, and anything else is a 400. A parameter a route does not support (`from` on expenses, `month` on tax, `account` on anything but the register) is also a 400 rather than ignored. `from`/`to` remain a pair, enforced in the route layer so the failure is a 400 and not `date_filter`'s error surfacing as a 500
- Unknown `account` on `/api/reports/register` is a 404. `reports::get_register` itself reports an unknown account as an empty register, which over HTTP is indistinguishable from an account with no transactions, so the route checks existence first
- The database password never reaches a log, an error message, or `Debug` output: request bodies hold it in `server::secret::Secret` (redacted `Debug`, zeroized on drop), and errors from opening the database on the unlock path are replaced with a fixed message because rusqlite renders `PRAGMA key = '<password>'` as literal SQL inside `SqlInputError`
- The session middleware wraps the `/api` router with `.layer`, not `.route_layer`, so it also covers that router's 404 fallback — otherwise unauthenticated requests to unknown `/api` paths would skip the check
- Web assets are embedded with rust-embed's `debug-embed` on, so debug and release builds behave identically and neither reads `web/dist` at runtime. `web/dist` is generated by the vite build and gitignored; `build.rs` seeds it from the committed `web/placeholder/index.html` when no `index.html` is present, so `cargo build` works without node
- `build.rs` also emits `cargo:rerun-if-changed` for `web/dist`. This is load bearing, not decorative: `debug-embed` controls *when* assets are baked in, not when cargo reconsiders them, and rust-embed's proc macro cannot emit the key itself — without the build script a fresh `npm run build` followed by `cargo run` serves the previously embedded bytes. Both CI and the release workflow build `web/` before any cargo step for the same reason; a release built without node would ship the placeholder
- Every visual change ships through `@nigel/ui` (see "Component-First UI Workflow" below). All server access goes through `web/apps/app/src/api/` — a guard test enforces it. No `window.confirm`: confirmations use `wc-confirm`/`confirmDialog()`
- A 401 carrying `invalid_password` is a mistyped database password, not a dead session, and must never raise `appUnauthorized` — that would show a "session expired" banner to someone who simply typed the wrong key
- `wc-money` renders the sign as a literal `-` rather than conveying it by color alone as `tui::money_span` does. A terminal can rely on red-versus-green; a browser cannot (WCAG 1.4.1), and red/green is the pair color-vision deficiency flattens most. Its formatting is tested against the same vectors `src/fmt.rs` asserts, so the two front ends cannot disagree about how an amount reads

## Project Structure

```
src/
  lib.rs                # Library root — exposes all modules as the `nigel` crate
  main.rs               # Binary entry point: clap parse, panic hook, command dispatch
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
    serve.rs            # nigel serve (feature gate + pre-flight, delegates to src/server/)
    status.rs           # nigel status (show active DB + stats)
    update.rs           # nigel update (version check + self-replace from GitHub Releases)
  server/               # Web server (feature-gated behind "serve")
    mod.rs              # Tokio runtime, router assembly, middleware order, graceful shutdown
    auth.rs             # Session token, Host/Origin validation, cookie parsing, /auth handler
    error.rs            # ApiError + ApiErrorCode, JSON error envelope, status mapping
    extract.rs          # ApiJson/ApiPath: axum extractors whose rejections use the error envelope
    state.rs            # AppState (db path, session token, build features, unlock gate, db file gate)
    secret.rs           # Secret: redacted-Debug, zeroize-on-drop password wrapper
    uploads.rs          # Spooled browser uploads: sanitize, store 0600/0700, resolve by id, purge
    static_files.rs     # rust-embed hosting of web/dist with SPA index fallback
    fixture_capture.rs  # cfg(test) only — captures the web UI's report fixtures
    testutil.rs         # Test-only: temp/seeded databases, session router, JSON request helpers
    routes/
      mod.rs            # API router; GET /api/ping, JSON 404 fallback, guarded data_router(), with_conn()
      status.rs         # GET /api/status, POST /api/unlock, locked guard middleware
      reports.rs        # The eight GET /api/reports/* endpoints + query param validation
      exports.rs        # The eight GET /api/exports/* downloads (format=pdf|text)
      accounts.rs       # GET/POST /api/accounts, PATCH/DELETE /api/accounts/:id
      categories.rs     # GET/POST /api/categories, PATCH/DELETE /api/categories/:id
      rules.rs          # GET/POST /api/rules, PATCH/DELETE /api/rules/:id, POST /api/rules/test
      imports.rs        # GET /api/imports, /api/imports/formats, /api/csv-profiles; DELETE /api/imports/:id; POST upload/preview/confirm
      transactions.rs   # PATCH /api/transactions/:id, POST /api/categorize
      review.rs         # GET /api/review/queue, GET /api/review/:id, POST apply/undo
      reconcile.rs      # POST /api/reconcile, GET /api/reconciliations
      settings.rs       # GET/PUT /api/settings/app, PUT company-name, POST data-dir, POST password/{set,change,remove}
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
build.rs                # Seeds web/dist from the placeholder; rerun-if-changed for rust-embed
web/                    # npm workspace for the SPA (see web/README.md)
  package.json          # Workspace root: packages/*, apps/* — no turbo
  placeholder/
    index.html          # Committed "SPA not built" fallback (build.rs copies it into dist/)
  dist/                 # Generated by `npm run build`, gitignored, embedded by rust-embed
  packages/
    theme/              # @nigel/theme — token modules composed into one CSSResult + a plain .css
      src/tokens/       # color, gradient, typography, spacing, radius, shadow, motion
      src/global.ts     # ::part() overrides for wa-* primitives (document-level)
      src/print.ts      # @media print — composed last so it wins over dark mode
      scripts/build-css.js
    ui/                 # @nigel/ui — wc-* components + preview harness
      src/components/   # wc-app-shell, wc-nav-sidebar, wc-toast, wc-confirm, wc-money,
                        #   wc-empty-state, wc-spinner, wc-panel, the register, review,
                        #   import, report and manager families (wc-report-table,
                        #   wc-export-links, wc-link-grid, wc-manager-*), the reconcile
                        #   family (wc-reconcile-form, wc-reconcile-result,
                        #   wc-reconciliation-history, wc-import-history),
                        #   wc-category-picker
                        #   (each with .preview.ts + .test.ts)
      src/icons/        # WcIconBase + the starter icon set
      preview/          # Glob manifest, query-string router, axe suite (port 9090)
  apps/
    app/                # @nigel/app — composition only
      src/api/          # types.ts, client.ts — the ONLY module that talks to the server
      src/screens/      # registry.ts (Record<ScreenId, ScreenDef>), context.ts, hash-route.ts,
                        #   one module per screen (+ a *-data.ts sibling for its pure logic)
      src/state/        # app-store.ts (status/locked/company signals)
      src/__fixtures__/ # Report fixtures captured from a seeded DB (figure-parity tests)
      src/mixins/       # signal-watcher.ts — the @lit-labs/signals seam
      src/components/   # nigel-app.ts — root container
docs/
  api.md                # HTTP API inventory (endpoints, error envelope, security model)
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

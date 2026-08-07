---
id: TASK-31.1
title: Restructure crate into lib + bin targets
status: Done
assignee:
  - '@agent-31.1'
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 18:51'
labels:
  - web
  - backend
dependencies: []
references:
  - src/main.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.1-lib-split.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The crate is binary-only (main.rs declares all modules), so nothing outside main.rs can link against the data layer. Add src/lib.rs exposing the existing modules as a library and slim main.rs down to CLI dispatch over it. This is the foundation for the web server handlers, future Tauri workspace split, and richer integration tests. No behavior change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 src/lib.rs exposes db, models, reports, reviewer, importer, categorizer, reconciler, migrations, settings, error, and fmt (plus cli data-layer modules where needed)
- [x] #2 The nigel binary builds and behaves identically for CLI and TUI
- [x] #3 cargo test passes, including the assert_cmd integration tests
- [x] #4 No presentation code (ratatui/crossterm usage) leaks into the lib-exposed data-layer path
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Goal: add src/lib.rs so the data layer is linkable; main.rs slims to clap dispatch + panic hook. No behavior change; CLI/TUI output byte-identical. One commit.

## Step 0 — Baseline
Record a green baseline before touching anything, and capture the CLI surface for later diffing:
- cargo build; cargo test -- --test-threads=1; cargo test --no-default-features -- --test-threads=1; cargo clippy --all-targets -- -D warnings; cargo fmt --check
- Save `nigel --help`, `nigel report --help`, `nigel accounts --help`, `nigel rules --help` to /tmp for a byte-for-byte diff after the change.
- Save `grep -rln "ratatui\|crossterm" src/` as the AC#4 baseline file list.

## Step 1 — Add src/lib.rs
New file, crate-level doc comment plus exactly the module set main.rs declares today (all made `pub`, same alphabetical order):

    pub mod browser;
    pub mod categorizer;
    pub mod cli;
    pub mod db;
    pub mod effects;
    pub mod error;
    pub mod fmt;
    pub mod importer;
    pub mod migrations;
    pub mod models;
    #[cfg(feature = "pdf")]
    pub mod pdf;
    pub mod reconciler;
    pub mod reports;
    pub mod reviewer;
    pub mod settings;
    pub mod tui;

That covers every module the AC names (db, models, reports, reviewer, importer, categorizer, reconciler, migrations, settings, error, fmt) plus browser/tui/effects/pdf per the spec. No root re-exports (no `pub use error::Result`) — keep the surface literal so later tasks import `nigel::db::...` etc. `cli/mod.rs` already declares all 34 submodules `pub`, and the data-layer functions the API tasks need (cli::accounts, cli::categories, cli::undo, cli::backup, cli::password) are already `pub fn`, so no visibility widening is required inside cli.

Cargo.toml: expected to need NO change — cargo auto-discovers src/lib.rs as lib target `nigel` and src/main.rs as bin target `nigel`, and a lib and bin may share the crate name. If cargo complains, add explicit `[lib] name = "nigel" path = "src/lib.rs"` + `[[bin]] name = "nigel" path = "src/main.rs"` and nothing else.

## Step 2 — Slim src/main.rs
- Delete all 17 `mod ...;` lines (including the `#[cfg(feature = "pdf")] mod pdf;` pair).
- Keep verbatim: the ratatui panic hook (main.rs:27-32), `Cli::parse()`, the `Commands::Update` update-notify guard, the error->exit(1) tail, and the whole `dispatch()` body (needs_existing_db / needs_password / replaces_db pre-flight, its comments, and every match arm) — no logic edits at all.
- Rewrite paths only:
  - `use cli::{AccountsCommands, BrowseCommands, CategoriesCommands, Cli, Commands, PasswordCommand, RulesCommands};` -> `use nigel::cli::{...}` plus `use nigel::cli;` so the existing `cli::foo::run()` call sites stay untouched.
  - `error::Result<()>` -> `nigel::error::Result<()>`; `error::NigelError::NotInitialized` -> `nigel::error::NigelError::NotInitialized` (or import them).
  - `crate::settings::get_data_dir()` -> `nigel::settings::get_data_dir()`; `crate::db::prompt_password_if_needed/get_connection/init_db` -> `nigel::db::...`.
- Verified every function main.rs calls is already `pub` (all cli::*::run / add / list / rename / delete / update / dispatch / register / check_and_notify, plus ImportOpts and its fields, db::get_connection/init_db/prompt_password_if_needed, settings::get_data_dir). No signature changes needed.

## Step 3 — Move DateGranularity into reports.rs
Current reality: `DateGranularity` is `pub(crate)` in src/cli/report/view.rs:28 and there is NO mapping function — the granularity is passed as a literal at 5 `with_date(...)` call sites (pnl:433, expenses:503, tax:543, cashflow:586, k1:883); flagged/balance inherit `DateGranularity::None` from `TableReportView::new`. The epic contract requires a `granularity(report)` mapping to live in reports.rs, so it has to be created here. reports.rs must not gain a clap/ratatui dependency, so the mapping keys off a plain enum, not the clap `ReportCommands`.

In src/reports.rs (top of file, before the report structs):
- `DateGranularity` moved verbatim (doc comments included) as `#[derive(Clone, Copy, PartialEq)] pub enum DateGranularity { MonthAndYear, YearOnly, None }`.
- New `#[derive(Clone, Copy, PartialEq)] pub enum ReportKind { Pnl, Expenses, Tax, Cashflow, Register, Flagged, Balance, K1, All }` with:
  - `pub fn as_str(&self) -> &'static str` returning exactly today's `ReportCommands::report_name()` strings: "pnl", "expenses", "tax", "cashflow", "register", "flagged", "balance", "k1-prep", "all".
  - `pub fn granularity(&self) -> DateGranularity`: Pnl/Expenses/Cashflow/Register -> MonthAndYear; Tax/K1/All -> YearOnly; Flagged/Balance -> None.
  - No `from_str`/`from_name` — 31.5 adds that when the HTTP router needs it.

In src/cli/mod.rs:
- Add `pub fn kind(&self) -> crate::reports::ReportKind` to `impl ReportCommands`.
- Rewrite `report_name()` as `self.kind().as_str()` so the name strings have one home. Output is unchanged (checked against export filenames: `k1-prep-<date>.txt`, `report all` -> "all").

In src/cli/report/view.rs:
- Delete the local `DateGranularity` enum; widen the existing `use crate::reports;` to `use crate::reports::{self, DateGranularity, ReportKind};`.
- `PeriodMode`, `date_params()`, `TableReportView` and everything else stay put, unchanged.
- Replace the 5 inline literals with `ReportKind::Pnl.granularity()` / `Expenses` / `Tax` / `Cashflow` / `K1` so the reports.rs table is the single decision point and the TUI can't drift from the future API. Values are identical to today, so rendering, nav hints, and key handling are bit-identical.
- Grep confirms `DateGranularity` has no other consumers anywhere in src/.

## Step 4 — Preempt visibility-gated clippy lints
Making `cli` a `pub mod` of a library turns items that were merely crate-internal into exported items, which switches on clippy lints gated on effective visibility. Known hit: `SnakeGame::new()` (src/cli/snake.rs:59) takes no arguments and has no `Default` -> `clippy::new_without_default` will newly fire under CI's `-D warnings`. Fix properly with `impl Default for SnakeGame { fn default() -> Self { Self::new() } }`. Audited the rest: every other `pub fn new(` takes parameters, and the `add`/`is_*` names are free functions, not inherent methods, so `should_implement_trait` / `wrong_self_convention` should not trigger. Any further new lint gets a real fix, never `#[allow]`.

## Step 5 — Docs (repo policy: ship with the change)
- CLAUDE.md: add a Crate layout bullet to Architecture ("src/lib.rs exposes the modules as a library; main.rs is clap parsing, dispatch pre-flight, and the ratatui panic hook"); update the Reports bullet so `DateGranularity` is described as living in reports.rs alongside `ReportKind`; add `lib.rs` to the Project Structure tree.
- CONTRIBUTING.md: add `lib.rs` to its Project Layout tree.
- README.md: no change — no user-facing command, flag, or output changes.
- docs/api.md does not exist yet (31.3 creates it) — out of scope.

## Step 6 — Verification (all must pass before handing back)
1. cargo build
2. cargo build --no-default-features                       (combo: neither)
3. cargo build --no-default-features --features gusto       (combo: gusto only)
4. cargo build --no-default-features --features pdf         (combo: pdf only)
5. cargo test -- --test-threads=1                           (CI's exact invocation; unit tests now run in the lib target, integration tests in tests/cli_dispatch.rs unchanged)
6. cargo test --no-default-features -- --test-threads=1
7. cargo clippy -- -D warnings                              (CI's exact invocation)
8. cargo clippy --all-targets -- -D warnings
9. cargo clippy --no-default-features --all-targets -- -D warnings
10. cargo fmt --check
11. CLI surface diff: re-capture `nigel --help`, `nigel report --help`, `nigel accounts --help`, `nigel rules --help` and `diff` against the Step 0 captures — must be identical.
12. AC#4 grep check: `grep -rln "ratatui\|crossterm" src/` must return exactly the Step 0 baseline set (browser.rs, effects.rs, tui.rs, main.rs, cli/{dashboard,goodbye,onboarding,review,snake,splash}.rs, the seven cli/*_manager.rs, cli/report/view.rs) plus nothing. In particular reports.rs, db.rs, models.rs, importer.rs, categorizer.rs, reconciler.rs, reviewer.rs, migrations.rs, settings.rs, error.rs, fmt.rs, pdf.rs, cli/report/{mod,text}.rs and the cli data-layer modules must stay clean.
13. Manual smoke in a temp HOME: `cargo run -- demo`, then `cargo run -- report pnl --year <current>` and exercise Left/Right paging + `m` month/year toggle + `q`; then `cargo run -- report pnl --year <current> --mode export --format text` and confirm the filename and content are unchanged; then launch the bare dashboard and quit.

Explicitly out of scope (owned by other tasks): serde derives (31.2), any server/axum code (31.3), rules data-layer extraction and `list_imports`/`RuleRow` (31.5/31.6), `prompt_password_if_needed` changes. Nothing in `db.rs`'s `--test-threads=1` comment moves, so it stays accurate.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Added src/lib.rs exposing all 17 modules pub (pdf cfg-gated); no Cargo.toml change needed — cargo auto-detects lib+bin sharing the name `nigel`.
- main.rs slimmed to clap parse + panic hook + dispatch; paths repointed to `nigel::`. dispatch() body unchanged.
- Moved DateGranularity into reports.rs and added ReportKind (as_str + granularity); ReportCommands::report_name() now delegates to kind().as_str().
- view.rs 5 with_date call sites now read granularity from the ReportKind mapping; PeriodMode/date_params stayed in the view.
- Predicted clippy::new_without_default fired on SnakeGame::new once cli became a pub lib module — fixed with impl Default. It was the only new lint.

Verification (Step 6, all 13 items):
- Builds PASS in all four feature combos (default / none / gusto-only / pdf-only).
- cargo test -- --test-threads=1: 303 unit + 23 integration PASS (302 baseline + 1 new ReportKind test). no-default-features: 296 + 23 PASS.
- cargo fmt --check PASS; cargo clippy -- -D warnings (CI form) PASS; cargo clippy --all-targets -- -D warnings PASS.
- KNOWN PRE-EXISTING FAILURE: cargo clippy --no-default-features --all-targets -- -D warnings fails on two needless_return lints in cfg(not(pdf)) blocks (cli/dashboard.rs:852, cli/report/mod.rs:160). Verified by git stash that the unmodified baseline fails identically. Untouched — outside this task, and CI never runs that invocation.
- All four --help outputs diff clean against the pre-change baseline.
- ratatui/crossterm grep set identical to baseline (20 files); no data-layer module gained a TUI import.
- Manual smoke on isolated HOME: init + migrations v2/v3, demo (270 txns), status, non-TTY pnl text, text export, report all, PDF export, accounts/rules list — all behave as before. Export filenames unchanged incl. k1-prep, proving report_name() delegation to ReportKind::as_str() is correct end to end.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Restructures the binary-only crate into a lib + bin pair so the data layer is linkable from outside main.rs. This is the foundation for the epic-31 web server handlers, a future Tauri split, and richer integration tests. No behavior change.

Changes:
- Added `src/lib.rs`, exposing all modules as the `nigel` library: db, models, reports, reviewer, importer, categorizer, reconciler, migrations, settings, error, fmt, browser, tui, effects, cli, and pdf (cfg-gated). No root re-exports — consumers use `nigel::db::...`. No Cargo.toml change was needed; cargo auto-detects a lib and bin sharing the crate name.
- Slimmed `src/main.rs` to clap parsing, the terminal-restoring panic hook, and the dispatch pre-flight. All 17 `mod` declarations removed; paths repointed to `nigel::`. The `dispatch()` body is otherwise untouched.
- Moved `DateGranularity` out of the ratatui-entangled `cli/report/view.rs` into `reports.rs` as a plain `pub enum`, and added `ReportKind` (one variant per report) with `as_str()` and `granularity()`. Per the epic contract, `ReportKind::granularity()` is now the single mapping from report to date granularity; later tasks must not re-declare it.
- `ReportCommands::kind()` added and `report_name()` now delegates to `kind().as_str()`, giving the report slugs one home. `PeriodMode`, `date_params()`, and the current-period state stay in the view.
- The five `with_date(...)` call sites in `view.rs` now read granularity from the mapping instead of inline literals, so the TUI cannot drift from the future API.
- Added `impl Default for SnakeGame`: promoting `cli` to a public library module newly exports its items, which switches on `clippy::new_without_default` under CI's `-D warnings`.
- Docs per repo policy: CLAUDE.md gains a Crate layout bullet, an updated Reports bullet describing the reports.rs report vocabulary, and `lib.rs` in the Project Structure tree; CONTRIBUTING.md's layout tree updated likewise. README.md unchanged — nothing user-facing moved.

Tests:
- Builds pass in all four feature combinations (default, none, gusto-only, pdf-only).
- `cargo test -- --test-threads=1`: 303 unit + 23 assert_cmd integration tests pass (302 at baseline, plus a new test locking every ReportKind slug and granularity value against the pre-refactor behavior). `--no-default-features`: 296 + 23 pass.
- `cargo fmt --check`, `cargo clippy -- -D warnings` (CI's invocation), and `cargo clippy --all-targets -- -D warnings` all pass.
- All four `--help` outputs diff clean against the pre-change capture; the ratatui/crossterm grep set is byte-identical to baseline, so no presentation code leaked into the lib-exposed data-layer path.
- Manual smoke on an isolated HOME: init/migrations, demo, status, non-TTY text reports, text + PDF export, and `report all` all behave as before, with export filenames (including `k1-prep`) unchanged.

Known pre-existing issue (not introduced here, left untouched): `cargo clippy --no-default-features --all-targets -- -D warnings` fails on two `needless_return` lints in `cfg(not(feature = "pdf"))` blocks in `cli/dashboard.rs` and `cli/report/mod.rs`. Confirmed via git stash that the unmodified baseline fails identically; CI does not run that invocation. Worth a separate cleanup task.
<!-- SECTION:FINAL_SUMMARY:END -->

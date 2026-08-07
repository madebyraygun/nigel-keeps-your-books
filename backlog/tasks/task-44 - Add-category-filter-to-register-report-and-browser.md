---
id: TASK-44
title: Add --category filter to the register report and browser
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-06 22:10'
updated_date: '2026-08-07 00:51'
labels:
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

There is no way to filter transactions by category anywhere in the read paths. `nigel report register` and `nigel browse register` accept date bounds and `--account`, but answering "which transactions are in Taxes & Licenses this year?" requires exporting the register to text and grepping the category column, or abusing `nigel recategorize --from-category … --dry-run` as a query tool (which additionally hides rows already in the `--category` target, so a self-move preview prints only a skip count).

## Proposal

Add `--category <NAME>` and `--uncategorized` to both register surfaces:

- `nigel report register` (`src/cli/mod.rs` ReportCommands::Register, `src/reports.rs` register query) — flows into the register WHERE clause the same way the existing `--account` filter does, composing with date bounds and `--account`. Works in both view and export modes.
- `nigel browse register` (`src/cli/mod.rs` BrowseCommands::Register, `src/browser.rs`) — same flags, shown in the header like the account filter.

Name resolution should match `recategorize --from-category`: exact name match, `QueryReturnedNoRows` → `NigelError::UnknownCategory` listing the failure clearly, other DB errors propagated (see `src/cli/recategorize.rs` for the pattern).

## Filter visibility in exports

Only the interactive view surfaces active filters today (`src/cli/report/view.rs`, `filter_desc`). Exports drop them: `text::register` emits the company name alone via `with_header`, `pdf::render_register` receives a date `range` label but no filter label, and the default export filename comes from `report_name()` — a bare `register` no matter what `--account` was passed. A filtered export is therefore indistinguishable from an unfiltered one once it is on disk.

Close that gap for account and category together:

- Render the active filters into the text and PDF report headers alongside the date range.
- Encode the active filters into the default export filename in slug form, so `--account 'BofA Checking' --category 'Taxes & Licenses'` writes `register-bofa-checking-taxes-licenses-<date>.txt`. An explicit `--output` still wins.

This intentionally changes existing `--account` export output, so that both filters behave the same way.

## Relevant code

- `src/reports.rs` — register query and `date_filter`
- `src/cli/report/` — dispatch, view, text export, default export path
- `src/cli/export.rs` and `src/pdf.rs` — PDF register rendering and default path
- `src/browser.rs` — interactive register browser
- `src/cli/recategorize.rs` — category name-resolution error handling to mirror
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `nigel report register --category "Name"` filters to that category in view and export modes, composing with `--year`/`--month`/`--from`/`--to` and `--account`
- [x] #2 `nigel browse register --category "Name"` filters the interactive browser and shows the active filter in the header
- [x] #3 An unknown category name is a hard error naming the category; real DB errors are not misreported as unknown-name
- [x] #4 `--uncategorized` selects rows with no category on both surfaces and is mutually exclusive with `--category`
- [x] #5 Text and PDF register exports render the active --account/--category/--uncategorized filters in the report header alongside the date range
- [x] #6 The default export filename for a filtered register encodes the active filters in slug form (e.g. register-bofa-checking-taxes-licenses-2026-08-06.txt); an explicit --output path still takes precedence
- [x] #7 Update test coverage (register query unit tests + CLI integration tests, covering export header and filename for --account as well as the new category flags)
- [x] #8 Create or update documentation (README, CLAUDE.md Commands), making sure to remove any out of date information
- [x] #9 All linting checks pass
- [x] #10 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. reports.rs: add `CategorySelection` (Named{id,name} | Uncategorized) and `RegisterFilters { account, category }` with `resolve()` (mirrors recategorize name resolution), `labels()` for headers, and `slug_parts()` for filenames. Change `get_register` to take `&RegisterFilters` instead of `account: Option<&str>` (keeps arg count under the clippy threshold).
2. fmt.rs: add `slugify()` for filename parts.
3. cli/mod.rs: add `--category` and `--uncategorized` (clap `conflicts_with`) to both `ReportCommands::Register` and `BrowseCommands::Register`.
4. cli/report/text.rs: register header shows date range + active filters (new `with_subtitle` helper).
5. cli/report/mod.rs: thread filters through dispatch; `default_text_path` takes filter slug parts derived from the raw CLI strings.
6. cli/export.rs + pdf.rs: `render_register` gains a `filters` param rendered in the PDF header; default PDF path uses the same slug parts.
7. cli/browse.rs + report/view.rs: add category/uncategorized to the browser filter description.
8. Tests: reports.rs unit tests (category filter, uncategorized, composition with date/account, unknown category vs real DB error), fmt slugify tests, text header test, integration tests in tests/cli_dispatch.rs (export filename + header, unknown-category exit, mutual exclusion).
9. Docs: README + CLAUDE.md Commands; note the export header/filename change applies to `--account` too.
10. cargo fmt + clippy + full test suite.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Bundled the three filter flags into a shared `RegisterFilterArgs` clap struct (cli/mod.rs), flattened into both `report register` and `browse register`, rather than threading three params through six call sites.
- `get_register` now takes `&RegisterFilters` in place of `account: Option<&str>`; `RegisterFilters::resolve` does the DB-backed name validation up front, so every surface fails identically on an unknown category and real rusqlite errors are never rewritten as unknown-name.
- Moved `date_range_label` out of the pdf-gated cli/export.rs into cli/report/mod.rs so text exports can use it, and corrected its no-date-filter case: it returned "FY <current year>" for reports that actually span every year in the database. It now returns "All dates". This also affects the existing pnl/tax/cashflow PDF headers.
- `report register`'s interactive header now lists year alongside the other filters, matching `browse register`; previously account suppressed the year entirely.
- Verified end-to-end against demo data: filtered text and PDF export headers and filenames, uncategorized selection, unknown-category exit code, clap conflict message, and the browse TUI footer captured in a pty.

Draft PR: https://github.com/madebyraygun/nigel-keeps-your-books/pull/181
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds `--category <NAME>` and `--uncategorized` to both register surfaces, and makes active filters visible wherever a register goes.

Filtering:
- `nigel report register` and `nigel browse register` share a new `RegisterFilterArgs` clap struct (`--account`, `--category`, `--uncategorized`), so the two surfaces cannot drift. `--category` and `--uncategorized` are mutually exclusive via clap.
- `reports::RegisterFilters::resolve()` validates the category name against the database before any query runs, mirroring `recategorize --from-category`: `QueryReturnedNoRows` becomes `NigelError::UnknownCategory`, every other rusqlite error propagates unchanged. `get_register` now takes `&RegisterFilters` instead of a bare account name, and composes the category clause with the existing date and account clauses.

Filter visibility (new behavior, applies to `--account` too):
- Previously only the interactive view surfaced active filters. Text and PDF exports dropped them entirely, and the default export filename was a bare `register-<date>` no matter what was filtered — a filtered export was indistinguishable from an unfiltered one on disk.
- Text and PDF register headers now read e.g. `FY 2025 — account: BofA Checking, category: Taxes & Licenses`, and the default export filename encodes the same filters in slug form: `register-bofa-checking-taxes-licenses-<date>.txt`. An explicit `--output` still wins.

Drive-by correctness fix: `date_range_label` returned "FY <current year>" when no date filter was supplied, even though such reports span every year in the database. It now returns "All dates". This changes the existing pnl/tax/cashflow PDF headers as well as the new register text headers.

Tests: 10 new register-filter unit tests in `reports.rs` and `cli/report/mod.rs` (category filter, uncategorized, composition with account and date bounds, unknown-name vs. real DB error, labels and slugs, date-range labels), 8 new integration tests in `tests/cli_dispatch.rs` (row narrowing, header contents, export filename with and without filters, `--output` precedence, unknown category exit, clap conflict, browse rejection). Full suite: 336 unit + 47 integration passing; `cargo fmt --check` and `cargo clippy -- -D warnings` clean.

Docs: README Features and Quick Start, CLAUDE.md Architecture, Commands, Key Design Constraints, and Project Structure.

Risk: the export header and filename change is intentionally visible for existing `--account` users — anything scripted against the old bare `register-<date>` filename for a filtered export will need updating.
<!-- SECTION:FINAL_SUMMARY:END -->

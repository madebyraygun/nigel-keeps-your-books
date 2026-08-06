---
id: TASK-44
title: Add --category filter to the register report and browser
status: To Do
assignee: []
created_date: '2026-08-06 22:10'
updated_date: '2026-08-06 23:46'
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
- [ ] #1 `nigel report register --category "Name"` filters to that category in view and export modes, composing with `--year`/`--month`/`--from`/`--to` and `--account`
- [ ] #2 `nigel browse register --category "Name"` filters the interactive browser and shows the active filter in the header
- [ ] #3 An unknown category name is a hard error naming the category; real DB errors are not misreported as unknown-name
- [ ] #4 `--uncategorized` selects rows with no category on both surfaces and is mutually exclusive with `--category`
- [ ] #5 Text and PDF register exports render the active --account/--category/--uncategorized filters in the report header alongside the date range
- [ ] #6 The default export filename for a filtered register encodes the active filters in slug form (e.g. register-bofa-checking-taxes-licenses-2026-08-06.txt); an explicit --output path still takes precedence
- [ ] #7 Update test coverage (register query unit tests + CLI integration tests, covering export header and filename for --account as well as the new category flags)
- [ ] #8 Create or update documentation (README, CLAUDE.md Commands), making sure to remove any out of date information
- [ ] #9 All linting checks pass
- [ ] #10 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

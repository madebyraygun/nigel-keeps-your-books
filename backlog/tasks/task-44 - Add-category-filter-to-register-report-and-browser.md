---
id: TASK-44
title: Add --category filter to the register report and browser
status: To Do
assignee: []
created_date: '2026-08-06 22:10'
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

Add `--category <NAME>` to both register surfaces:

- `nigel report register` (`src/cli/mod.rs` ReportCommands::Register, `src/reports.rs` register query) — flows into the register WHERE clause the same way the existing `--account` filter does, composing with date bounds and `--account`. Works in both view and export modes; export filename/header should reflect the filter the way the account filter does.
- `nigel browse register` (`src/cli/mod.rs` BrowseCommands::Register, `src/browser.rs`) — same flag, shown in the header like the account filter.

Name resolution should match `recategorize --from-category`: exact name match, `QueryReturnedNoRows` → `NigelError::UnknownCategory` listing the failure clearly, other DB errors propagated (see `src/cli/recategorize.rs` for the pattern). Consider `--uncategorized` as a companion flag for rows with `category_id IS NULL`, mirroring recategorize's selection vocabulary.

## Relevant code

- `src/reports.rs` — register query and `date_filter`
- `src/cli/report/` — dispatch, view, text/pdf export
- `src/browser.rs` — interactive register browser
- `src/cli/recategorize.rs` — category name-resolution error handling to mirror
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report register --category "Name"` filters to that category in view and export modes, composing with `--year`/`--month`/`--from`/`--to` and `--account`
- [ ] #2 `nigel browse register --category "Name"` filters the interactive browser and shows the active filter in the header
- [ ] #3 An unknown category name is a hard error naming the category; real DB errors are not misreported as unknown-name
- [ ] #4 `--uncategorized` selects rows with no category on both surfaces and is mutually exclusive with `--category`
- [ ] #5 Update test coverage (register query unit tests + CLI integration tests)
- [ ] #6 Create or update documentation (README, CLAUDE.md Commands), making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

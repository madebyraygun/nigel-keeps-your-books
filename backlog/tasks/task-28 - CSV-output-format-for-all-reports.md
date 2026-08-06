---
id: TASK-28
title: CSV output format for all reports
status: To Do
assignee: []
created_date: '2026-08-05 23:30'
labels:
  - enhancement
  - reports
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add `--format csv` alongside the existing pdf|text options so every report can be exported as machine-readable CSV, not just rendered for humans.

## Motivation

Every current export path produces something formatted for reading: PDF via printpdf, or comfy_table text. Neither round-trips into a spreadsheet, tax package, or accountant workflow without retyping. CSV is the lowest-friction interchange format and unblocks handing report data to third parties.

## Current shape

- `src/cli/report/mod.rs` dispatches on mode: `dispatch_view`, `dispatch_text`, `dispatch_export`, with `export_text` / `dispatch_pdf_export` / `export_all_text` as the export arms.
- `format` is an `Option<String>` (src/cli/mod.rs:330, :424), not a clap `ValueEnum`, so accepted values are matched as strings and an unknown value needs explicit rejection. Converting to a `ValueEnum` is the natural cleanup and overlaps with TASK-15 (same freeform-String-to-enum pattern for rule match_type).
- Each report has its own row shape, so CSV needs a per-report column definition rather than one generic writer. Reports with sections (P&L income vs expenses, K-1 deduction lines vs Schedule K items vs needs-mapping) need a decision on whether sections become a column or separate files.

## Relationship to TASK-27

TASK-27 (trial balance) needs CSV output for TaxAct import and scopes a trial-balance-only CSV writer. If this task lands first, TASK-27 inherits the plumbing and drops that AC. Either order works; whichever lands second should not duplicate the writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `--format csv` is accepted by `nigel report <kind> --mode export` for every report kind
- [ ] #2 An unknown --format value fails with a clear error listing the valid options rather than silently defaulting
- [ ] #3 Each report defines an explicit column header row; values are unformatted (raw numbers, no currency symbols, thousands separators, or ANSI color)
- [ ] #4 Fields containing commas, quotes, or newlines are correctly quoted and escaped
- [ ] #5 Multi-section reports (pnl, k1) represent sections in a way that survives a spreadsheet round-trip, documented in the report docs
- [ ] #6 `nigel report all --format csv --output-dir <dir>` writes one CSV per report
- [ ] #7 Non-TTY invocation with --format csv writes CSV to stdout so it can be piped
- [ ] #8 Tests cover the escaping edge cases and assert header/row counts for at least pnl, tax, and register
- [ ] #9 CLAUDE.md (Reports architecture, Commands) and README.md are updated
<!-- AC:END -->

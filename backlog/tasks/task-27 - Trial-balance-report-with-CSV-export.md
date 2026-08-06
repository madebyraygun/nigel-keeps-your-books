---
id: TASK-27
title: Trial balance report with CSV export
status: To Do
assignee: []
created_date: '2026-08-05 23:26'
updated_date: '2026-08-05 23:30'
labels:
  - enhancement
  - reports
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add a trial balance report that lists every account and category with Debit/Credit columns and ties to zero, exportable as CSV for tax-software import (TaxAct Business accepts a Trial Balance CSV upload).

## Why this is not already possible

Nigel is cash-basis single-entry, but a balancing trial balance is derivable from existing data:

    Assets(12/31/Y) - Liabilities(12/31/Y) = Equity(12/31/Y-1) + Income(Y) - Expenses(Y)

Every transaction hits exactly one account and one category, so the two sides tie automatically as long as nothing is uncategorized. What is missing today:

- `get_balance()` in src/reports.rs has no date filter. It reports balances as of today, so it cannot produce year-end balance-sheet figures. An as-of-date balance function is the main new primitive, and `get_balance()` would benefit from it regardless.
- There is no equity concept. Prior-year retained earnings has to be computed as a plug row.
- Owner distributions/contributions currently live as expense categories, so they would import into tax software as deductions and overstate expenses. They belong in the equity section.
- Reports support --format pdf|text only. CSV is a new output format.

## Relationship to TASK-9

TASK-9 (journal entry layer) lists trial balance as something it enables. This task deliberately does not wait for it: the numbers are derivable from single-entry today, and TASK-9 is a large architectural change with no date. If TASK-9 lands later, this report should be reimplemented as a straight query over journal lines and the derivation logic dropped. Not a blocking dependency in either direction.

## Priority caveat

TaxAct does not publish a required column format ("generally Account or Description plus Debit and Credit amounts"), and their assistant indicated the K-1 prep worksheet already carries the 2025 income and deduction detail needed to complete the interview without a trial balance import. So this may be convenience rather than necessity for the immediate filing. Scoped now; worth confirming the real need before starting.

## Out of scope

Schedule L balance-sheet items Nigel does not track: fixed assets, depreciation, A/R, A/P, loan principal. Those stay manual.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report trialbalance --year <Y>` outputs one row per bank/liability account and per income/expense category with separate Debit and Credit columns
- [ ] #2 Balance-sheet rows use balances as of the last day of the reporting year, not as of today
- [ ] #3 An opening equity row carries prior-year retained earnings so total debits equal total credits to the cent
- [ ] #4 Owner distribution and contribution categories are reported under equity, not as deductions
- [ ] #5 The report warns and names the offending transactions when uncategorized transactions exist on or before the reporting year end, since the balance cannot tie
- [ ] #6 Transfer-category rows that net to zero are omitted rather than emitted as zero-value lines
- [ ] #7 `--format csv` produces an Account/Debit/Credit CSV that imports into TaxAct Business without manual editing
- [ ] #8 `--mode export --output <path>` writes the CSV to a file; non-TTY invocation falls back to plain stdout like other reports
- [ ] #9 A test asserts debits equal credits across a multi-year fixture including a mid-period year
- [ ] #10 CLAUDE.md (Architecture, Commands) and README.md are updated
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TASK-28 (CSV output format for all reports) now covers the generic CSV writer. If TASK-28 lands first, AC #7 here narrows to defining the trial-balance column set and AC #8 drops entirely.
<!-- SECTION:NOTES:END -->

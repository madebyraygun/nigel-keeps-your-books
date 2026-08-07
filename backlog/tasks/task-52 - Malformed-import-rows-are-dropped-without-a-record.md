---
id: TASK-52
title: Malformed import rows are dropped without a record
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - bug
  - importer
  - data-loss
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every parser increments a malformed counter and continues. The row's content is never captured — not into transactions, not into a rejects table, not into flag_reason, not to stderr. The imports row records parsed_rows.len(), which excludes malformed rows, so the import history agrees with the truncated import.

The count reaches the user once, in the response, and then ceases to exist. A bank changing its date format mid-statement drops those rows silently; the P&L is wrong by whatever they totalled and nothing in the database, the import history or any report records that the books are incomplete.

For a cash-basis bookkeeping tool this is the worst shape of failure available: not a swallowed error but swallowed financial data, with the count of what was swallowed held only in volatile UI state.

Pre-existing on main. Minimum useful fix is persisting the malformed count on the imports row so the history and nigel status can surface it. Better is capturing the rejected lines (row number plus raw text) so the user can see what was dropped and correct it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The malformed count is persisted with the import and visible in the import history
- [ ] #2 The rejected rows are recoverable well enough to diagnose why they failed
- [ ] #3 nigel status or a report surfaces that an account's books have dropped rows
<!-- AC:END -->

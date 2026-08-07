---
id: TASK-46
title: Support point-in-time balances via --as-of date and opening balances
status: To Do
assignee: []
created_date: '2026-08-06 23:05'
labels:
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

`nigel report balance` only reports the cash position as of now, and computed balances are wrong anyway for any account whose history predates the first import: transactions start at whatever date the first statement covered, with no opening-balance entries, so summing transactions omits each account's pre-import balance. Answering "what was cash at 1/1 and 12/31?" (asked by tax software for Schedule L) currently means going back to bank statement PDFs by hand.

## Proposal

Two pieces, useful together:

1. **Per-account opening balances** — an `opening_balance` and `opening_balance_date` on accounts (schema migration; `nigel accounts` flags to set them, e.g. `nigel accounts update <ID> --opening-balance 13994.73 --opening-date 2025-01-01`). Balance math becomes opening balance + sum of transactions after the opening date. Accounts default to 0/none, preserving current behavior.
2. **`--as-of <YYYY-MM-DD>` on `nigel report balance`** — reports each account's balance at end of that date (opening balance + transactions through the date), in both view and export modes. Header shows the as-of date. A date before an account's opening-balance date should say so rather than print a misleading number.

With both, beginning/end-of-year cash is `nigel report balance --as-of 2024-12-31` and `--as-of 2025-12-31`.

Reconciliation (`src/reconciler.rs`) already compares calculated month-end balances to statement balances — worth reusing/aligning its calculation, and reconcile gains accuracy from opening balances too.

Relevant code: `src/db.rs` (accounts schema + migration), `src/cli/accounts.rs`, `src/reports.rs` (balance report), `src/cli/report/`, `src/reconciler.rs`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Accounts can store an opening balance and its date, settable via the accounts CLI; schema migration covers existing databases
- [ ] #2 `nigel report balance --as-of <date>` reports per-account balances at that date incorporating opening balances, in view and export modes
- [ ] #3 Without --as-of, behavior matches today (current position), now also incorporating opening balances
- [ ] #4 An --as-of date earlier than an account's opening-balance date is reported clearly, not silently miscomputed
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

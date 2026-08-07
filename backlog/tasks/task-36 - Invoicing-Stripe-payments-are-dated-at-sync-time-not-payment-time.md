---
id: TASK-36
title: 'Invoicing: Stripe payments are dated at sync time, not payment time'
status: To Do
assignee: []
created_date: '2026-08-06 19:14'
labels:
  - bug
  - invoicing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`PaidSession` does not carry the Stripe session timestamp, so `invoice sync` records `paid_date` as the day the sync ran rather than the day the client actually paid.

A payment made in late December and synced in January is recorded in the wrong calendar year. This does not corrupt the P&L — the bank transaction remains the cash-basis source of truth — but it misdates the invoice record, distorts aging, and puts the invoicing view of the year at odds with the books.

Carried over from the review ledger of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `PaidSession` carries the Stripe session completion timestamp
- [ ] #2 `invoice sync` records paid_date from that timestamp, not from the sync date
- [ ] #3 A payment completed before year end but synced after it is recorded in the earlier year
- [ ] #4 Sync falls back to a defined, documented date when Stripe returns no timestamp, rather than silently using today
- [ ] #5 Parser tests cover the timestamp field, including its absence
<!-- AC:END -->

---
id: TASK-71
title: 'Invoicing: update_invoice derives status against the issue date, not today'
status: To Do
assignee: []
created_date: '2026-08-08 08:22'
labels:
  - invoicing
  - bug
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
update_invoice passes the invoice's issue date to refresh_status as "today", so a due-date edit derives overdue relative to the issue date rather than the wall clock. Harmless today because only drafts are editable and drafts never derive overdue — but it is a trap if edit is ever widened (TASK-35's stale-overdue work touches the same derivation). Fix by threading the real today through, matching void_invoice/record_payment. Surfaced during TASK-68.6 stage 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 refresh_status is always called with the wall-clock today, and a test pins the due-date-edit path
<!-- AC:END -->

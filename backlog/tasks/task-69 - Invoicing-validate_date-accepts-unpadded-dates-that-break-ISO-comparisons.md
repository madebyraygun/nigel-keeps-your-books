---
id: TASK-69
title: 'Invoicing: validate_date accepts unpadded dates that break ISO comparisons'
status: To Do
assignee: []
created_date: '2026-08-08 04:33'
labels:
  - invoicing
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
invoices::validate_date (68.1's shared rule) accepts chrono-parseable but unpadded dates like 2026-8-7. Stored in paid_date/due_date/issue_date, such a value breaks refresh_status's ISO string comparison against due_date and ar_aging's parse_from_str, whose failure path silently falls back to *today* — an invoice can misreport overdue status or aging bucket with no error. Fix: normalize to zero-padded %Y-%m-%d on validation (re-format the parsed date rather than storing the input verbatim), and add regression tests for an unpadded date round-tripping through pay/edit and landing padded in the row. Surfaced during TASK-68.4 implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An unpadded but valid date entered anywhere (new/edit/pay, CLI or TUI) is stored zero-padded
- [ ] #2 refresh_status and ar_aging behave identically for dates entered padded or unpadded
<!-- AC:END -->

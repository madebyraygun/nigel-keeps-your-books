---
id: TASK-68.4
title: 'TUI: client and invoice management screens'
status: To Do
assignee: []
created_date: '2026-08-08 00:28'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Following the manager-screen pattern (account_manager.rs, category_manager.rs): a Clients screen (list, add, edit) and an Invoices screen (scrollable list with status/total/due, detail view with line items and payments, and actions — send with confirmation, record payment form, void with confirmation). Both reachable from the dashboard command chooser with shortcuts. Reuses the CLI data layer from 68.1; send/pay honor the same guards and error wording the CLI prints.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clients screen lists, adds, and edits clients from the dashboard
- [ ] #2 Invoices screen lists and shows invoices with line items, payments, and balance
- [ ] #3 Send, record-payment, and void actions work from the invoice screen with confirmations
- [ ] #4 Dashboard command chooser gains shortcuts for both screens
<!-- AC:END -->

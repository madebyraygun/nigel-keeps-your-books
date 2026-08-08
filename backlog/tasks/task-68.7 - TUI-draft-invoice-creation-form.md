---
id: TASK-68.7
title: 'TUI: draft-invoice creation form'
status: To Do
assignee: []
created_date: '2026-08-08 01:02'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-68
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
68.4 ships the invoice screens without creation — the empty state points at `nigel invoice new`. This subtask adds a draft form to the invoice manager: client selector, issue/due dates (validate_date), currency, and repeatable line-item rows (the hardest TUI form in the app — study how import_manager and reconcile_manager handle multi-field forms, and design row add/remove/edit keys). Creates through invoicing::create_invoice; drafts only — send stays a separate deliberate action. Depends on 68.1 (validation) and 68.4 (the screens it extends).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A draft invoice can be created entirely from the TUI, including multiple line items
- [ ] #2 Validation failures render beside the field with the CLI's wording
- [ ] #3 The invoice-list empty state stops pointing at the CLI once the form exists
<!-- AC:END -->

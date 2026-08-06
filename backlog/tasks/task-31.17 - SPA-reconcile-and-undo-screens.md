---
id: TASK-31.17
title: 'SPA: reconcile and undo screens'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.6
  - TASK-31.9
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two small form screens. Reconcile: account, month, and statement balance inputs showing the reconciled-or-discrepancy result plus past reconciliations for context. Undo: list recent imports with details (filename, account, date, transaction count) and confirm undo of a selected import — the web UI supersets the TUI last-import-only behavior since delete_import already takes an id.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Reconcile form returns and displays the reconciled or discrepancy result and lists prior reconciliations
- [ ] #2 Undo screen lists recent imports with details and undoes a selected import after confirmation
- [ ] #3 Dependent screens (dashboard, register) reflect changes after either action
<!-- AC:END -->

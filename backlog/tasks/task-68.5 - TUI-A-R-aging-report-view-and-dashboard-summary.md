---
id: TASK-68.5
title: 'TUI: A/R aging report view and dashboard summary'
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
The aging report joins the interactive report views (ReportView pattern in cli/report/view.rs) so it is browsable like pnl/expenses, and the dashboard home gains an A/R line (outstanding total, oldest bucket) next to the YTD P&L so money owed is visible on launch. Invoice list/show stay in the Invoices screen (68.4); this task is the reporting half.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Aging is available as an interactive report view alongside the existing reports
- [ ] #2 Dashboard home shows outstanding A/R when any open invoices exist
<!-- AC:END -->

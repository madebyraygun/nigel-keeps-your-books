---
id: TASK-31.11
title: 'SPA: dashboard screen'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.11-dashboard.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web home mirroring the TUI dashboard (cli/dashboard.rs): YTD P&L summary, account balances, monthly income/expense bar chart (get_cashflow data), flagged-transaction count with a call to action into review, quick navigation to the major screens, and the update-available notice when the server-side check finds a newer release.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Shows YTD income, expenses, and net; account balances; and a twelve-month income/expense chart from the API
- [ ] #2 Flagged count badge links into the review flow; quick links reach all major screens
- [ ] #3 A manual refresh action reloads dashboard data
- [ ] #4 Update-available notice appears when the server reports a newer release
<!-- AC:END -->

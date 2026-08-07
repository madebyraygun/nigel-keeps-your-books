---
id: TASK-48
title: Enforce last-four format server-side in accounts data layer
status: To Do
assignee: []
created_date: '2026-08-07 12:02'
labels:
  - tech-debt
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The 4-digit last-four rule for accounts lives only in the TUI (account_manager.rs); accounts::add_account and POST /api/accounts accept any string. Validate in the data layer (exactly 4 ASCII digits or null) so CLI, TUI, and API agree. Noted during task 31.16 planning; the web form enforces it client-side in the meantime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 add_account rejects a non-4-digit lastFour with a clear message across CLI and API
<!-- AC:END -->

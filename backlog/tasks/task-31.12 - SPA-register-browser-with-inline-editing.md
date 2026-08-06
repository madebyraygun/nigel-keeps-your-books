---
id: TASK-31.12
title: 'SPA: register browser with inline editing'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.6
  - TASK-31.9
references:
  - src/browser.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.12-register.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web version of browser.rs, the richest screen in the app: paginated or virtualized transaction table backed by the register endpoint, account filter, date-range navigation with scroll-to-today on load, incremental text search, row selection, inline category and vendor editing, and flag toggling. Reuse boxcraft-app table and form patterns.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Full register table with account filter, date range, and incremental search
- [ ] #2 Inline category and vendor edits and flag toggles persist via the API and update in place
- [ ] #3 Initial load lands at the current date, matching the TUI scroll-to-today behavior
- [ ] #4 Basic keyboard navigation works (arrows, enter to edit, escape to cancel)
<!-- AC:END -->

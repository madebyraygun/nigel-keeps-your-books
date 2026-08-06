---
id: TASK-31.5
title: 'JSON API: read-only endpoints (reports and lists)'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies:
  - TASK-31.2
  - TASK-31.3
references:
  - src/reports.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.5-read-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wrap the existing pure report and list functions in GET endpoints: pnl, expenses, tax, cashflow, balance, flagged, register, and k1 with year/month/from/to params matching existing CLI semantics (including the from/to must-be-a-pair rule), plus list endpoints for accounts, categories, rules, imports history, and saved CSV profile names. Handlers stay thin: open a connection per request via get_connection, call the data layer, serialize the result.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 GET endpoints exist for all eight reports with date params matching CLI semantics, including from/to pair validation
- [ ] #2 GET list endpoints exist for accounts, active categories, active rules, imports history, and CSV profile names
- [ ] #3 Errors map to structured JSON with appropriate HTTP status codes (unknown account 404, bad params 400)
- [ ] #4 The endpoint inventory is documented (docs/api.md or OpenAPI sketch)
<!-- AC:END -->

---
id: TASK-32.5
title: 'Audit log: table, hooks, and admin viewer'
status: To Do
assignee: []
created_date: '2026-08-06 16:28'
labels:
  - multiuser
  - backend
dependencies:
  - TASK-32.4
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An append-only audit_log table (timestamp, user, action, entity type and id, summary of before/after) written at the data-layer choke points for financially meaningful mutations: transaction edits, review decisions, category/account/rule changes, imports, undo, reconcile, settings and password changes, login events. Admin-only API and screen with filters by user, entity, and date range. The app exposes no update or delete for audit entries.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Financially meaningful mutations produce audit entries with user, action, entity, and details
- [ ] #2 Admin-only viewer filters by user, entity type, and date range
- [ ] #3 No API exists to modify or delete audit entries
- [ ] #4 Login successes and failures are recorded
<!-- AC:END -->

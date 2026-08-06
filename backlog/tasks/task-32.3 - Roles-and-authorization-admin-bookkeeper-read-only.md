---
id: TASK-32.3
title: 'Roles and authorization: admin, bookkeeper, read-only'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - multiuser
  - backend
dependencies:
  - TASK-32.2
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Server-side authorization enforced per route, not just hidden in the UI. Read-only can view reports, the register, and lists but cannot mutate anything including import and review. Bookkeeper can do all bookkeeping operations. Admin additionally manages users, settings, database password operations, and boot unlock. Document the role matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The role matrix is documented and enforced server-side on every route
- [ ] #2 Read-only users are rejected from all mutation endpoints including import, review, and categorize
- [ ] #3 Only admins reach user management, settings mutation, password operations, and unlock
- [ ] #4 Tests cover the matrix for each role
<!-- AC:END -->

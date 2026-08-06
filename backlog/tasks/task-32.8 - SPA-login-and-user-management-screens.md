---
id: TASK-32.8
title: 'SPA: login and user management screens'
status: To Do
assignee: []
created_date: '2026-08-06 16:28'
labels:
  - multiuser
  - frontend
dependencies:
  - TASK-32.3
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Login screen (multiuser mode only), current-user context in the app shell (name, role badge, logout), role-aware navigation that hides what the server would reject anyway, and an admin users screen: create user, change role, deactivate, reset password.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Login and logout flows work end to end in multiuser mode; single-user mode shows no login
- [ ] #2 Admin users screen covers create, role change, deactivate, and password reset
- [ ] #3 Navigation and actions are gated by role, matching the server matrix
- [ ] #4 Current user name and role are visible with a logout control
<!-- AC:END -->

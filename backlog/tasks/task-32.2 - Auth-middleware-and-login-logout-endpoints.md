---
id: TASK-32.2
title: Auth middleware and login/logout endpoints
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - multiuser
  - backend
dependencies:
  - TASK-32.1
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Session-cookie authentication in front of the API when multiuser mode is enabled: login and logout endpoints, HttpOnly SameSite cookies, session expiry, and failed-attempt throttling with lockout. Multiuser mode is an explicit setting; when off, the current single-user tokenized-URL behavior is unchanged. In multiuser mode the database unlock endpoint becomes admin-only (the key is held server-side after an admin unlocks at boot).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Login and logout work with HttpOnly SameSite session cookies and server-side expiry
- [ ] #2 Failed logins are throttled and repeated failures lock the account temporarily
- [ ] #3 Single-user mode remains the default with behavior identical to before
- [ ] #4 All /api routes require an authenticated session in multiuser mode; logout invalidates the session server-side
<!-- AC:END -->

---
id: TASK-31.4
title: 'JSON API: encrypted database unlock flow'
status: To Do
assignee: []
created_date: '2026-08-06 16:25'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/db.rs
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web equivalent of the splash-screen password gate. GET /api/status reports initialized/encrypted/locked state plus company name; POST /api/unlock validates the password (db::validate_password) and sets it for the server process (set_db_password). The server starts locked for encrypted databases and every data endpoint returns a distinct locked status until unlocked. Mirror the TUI three-attempt behavior with delay/backoff on failures.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 GET /api/status reports initialized, encrypted, and locked states
- [ ] #2 POST /api/unlock with a valid password unlocks the process; invalid attempts get attempts-remaining feedback and backoff
- [ ] #3 Data endpoints refuse with a distinct locked status until unlock; unencrypted databases skip the flow entirely
- [ ] #4 The password is never logged or persisted to disk
<!-- AC:END -->

---
id: TASK-32.1
title: 'Users and sessions: schema migration + data layer'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - multiuser
  - backend
dependencies: []
references:
  - src/migrations.rs
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Foundation only, no HTTP: a migration (append to MIGRATIONS in migrations.rs) adding a users table (username, display name, argon2 password hash, role, active flag) and a sessions store, plus data-layer functions for user create/list/deactivate, credential verification, and password change. Argon2 with per-user salts for user passwords — this is user identity, distinct from the SQLCipher database key.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Migration adds users and sessions tables and runs cleanly on existing databases via the migration runner
- [ ] #2 Passwords are hashed with argon2 and per-user salts; verification and password change functions exist
- [ ] #3 Data-layer functions cover create, list, deactivate, and verify with unit tests
- [ ] #4 No plaintext credential ever touches disk or logs
<!-- AC:END -->

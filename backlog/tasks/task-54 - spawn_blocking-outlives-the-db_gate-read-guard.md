---
id: TASK-54
title: spawn_blocking outlives the db_gate read guard on client disconnect
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - bug
  - server
  - concurrency
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
routes::with_conn_api, routes::imports::blocking and routes::status::current_status all take the db_gate read guard in the handler future and then await a spawn_blocking JoinHandle.

If the client disconnects, axum drops the handler future. That drops the guard and releases the read side — but a spawn_blocking task keeps running to completion, with a live Connection on the database file. The invariant routes/mod.rs asserts, that the read guard is held for the life of the connection, does not survive cancellation.

Scenario: a user starts a large PDF register export, closes the tab while it renders, then encrypts from the settings screen. The password route sees no readers, takes the write guard, and encrypt_database deletes the -wal and -shm sidecars and renames a temp file over nigel.db while the orphaned export connection still has the old file open — the outcome db_gate exists to make impossible.

The fix is to move an owned guard (OwnedRwLockReadGuard) into the blocking closure so it is released by the task rather than by the future.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The read guard is released by the blocking task, not by the handler future
- [ ] #2 A cancelled request cannot leave a connection open past the guard
- [ ] #3 A test covers dropping a request future while its blocking work is in flight
<!-- AC:END -->

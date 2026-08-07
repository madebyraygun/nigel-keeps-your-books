---
id: TASK-32.7
title: Optimistic concurrency for shared edits
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
Two users editing the same transaction should not silently clobber each other. Add a version or updated_at check to transaction edits and review apply: a stale write is rejected with 409 and the current state, and the SPA surfaces the conflict with a refresh-and-retry path. Keep it cheap — this is bookkeeping-scale contention, not collaborative editing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A stale concurrent edit is rejected with 409 and the response carries current state
- [ ] #2 Review apply is protected against the transaction having been recategorized meanwhile
- [ ] #3 The SPA shows the conflict and offers refresh and retry
- [ ] #4 Tests simulate the race for both edit and review paths
<!-- AC:END -->

---
id: TASK-32.6
title: Imports history and per-import undo with attribution
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
With several people importing, last-import undo semantics stop making sense. The imports list gains who-imported-it attribution, undo works on any chosen import by id with confirmation, and a second undo of the same import fails gracefully instead of deleting unrelated data. Undo actions land in the audit log.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Imports list shows importer identity alongside filename, account, date, and counts
- [ ] #2 Undo works on a selected import by id with confirmation
- [ ] #3 Undoing an already-undone import fails with a clear message and no side effects
- [ ] #4 Undo actions are audit-logged
<!-- AC:END -->

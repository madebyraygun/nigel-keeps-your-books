---
id: TASK-32.4
title: 'Write attribution: user context through the data layer'
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
Thread an explicit user context from authenticated handlers into every data-layer write, and add created_by/updated_by columns (nullable, migration) to transactions, rules, imports, and reconciliations. CLI and TUI writes keep working and record a null or local default identity. This is the one-choke-point payoff of all writes already flowing through data-layer functions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Migration adds created_by and updated_by columns without breaking existing databases
- [ ] #2 All write paths through the API record the acting user
- [ ] #3 Register and list responses expose attribution fields
- [ ] #4 CLI and TUI writes still work, recording a defined local identity or null
<!-- AC:END -->

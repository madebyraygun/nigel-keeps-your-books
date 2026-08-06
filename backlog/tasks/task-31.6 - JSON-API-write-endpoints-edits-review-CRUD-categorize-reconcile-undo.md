---
id: TASK-31.6
title: 'JSON API: write endpoints (edits, review, CRUD, categorize, reconcile, undo)'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/reviewer.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.6-write-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Mutation endpoints wrapping the existing write data layer: transaction category/vendor edits and flag toggle (reviewer.rs), review apply and undo (apply_review/undo_review including rule cleanup), CRUD for accounts, categories, and rules honoring the existing guardrails (delete blocked when transactions exist, soft-delete semantics, blocking_reason surfaced), rule pattern testing against real transactions, running the categorizer, reconcile, and undo of a specific import by id (undo::delete_import already takes an id).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Transaction edit endpoints cover category, vendor, and flag toggle; review apply and undo work including created-rule cleanup
- [ ] #2 CRUD endpoints for accounts, categories, and rules enforce existing blocking and soft-delete semantics with clear error payloads
- [ ] #3 Endpoints exist to test a rule pattern (dry run), run the categorizer, reconcile an account month, and undo a specific import by id
- [ ] #4 All mutations are rejected while the database is locked
<!-- AC:END -->

---
id: TASK-31.16
title: 'SPA: accounts, categories, and rules managers'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.6
  - TASK-31.9
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CRUD screens mirroring the three TUI managers with their guardrails. Accounts: list, add, rename, delete with blocked-delete reasons. Categories: list, add, edit (name, type, tax line, form line), soft-delete with usage guardrails and blocking_reason surfaced. Rules: list, add, edit (priority, category, pattern, match type), deactivate, and a pattern test preview against real transactions (parity with rules test). Web reaches CLI parity here, which is a superset of the TUI managers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Accounts manager supports list, add, rename, and delete with blocked-delete reasons shown
- [ ] #2 Categories manager supports list, add, edit of all fields, and soft-delete with guardrail reasons shown
- [ ] #3 Rules manager supports list, add, edit, deactivate, and a live pattern test preview
- [ ] #4 All guardrail errors from the API render as clear inline feedback
<!-- AC:END -->

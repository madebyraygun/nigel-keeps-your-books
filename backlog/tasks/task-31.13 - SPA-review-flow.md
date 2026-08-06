---
id: TASK-31.13
title: 'SPA: review flow'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.6
  - TASK-31.9
references:
  - src/reviewer.rs
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web equivalent of the interactive review: step through flagged transactions one at a time, pick a category (vendor optional), optionally create a categorization rule from a pattern, with back navigation that undoes the previous decision including any created rule (undo_review semantics, parity with TUI Esc) and skip that leaves the transaction flagged (parity with Tab). Progress indicator and completion summary. Support re-reviewing a single transaction by id, matching review --id.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sequential review of flagged transactions with category picker and optional rule creation
- [ ] #2 Back undoes the previous decision including any created rule; skip leaves the transaction flagged
- [ ] #3 Progress indicator during review and a summary at completion
- [ ] #4 A single transaction can be re-reviewed by id
<!-- AC:END -->

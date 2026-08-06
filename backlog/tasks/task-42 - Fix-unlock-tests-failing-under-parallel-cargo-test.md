---
id: TASK-42
title: Fix unlock tests failing under parallel cargo test
status: To Do
assignee: []
created_date: '2026-08-06 21:08'
labels:
  - tech-debt
  - testing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two of task 31.4's unlock integration tests fail under plain 'cargo test' (parallel) because they share the process-global DB password mutex; they pass serially. CI runs -- --test-threads=1 so CI is green, but a developer running bare 'cargo test' sees spurious failures. Fix by serializing the affected tests (shared test mutex or serial_test) rather than relying on the global invocation flag. Noted during task 31.5 verification.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plain 'cargo test' (parallel) passes reliably
<!-- AC:END -->

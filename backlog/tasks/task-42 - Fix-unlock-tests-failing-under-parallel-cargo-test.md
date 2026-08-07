---
id: TASK-42
title: Fix unlock tests failing under parallel cargo test
status: To Do
assignee: []
created_date: '2026-08-06 21:08'
updated_date: '2026-08-07 14:30'
labels:
  - tech-debt
  - testing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Multiple server tests (8 observed during task 31.13, including two of task 31.4's unlock tests) fail under plain parallel 'cargo test' because they share process-global state — the DB password Mutex and related server globals; they pass with -- --test-threads=1, which is what CI runs. A developer running bare 'cargo test' sees spurious failures. Fix by serializing the affected tests (shared test mutex or serial_test) rather than relying on the global invocation flag.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plain 'cargo test' (parallel) passes reliably
<!-- AC:END -->

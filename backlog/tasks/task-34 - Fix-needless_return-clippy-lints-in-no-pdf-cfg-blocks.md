---
id: TASK-34
title: Fix needless_return clippy lints in no-pdf cfg blocks
status: To Do
assignee: []
created_date: '2026-08-06 18:56'
labels:
  - tech-debt
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
cargo clippy --no-default-features --all-targets -- -D warnings fails on two needless_return lints in cfg(not(feature = "pdf")) blocks at cli/dashboard.rs:852 and cli/report/mod.rs:160. Pre-existing (predates the task-31.1 lib split, confirmed via stash on the unmodified baseline); CI never runs this invocation so it does not block, but it blocks anyone linting the no-default-features matrix locally.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cargo clippy --no-default-features --all-targets -- -D warnings passes
<!-- AC:END -->

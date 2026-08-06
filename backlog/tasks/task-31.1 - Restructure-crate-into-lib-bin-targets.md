---
id: TASK-31.1
title: Restructure crate into lib + bin targets
status: To Do
assignee: []
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies: []
references:
  - src/main.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.1-lib-split.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The crate is binary-only (main.rs declares all modules), so nothing outside main.rs can link against the data layer. Add src/lib.rs exposing the existing modules as a library and slim main.rs down to CLI dispatch over it. This is the foundation for the web server handlers, future Tauri workspace split, and richer integration tests. No behavior change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 src/lib.rs exposes db, models, reports, reviewer, importer, categorizer, reconciler, migrations, settings, error, and fmt (plus cli data-layer modules where needed)
- [ ] #2 The nigel binary builds and behaves identically for CLI and TUI
- [ ] #3 cargo test passes, including the assert_cmd integration tests
- [ ] #4 No presentation code (ratatui/crossterm usage) leaks into the lib-exposed data-layer path
<!-- AC:END -->

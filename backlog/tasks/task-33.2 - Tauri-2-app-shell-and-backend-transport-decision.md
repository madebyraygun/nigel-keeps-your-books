---
id: TASK-33.2
title: Tauri 2 app shell and backend transport decision
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
dependencies:
  - TASK-33.1
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scaffold the Tauri 2 app hosting the SPA, and settle the transport question: embedded axum server on a random port with a token (one API surface shared with web mode) versus native Tauri IPC commands wrapping nigel-core directly (no open port, more idiomatic). Implement the chosen transport as a backend behind the existing api client seam, record the decision and rationale in backlog/decisions, and document the desktop dev workflow.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app boots the SPA against a local database end to end
- [ ] #2 The transport decision is recorded in backlog/decisions with rationale
- [ ] #3 The api client seam holds: web mode still works from the same SPA code
- [ ] #4 Desktop dev workflow is documented
<!-- AC:END -->

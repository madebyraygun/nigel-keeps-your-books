---
id: TASK-33.3
title: Native file handling for imports
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - frontend
dependencies:
  - TASK-33.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace the browser upload dance with native affordances when running in the desktop shell: file-open dialog scoped to CSV/XLSX and drag-and-drop onto the window, passing paths straight to the path-based import pipeline. The web upload flow remains for remote mode.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Import works via native file dialog and window drag-and-drop in the desktop app
- [ ] #2 Preview and confirm behavior matches the web import flow
- [ ] #3 Remote mode falls back to the upload flow
<!-- AC:END -->

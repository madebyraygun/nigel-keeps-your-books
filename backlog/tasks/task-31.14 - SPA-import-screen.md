---
id: TASK-31.14
title: 'SPA: import screen'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.7
  - TASK-31.9
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser import flow: drag-and-drop plus file picker upload, account selector, optional format override including saved CSV profiles and a generic column-mapping form with save-as-profile, dry-run preview showing detected format, sample rows, and counts, then confirm and show results with a link into review for newly flagged transactions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Upload, preview, confirm flow works end to end with detected format and sample rows shown before anything is written
- [ ] #2 Account selector, format override, generic CSV column mapping, and save-profile are available
- [ ] #3 Results show imported, skipped, malformed, and flagged counts with a link into review
- [ ] #4 Duplicate files and duplicate rows are surfaced clearly
<!-- AC:END -->

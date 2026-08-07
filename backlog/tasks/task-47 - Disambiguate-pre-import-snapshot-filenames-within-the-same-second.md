---
id: TASK-47
title: Disambiguate pre-import snapshot filenames within the same second
status: To Do
assignee: []
created_date: '2026-08-07 11:46'
labels:
  - tech-debt
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pre-import snapshots are named pre-import-YYYYmmdd-HHMMSS.db; two imports in the same second produce the same name and the later snapshot clobbers the earlier. Rare from the CLI, easier to hit over the HTTP import API. Add a disambiguator (counter or import id) while keeping the naming recognizable; applies to CLI, TUI, and serve paths uniformly. Noted during task 31.7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Two imports within the same second produce distinct snapshot files
<!-- AC:END -->

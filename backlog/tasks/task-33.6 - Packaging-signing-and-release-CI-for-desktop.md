---
id: TASK-33.6
title: 'Packaging, signing, and release CI for desktop'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - ci
dependencies:
  - TASK-33.5
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Produce installable, trusted artifacts: macOS universal build with notarization, Windows installer with code signing (or a documented interim unsigned stance), Linux AppImage and deb. CI matrix builds on tag, versions aligned with the CLI, install instructions in the README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tagged releases produce installers for macOS (notarized universal), Windows, and Linux
- [ ] #2 Desktop and CLI report the same version for a given release
- [ ] #3 README documents desktop installation per platform
<!-- AC:END -->

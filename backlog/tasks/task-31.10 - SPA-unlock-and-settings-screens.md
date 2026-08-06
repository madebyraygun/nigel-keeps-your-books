---
id: TASK-31.10
title: 'SPA: unlock and settings screens'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.4
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.10-unlock-settings.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Unlock screen for encrypted databases gates the app before any data loads: masked input, attempts-remaining feedback, backoff — parity with the splash-screen flow. Settings screen covers what settings_manager.rs does today: edit business name (metadata company_name), toggle the auto-update check, show the active data directory, and manage the database password (set/change/remove with confirmation, calling new endpoints that wrap the existing password data layer). Include switching data directories (load) with a full app-wide refresh.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An encrypted database shows the unlock screen before any data is fetched, with wrong-password feedback matching the TUI behavior
- [ ] #2 Settings screen edits company name, toggles update check, and shows the data directory
- [ ] #3 Password set, change, and remove flows work with confirmation
- [ ] #4 Switching data directory reloads the whole app cleanly
<!-- AC:END -->

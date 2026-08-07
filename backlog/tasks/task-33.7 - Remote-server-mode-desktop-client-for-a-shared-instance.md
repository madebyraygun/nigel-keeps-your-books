---
id: TASK-33.7
title: 'Remote server mode: desktop client for a shared instance'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - multiuser
dependencies:
  - TASK-33.2
  - TASK-32.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Let the desktop app connect to a shared multiuser server instead of its local database: a connection settings screen (server URL plus login), the remote backend for the api client, a clear local-versus-remote indicator, and graceful offline/unreachable handling. Local and remote data never mix. Bridges this epic with the multiuser epic.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app can connect and log in to a remote multiuser server
- [ ] #2 Switching between local and remote profiles is explicit with a visible mode indicator
- [ ] #3 Unreachable server states degrade gracefully with retry, never data loss
- [ ] #4 Local and remote data are never mixed in one view
<!-- AC:END -->

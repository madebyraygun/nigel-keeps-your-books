---
id: TASK-33.5
title: Desktop auto-update via the Tauri updater
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - backend
dependencies:
  - TASK-33.2
references:
  - src/cli/update.rs
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire the Tauri updater plugin to GitHub Releases with signed update manifests for the desktop app, while the CLI keeps its existing self_replace update path (cli/update.rs). One release pipeline feeds both: CLI binaries plus desktop bundles and the update manifest.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app checks, downloads, and installs updates from GitHub Releases with signature verification
- [ ] #2 CLI self-update behavior is unchanged
- [ ] #3 Release CI publishes CLI artifacts, desktop bundles, and the update manifest from one pipeline
<!-- AC:END -->

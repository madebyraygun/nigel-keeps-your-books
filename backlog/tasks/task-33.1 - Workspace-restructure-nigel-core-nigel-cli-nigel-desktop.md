---
id: TASK-33.1
title: 'Workspace restructure: nigel-core, nigel-cli, nigel-desktop'
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - backend
dependencies:
  - TASK-31.1
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split the crate into a cargo workspace: nigel-core holds the data layer, importers, reports, migrations, and settings with no ratatui/crossterm/clap dependencies; the nigel crate keeps the CLI/TUI binary (name and behavior unchanged); nigel-desktop is the Tauri shell scaffold. Builds on the lib/bin split from the web epic. Release workflows keep producing the same CLI artifacts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Workspace builds with core free of TUI and CLI dependencies
- [ ] #2 The nigel binary keeps its name, features, and behavior; cargo test passes across the workspace
- [ ] #3 Release CI still produces the existing CLI binaries for all platforms
<!-- AC:END -->

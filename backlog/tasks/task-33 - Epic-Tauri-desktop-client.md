---
id: TASK-33
title: 'Epic: Tauri desktop client'
status: To Do
assignee: []
created_date: '2026-08-06 16:28'
labels:
  - epic
  - tauri
dependencies:
  - TASK-31
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Package the SPA as a native desktop app with Tauri 2 so Nigel runs without a terminal. Restructure into a cargo workspace — nigel-core (library, no TUI deps), nigel (existing CLI/TUI binary), nigel-desktop (Tauri shell) — and reuse the exact same SPA through the api client seam established in the web epic: local embedded backend by default, optional remote mode connecting to a shared multiuser server. Native niceties replace web workarounds: file-open dialogs instead of uploads, the Tauri updater plugin instead of self_replace, OS keychain for optional password remembering, and signed installers from release CI. Depends on the web epic (task-31); remote mode depends on multiuser auth (task-32).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The desktop app runs the full Nigel UI against a local database with no terminal involved
- [ ] #2 The SPA codebase is shared with web mode — no fork — switched via api client backends
- [ ] #3 The CLI/TUI binary is unaffected and still ships from the same repository
- [ ] #4 Signed installers for macOS, Windows, and Linux are produced by release CI
- [ ] #5 Optional remote mode connects to a shared multiuser server
<!-- AC:END -->

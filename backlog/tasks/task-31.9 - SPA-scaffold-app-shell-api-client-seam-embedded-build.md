---
id: TASK-31.9
title: 'SPA scaffold: app shell, api client seam, embedded build'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.3
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.9-spa-scaffold.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the web/ SPA (Vite; framework, component library, and patterns carried over from boxcraft-app). The load-bearing piece is a single api client module that isolates transport: fetch backend now, with the interface shaped so a Tauri invoke backend and a remote-server backend can be swapped in later without touching components. App shell includes routing, navigation, layout, theme, and centralized loading/error/locked/unauthorized states. Build integration embeds the SPA dist into the binary via rust-embed, with the dev workflow (vite dev server proxying to a running nigel serve) and CI story documented.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 SPA builds and is embedded in the binary; nigel serve serves it at the root path
- [ ] #2 All server communication flows through the single api client module — no scattered fetch calls — with token and locked-state handling centralized
- [ ] #3 Routing and navigation shell exists with routes stubbed for all planned screens
- [ ] #4 Dev workflow (vite proxy against nigel serve) is documented and works
- [ ] #5 CI builds the SPA and cargo test stays green
<!-- AC:END -->
